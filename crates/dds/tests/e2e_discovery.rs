//! E2E: discovery-driven matchmaking over the wire.

mod common;

use common::{
    create_participant_pair, default_wire, plain_type_support, read_next_plain, spawn_receivers,
    wire_bidirectional_discovery, wire_pub_to_sub_reader, PlainMessage,
};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, PublisherQos, ReliabilityKind, SubscriberQos, TopicQos,
};
use std::time::Duration;

const DOMAIN: u32 = 56;
const TOPIC: &str = "E2EDiscoveryTopic";
const TYPE: &str = "PlainMessage";

#[test]
fn e2e_bidirectional_discovery_delivers_sample() {
    let pair = create_participant_pair(DOMAIN);
    let ts = plain_type_support();

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

    spawn_receivers(&pair);
    std::thread::sleep(Duration::from_millis(40));

    let publisher = pair
        .pub_participant
        .create_publisher(PublisherQos::default())
        .unwrap();
    let writer = publisher
        .create_datawriter(&pub_topic, DataWriterQos::default(), ts)
        .unwrap();

    wire_bidirectional_discovery(
        &pair.pub_participant,
        &pair.sub_participant,
        subscriber.unicast_port(),
        writer.guid(),
        reader.guid(),
        &default_wire(TOPIC, TYPE),
    );

    let sample = PlainMessage {
        content: "discovery-bidirectional".to_string(),
    };
    writer.write(&sample).unwrap();

    let received = read_next_plain(&reader, Duration::from_secs(3));
    assert_eq!(received, Some(sample));
}

#[test]
fn e2e_pub_to_sub_unidirectional_delivery() {
    let pair = create_participant_pair(DOMAIN + 1);
    let ts = plain_type_support();

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
    let writer = publisher
        .create_datawriter(&pub_topic, DataWriterQos::default(), ts)
        .unwrap();

    let _pub_rx = pair.pub_participant.spawn_receiver_loop();
    std::thread::sleep(Duration::from_millis(40));

    wire_pub_to_sub_reader(
        &pair.pub_participant,
        pair.sub_participant.guid_prefix(),
        common::localhost_locator(subscriber.unicast_port()),
        reader.guid(),
        &default_wire(TOPIC, TYPE),
    );

    writer
        .write(&PlainMessage {
            content: "pub-to-sub".to_string(),
        })
        .unwrap();

    let received = read_next_plain(&reader, Duration::from_secs(3));
    assert_eq!(
        received.map(|m| m.content),
        Some("pub-to-sub".to_string())
    );
}

#[test]
fn e2e_incompatible_qos_blocks_wire_delivery() {
    let pair = create_participant_pair(DOMAIN + 2);
    let ts = plain_type_support();

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
    writer_qos.reliability.kind = ReliabilityKind::BestEffort;
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts)
        .unwrap();

    let mut wire = default_wire(TOPIC, TYPE);
    wire.writer_qos = Some(writer_qos);
    wire.reader_qos = Some(reader_qos);

    wire_pub_to_sub_reader(
        &pair.pub_participant,
        pair.sub_participant.guid_prefix(),
        common::localhost_locator(subscriber.unicast_port()),
        reader.guid(),
        &wire,
    );

    writer
        .write(&PlainMessage {
            content: "should-not-arrive".to_string(),
        })
        .unwrap();

    std::thread::sleep(Duration::from_millis(250));
    assert!(
        reader.read_next().is_err(),
        "Incompatible reliability should block reader proxy and delivery"
    );
}

#[test]
fn e2e_partition_mismatch_blocks_wire_delivery() {
    let pair = create_participant_pair(DOMAIN + 3);
    let ts = plain_type_support();

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

    spawn_receivers(&pair);

    let mut pub_qos = PublisherQos::default();
    pub_qos.partition.name = vec!["lab-a".to_string()];
    let publisher = pair.pub_participant.create_publisher(pub_qos).unwrap();
    let writer = publisher
        .create_datawriter(&pub_topic, DataWriterQos::default(), ts)
        .unwrap();

    let mut wire = default_wire(TOPIC, TYPE);
    wire.partition = vec!["lab-b".to_string()];

    wire_pub_to_sub_reader(
        &pair.pub_participant,
        pair.sub_participant.guid_prefix(),
        common::localhost_locator(subscriber.unicast_port()),
        reader.guid(),
        &wire,
    );

    writer
        .write(&PlainMessage {
            content: "partition-blocked".to_string(),
        })
        .unwrap();

    std::thread::sleep(Duration::from_millis(250));
    assert!(
        reader.read_next().is_err(),
        "Partition mismatch should prevent matchmaking"
    );
}
