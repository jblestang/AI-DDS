//! E2E: History KEEP_LAST depth enforcement on reader over the wire.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{
    create_participant_pair, default_wire, spawn_receivers, wire_bidirectional_discovery,
    wire_type_support, WireMessage,
};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, History, HistoryKind, PublisherQos, ReliabilityKind,
    SubscriberQos, TopicQos,
};
use std::time::Duration;

const DOMAIN: u32 = 73;
const TOPIC: &str = "E2EHistoryTopic";
const TYPE: &str = "WireMessage";
const HISTORY_DEPTH: i32 = 2;

#[test]
fn e2e_reader_keep_last_depth_over_wire() {
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
    reader_qos.history = History {
        kind: HistoryKind::KeepLast,
        depth: HISTORY_DEPTH,
    };
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
        subscriber.unicast_port(),
        writer.guid(),
        reader.guid(),
        &wire,
    );

    for i in 1..=3 {
        let sample = WireMessage {
            id: i,
            payload: format!("hist-{i}"),
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

    assert!(
        ids.len() <= HISTORY_DEPTH as usize,
        "KEEP_LAST depth={HISTORY_DEPTH} should cap samples, got {ids:?}"
    );
    assert_eq!(ids, vec![2, 3], "Should retain the two newest samples");
}
