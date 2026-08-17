//! E2E: SampleInfo metadata on wire-delivered samples.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{
    create_participant_pair, default_wire, plain_type_support, spawn_receivers,
    wire_bidirectional_discovery, PlainMessage,
};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, PublisherQos, ReliabilityKind, SubscriberQos, TopicQos,
};
use std::time::Duration;

const DOMAIN: u32 = 77;
const TOPIC: &str = "E2ESampleInfoTopic";
const TYPE: &str = "PlainMessage";

#[test]
fn e2e_wire_sample_has_valid_data_sample_info() {
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

    writer
        .write(&PlainMessage {
            content: "sample-info".to_string(),
        })
        .unwrap();

    assert!(common::wait_until(Duration::from_secs(3), || reader.read().is_ok()));

    let (boxed, info) = reader.read().unwrap();
    assert!(info.valid_data);
    assert_eq!(
        boxed.downcast_ref::<PlainMessage>().unwrap().content,
        "sample-info"
    );
}
