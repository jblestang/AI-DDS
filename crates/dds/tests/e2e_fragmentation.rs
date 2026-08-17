//! E2E: RTPS fragmentation for large payloads over the wire.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{
    create_participant_pair, read_next_wire, spawn_receivers, wire_bidirectional_discovery,
    wire_type_support, WireMessage,
};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, PublisherQos, ReliabilityKind, SubscriberQos, TopicQos,
};
use std::time::Duration;

const DOMAIN: u32 = 63;
const TOPIC: &str = "E2EFragmentTopic";
const TYPE: &str = "WireMessage";

#[test]
fn e2e_large_payload_fragmentation_over_wire() {
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

    let large = WireMessage {
        id: 500,
        payload: "Z".repeat(8000),
    };
    writer.register_instance(&large).unwrap();
    writer.write(&large).unwrap();

    let received = read_next_wire(&reader, Duration::from_secs(5));
    assert_eq!(received, Some(large));
}

#[test]
fn e2e_multiple_large_samples_over_wire() {
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

    wire_bidirectional_discovery(
        &pair.pub_participant,
        &pair.sub_participant,
        subscriber.unicast_port(),
        writer.guid(),
        reader.guid(),
        &wire,
    );

    for i in 0..3 {
        let sample = WireMessage {
            id: 600 + i,
            payload: "X".repeat(5000 + i as usize * 1000),
        };
        writer.register_instance(&sample).unwrap();
        writer.write(&sample).unwrap();

        let received = read_next_wire(&reader, Duration::from_secs(5));
        assert_eq!(received.as_ref().map(|m| m.id), Some(600 + i));
        assert!(received.unwrap().payload.len() >= 5000);
    }
}
