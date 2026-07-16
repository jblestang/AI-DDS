//! E2E: durability and QoS behavior over the wire.

mod common;

use common::{
    create_participant_pair, read_next_wire, spawn_receivers, wire_pub_to_sub_reader,
    wire_type_support, WireMessage,
};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, DurabilityKind, PublisherQos, ReliabilityKind, SubscriberQos,
    TopicQos,
};
use std::time::Duration;

const DOMAIN: u32 = 58;
const TOPIC: &str = "E2EDurabilityTopic";
const TYPE: &str = "WireMessage";

#[test]
fn e2e_transient_local_retransmits_to_late_joiner() {
    let pair = create_participant_pair(DOMAIN);
    let ts = wire_type_support();

    pair.sub_participant.register_type(TYPE, ts.clone()).unwrap();
    pair.pub_participant.register_type(TYPE, ts.clone()).unwrap();

    let sub_topic = pair
        .sub_participant
        .create_topic(TOPIC, TYPE, TopicQos::default())
        .unwrap();
    let pub_topic = pair
        .pub_participant
        .create_topic(TOPIC, TYPE, TopicQos::default())
        .unwrap();

    let subscriber = pair
        .sub_participant
        .create_subscriber(SubscriberQos::default())
        .unwrap();
    let reader = subscriber
        .create_datareader(&sub_topic, DataReaderQos::default(), ts.clone())
        .unwrap();

    let _sub_rx = pair.sub_participant.spawn_receiver_loop();

    let publisher = pair
        .pub_participant
        .create_publisher(PublisherQos::default())
        .unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.durability.kind = DurabilityKind::TransientLocal;
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts)
        .unwrap();

    let _pub_rx = pair.pub_participant.spawn_receiver_loop();
    std::thread::sleep(Duration::from_millis(40));

    let sample = WireMessage {
        id: 1,
        payload: "durability-cached".to_string(),
    };
    writer.register_instance(&sample).unwrap();
    writer.write(&sample).unwrap();

    let mut wire = common::default_wire(TOPIC, TYPE);
    wire.writer_qos = Some(writer_qos);
    wire.reader_qos = Some(DataReaderQos::default());

    wire_pub_to_sub_reader(
        &pair.pub_participant,
        pair.sub_participant.guid_prefix(),
        common::localhost_locator(subscriber.unicast_port()),
        reader.guid(),
        &wire,
    );

    let received = read_next_wire(&reader, Duration::from_secs(3));
    assert_eq!(received, Some(sample));
}

#[test]
fn e2e_reliable_delivery_over_wire() {
    let pair = create_participant_pair(DOMAIN + 1);
    let ts = wire_type_support();

    pair.sub_participant.register_type(TYPE, ts.clone()).unwrap();
    pair.pub_participant.register_type(TYPE, ts.clone()).unwrap();

    let sub_topic = pair
        .sub_participant
        .create_topic(TOPIC, TYPE, TopicQos::default())
        .unwrap();
    let pub_topic = pair
        .pub_participant
        .create_topic(TOPIC, TYPE, TopicQos::default())
        .unwrap();

    let subscriber = pair
        .sub_participant
        .create_subscriber(SubscriberQos::default())
        .unwrap();
    let mut reader_qos = DataReaderQos::default();
    reader_qos.reliability.kind = ReliabilityKind::Reliable;
    let reader = subscriber
        .create_datareader(&sub_topic, reader_qos.clone(), ts.clone())
        .unwrap();

    spawn_receivers(&pair);

    let publisher = pair
        .pub_participant
        .create_publisher(PublisherQos::default())
        .unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts)
        .unwrap();

    let mut wire = common::default_wire(TOPIC, TYPE);
    wire.writer_qos = Some(writer_qos);
    wire.reader_qos = Some(reader_qos);

    common::wire_bidirectional_discovery(
        &pair.pub_participant,
        &pair.sub_participant,
        subscriber.unicast_port(),
        writer.guid(),
        reader.guid(),
        &wire,
    );

    for i in 1..=5 {
        let sample = WireMessage {
            id: i,
            payload: format!("reliable-{i}"),
        };
        writer.register_instance(&sample).unwrap();
        writer.write(&sample).unwrap();

        let received = read_next_wire(&reader, Duration::from_secs(3));
        assert_eq!(received.as_ref().map(|m| m.id), Some(i));
    }
}
