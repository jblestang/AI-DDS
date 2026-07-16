//! E2E: keyed instance lifecycle over the wire.

mod common;

use common::{
    create_participant_pair, read_next_wire, spawn_receivers, wire_bidirectional_discovery,
    wire_type_support, WireMessage,
};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, PublisherQos, ReliabilityKind, SubscriberQos, TopicQos,
};
use std::time::Duration;

const DOMAIN: u32 = 60;
const TOPIC: &str = "E2EInstanceTopic";
const TYPE: &str = "WireMessage";

#[test]
fn e2e_dispose_prevents_further_writes() {
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

    wire_bidirectional_discovery(
        &pair.pub_participant,
        &pair.sub_participant,
        subscriber.unicast_port(),
        writer.guid(),
        reader.guid(),
        &wire,
    );

    let sample = WireMessage {
        id: 42,
        payload: "before-dispose".to_string(),
    };
    let handle = writer.register_instance(&sample).unwrap();
    writer.write(&sample).unwrap();

    assert_eq!(
        read_next_wire(&reader, Duration::from_secs(3)),
        Some(sample.clone())
    );

    writer.dispose(handle).unwrap();
    assert!(
        writer.write(&sample).is_err(),
        "Writing to disposed instance should fail"
    );
}

#[test]
fn e2e_unregister_instance_allows_reregistration() {
    let pair = create_participant_pair(DOMAIN + 1);
    let ts = wire_type_support();

    pair.pub_participant.register_type(TYPE, ts.clone()).unwrap();
    let pub_topic = pair
        .pub_participant
        .create_topic(TOPIC, TYPE, TopicQos::default())
        .unwrap();

    let publisher = pair
        .pub_participant
        .create_publisher(PublisherQos::default())
        .unwrap();
    let writer = publisher
        .create_datawriter(&pub_topic, DataWriterQos::default(), ts)
        .unwrap();

    let sample = WireMessage {
        id: 99,
        payload: "lifecycle".to_string(),
    };
    let handle = writer.register_instance(&sample).unwrap();
    writer.write(&sample).unwrap();
    writer.unregister_instance(handle).unwrap();

    // After unregister, a new register for the same key should succeed
    let new_handle = writer.register_instance(&sample).unwrap();
    assert!(writer.write(&sample).is_ok());
    writer.dispose(new_handle).unwrap();
}

#[test]
fn e2e_multiple_instances_delivered_in_order() {
    let pair = create_participant_pair(DOMAIN + 2);
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

    wire_bidirectional_discovery(
        &pair.pub_participant,
        &pair.sub_participant,
        subscriber.unicast_port(),
        writer.guid(),
        reader.guid(),
        &wire,
    );

    let instances = [
        WireMessage {
            id: 1,
            payload: "inst-1".to_string(),
        },
        WireMessage {
            id: 2,
            payload: "inst-2".to_string(),
        },
        WireMessage {
            id: 3,
            payload: "inst-3".to_string(),
        },
    ];

    for inst in &instances {
        writer.register_instance(inst).unwrap();
        writer.write(inst).unwrap();

        let received = common::read_next_wire(&reader, Duration::from_secs(3));
        assert_eq!(received.as_ref().map(|m| m.id), Some(inst.id));
    }
}
