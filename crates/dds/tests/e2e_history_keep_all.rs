//! E2E: History KEEP_ALL retains all received samples over the wire.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{
    create_participant_pair, default_wire, spawn_receivers, wire_bidirectional_discovery,
    wire_type_support, WireMessage,
};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, HistoryKind, PublisherQos, ReliabilityKind,
    SubscriberQos, TopicQos,
};
use std::time::Duration;

const DOMAIN: u32 = 85;
const TOPIC: &str = "E2EKeepAllTopic";
const TYPE: &str = "WireMessage";

#[test]
fn e2e_reader_keep_all_retains_all_samples_over_wire() {
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
    reader_qos.history.kind = HistoryKind::KeepAll;
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

    let mut wire = default_wire(TOPIC, TYPE);
    wire.writer_qos = Some(writer_qos);
    wire.reader_qos = Some(reader_qos);

    wire_bidirectional_discovery(
        &pair.pub_participant,
        &pair.sub_participant,
        writer.guid(),
        reader.guid(),
        &wire,
    );

    for i in 1..=3 {
        let sample = WireMessage {
            id: i,
            payload: format!("keep-all-{i}"),
        };
        writer.register_instance(&sample).unwrap();
        writer.write(&sample).unwrap();
        std::thread::sleep(Duration::from_millis(80));
    }

    assert!(
        common::wait_until(Duration::from_secs(3), || reader.read().is_ok()),
        "At least one sample should arrive"
    );

    let mut ids = Vec::new();
    while let Ok((boxed, _)) = reader.take() {
        if let Some(msg) = boxed.downcast_ref::<WireMessage>() {
            ids.push(msg.id);
        }
    }

    assert_eq!(ids, vec![1, 2, 3], "KEEP_ALL should retain every sample");
}
