//! E2E: partition QoS — matching partitions allow wire delivery.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{
    create_participant_pair, default_wire, plain_type_support, read_next_plain, spawn_receivers,
    wire_pub_to_sub_reader, PlainMessage,
};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, PublisherQos, ReliabilityKind, SubscriberQos, TopicQos,
};
use std::time::Duration;

const DOMAIN: u32 = 71;
const TOPIC: &str = "E2EPartitionTopic";
const TYPE: &str = "PlainMessage";
const PARTITION: &str = "compliance-lab";

#[test]
fn e2e_matching_partition_delivers_over_wire() {
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

    let mut sub_qos = SubscriberQos::default();
    sub_qos.partition.name = vec![PARTITION.to_string()];
    let subscriber = pair
        .sub_participant
        .create_subscriber(sub_qos)
        .unwrap();
    let mut reader_qos = DataReaderQos::default();
    reader_qos.reliability.kind = ReliabilityKind::Reliable;
    let reader = subscriber
        .create_datareader(&sub_topic, reader_qos.clone(), ts.clone())
        .unwrap();

    spawn_receivers(&pair);

    let mut pub_qos = PublisherQos::default();
    pub_qos.partition.name = vec![PARTITION.to_string()];
    let publisher = pair.pub_participant.create_publisher(pub_qos).unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts)
        .unwrap();

    let mut wire = default_wire(TOPIC, TYPE);
    wire.partition = vec![PARTITION.to_string()];
    wire.writer_qos = Some(writer_qos);
    wire.reader_qos = Some(reader_qos);

    wire_pub_to_sub_reader(
        &pair.pub_participant,
        &pair.sub_participant,
        common::user_data_locator(&pair.sub_participant),
        writer.guid(),
        reader.guid(),
        &wire,
    );

    let sample = PlainMessage {
        content: "partition-match".to_string(),
    };
    writer.write(&sample).unwrap();

    assert_eq!(
        read_next_plain(&reader, Duration::from_secs(3)).as_ref(),
        Some(&sample)
    );
}
