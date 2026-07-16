//! E2E: partition QoS matching rules over the wire.

mod common;

use common::{
    create_participant_pair, default_wire, plain_type_support, read_next_plain, spawn_receivers,
    wire_pub_to_sub_reader, PlainMessage,
};
use dds::core::check_partition_compatibility;
use dds::types::qos::{
    DataReaderQos, DataWriterQos, Partition, PublisherQos, ReliabilityKind, SubscriberQos,
    TopicQos,
};
use std::time::Duration;

const DOMAIN: u32 = 87;
const TOPIC: &str = "E2EPartitionComplianceTopic";
const TYPE: &str = "PlainMessage";

#[test]
fn e2e_default_empty_partitions_match() {
    assert!(check_partition_compatibility(
        &[],
        &Partition { name: vec![] }
    ));

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

    let publisher = pair.pub_participant.create_publisher(PublisherQos::default()).unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts)
        .unwrap();

    let mut wire = default_wire(TOPIC, TYPE);
    wire.partition = vec![];
    wire.writer_qos = Some(writer_qos);
    wire.reader_qos = Some(reader_qos);

    wire_pub_to_sub_reader(
        &pair.pub_participant,
        pair.sub_participant.guid_prefix(),
        common::localhost_locator(subscriber.unicast_port()),
        reader.guid(),
        &wire,
    );

    let sample = PlainMessage {
        content: "empty-partition-match".to_string(),
    };
    writer.write(&sample).unwrap();

    assert_eq!(
        read_next_plain(&reader, Duration::from_secs(3)).as_ref(),
        Some(&sample)
    );
}

#[test]
fn e2e_partition_mismatch_blocks_wire_delivery() {
    assert!(!check_partition_compatibility(
        &["lab-a".to_string()],
        &Partition {
            name: vec!["lab-b".to_string()],
        }
    ));

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
    let mut reader_qos = DataReaderQos::default();
    reader_qos.reliability.kind = ReliabilityKind::Reliable;
    let reader = subscriber
        .create_datareader(&sub_topic, reader_qos.clone(), ts.clone())
        .unwrap();

    spawn_receivers(&pair);

    let mut pub_qos = PublisherQos::default();
    pub_qos.partition.name = vec!["lab-a".to_string()];
    let publisher = pair.pub_participant.create_publisher(pub_qos).unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts)
        .unwrap();

    let mut wire = default_wire(TOPIC, TYPE);
    wire.partition = vec!["lab-b".to_string()];
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
            content: "partition-mismatch".to_string(),
        })
        .unwrap();

    std::thread::sleep(Duration::from_millis(250));
    assert!(
        reader.read_next().is_err(),
        "Non-overlapping partitions should prevent matchmaking"
    );
}

#[test]
fn e2e_multi_partition_overlap_delivers_over_wire() {
    assert!(check_partition_compatibility(
        &["alpha".to_string(), "shared".to_string()],
        &Partition {
            name: vec!["shared".to_string(), "beta".to_string()],
        }
    ));

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

    let mut pub_qos = PublisherQos::default();
    pub_qos.partition.name = vec!["alpha".to_string(), "shared".to_string()];
    let publisher = pair.pub_participant.create_publisher(pub_qos).unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts)
        .unwrap();

    let mut wire = default_wire(TOPIC, TYPE);
    wire.partition = vec!["shared".to_string(), "beta".to_string()];
    wire.writer_qos = Some(writer_qos);
    wire.reader_qos = Some(reader_qos);

    wire_pub_to_sub_reader(
        &pair.pub_participant,
        pair.sub_participant.guid_prefix(),
        common::localhost_locator(subscriber.unicast_port()),
        reader.guid(),
        &wire,
    );

    let sample = PlainMessage {
        content: "multi-partition-overlap".to_string(),
    };
    writer.write(&sample).unwrap();

    assert_eq!(
        read_next_plain(&reader, Duration::from_secs(3)).as_ref(),
        Some(&sample)
    );
}
