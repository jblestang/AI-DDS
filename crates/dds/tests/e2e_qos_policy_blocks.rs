//! E2E: additional QoS policy compatibility blocks wire delivery.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{
    create_participant_pair, default_wire, plain_type_support, spawn_receivers, wire_pub_to_sub_reader,
};
use dds::core::check_qos_compatibility;
use dds::types::qos::{
    DataReaderQos, DataWriterQos, Deadline, DestinationOrder, DestinationOrderKind, Liveliness,
    LivelinessKind, Ownership, OwnershipKind, PublisherQos, ReliabilityKind, SubscriberQos,
    TopicQos,
};
use dds::types::qos::LatencyBudget;
use dds::types::time::Duration as DdsDuration;
use std::time::Duration;

const DOMAIN: u32 = 76;
const TOPIC: &str = "E2EQosPolicyTopic";
const TYPE: &str = "PlainMessage";

#[test]
fn e2e_ownership_mismatch_blocks_wire_delivery() {
    assert!(!check_qos_compatibility(
        &{
            let mut w = DataWriterQos::default();
            w.ownership.kind = OwnershipKind::Exclusive;
            w
        },
        &{
            let mut r = DataReaderQos::default();
            r.ownership.kind = OwnershipKind::Shared;
            r
        }
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
    reader_qos.ownership.kind = OwnershipKind::Shared;
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
    writer_qos.ownership.kind = OwnershipKind::Exclusive;
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts)
        .unwrap();

    let mut wire = default_wire(TOPIC, TYPE);
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

    writer
        .write(&common::PlainMessage {
            content: "ownership-blocked".to_string(),
        })
        .unwrap();

    std::thread::sleep(Duration::from_millis(250));
    assert!(
        reader.read_next().is_err(),
        "Ownership mismatch should prevent matchmaking"
    );
}

#[test]
fn e2e_deadline_mismatch_blocks_wire_delivery() {
    let mut writer_qos = DataWriterQos::default();
    writer_qos.deadline.period = DdsDuration::from_secs(10);
    let mut reader_qos = DataReaderQos::default();
    reader_qos.deadline.period = DdsDuration::from_secs(1);
    assert!(!check_qos_compatibility(&writer_qos, &reader_qos));

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
    reader_qos.reliability.kind = ReliabilityKind::Reliable;
    let reader = subscriber
        .create_datareader(&sub_topic, reader_qos.clone(), ts.clone())
        .unwrap();

    spawn_receivers(&pair);

    let publisher = pair
        .pub_participant
        .create_publisher(PublisherQos::default())
        .unwrap();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts)
        .unwrap();

    let mut wire = default_wire(TOPIC, TYPE);
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

    writer
        .write(&common::PlainMessage {
            content: "deadline-blocked".to_string(),
        })
        .unwrap();

    std::thread::sleep(Duration::from_millis(250));
    assert!(
        reader.read_next().is_err(),
        "Deadline mismatch should prevent matchmaking"
    );
}

#[test]
fn e2e_durability_upgrade_required_for_transient_local_reader() {
    let mut writer_qos = DataWriterQos::default();
    writer_qos.durability.kind = dds::types::qos::DurabilityKind::Volatile;
    let mut reader_qos = DataReaderQos::default();
    reader_qos.durability.kind = dds::types::qos::DurabilityKind::TransientLocal;
    assert!(!check_qos_compatibility(&writer_qos, &reader_qos));

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
    reader_qos.reliability.kind = ReliabilityKind::Reliable;
    let reader = subscriber
        .create_datareader(&sub_topic, reader_qos.clone(), ts.clone())
        .unwrap();

    spawn_receivers(&pair);

    let publisher = pair
        .pub_participant
        .create_publisher(PublisherQos::default())
        .unwrap();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts)
        .unwrap();

    let mut wire = default_wire(TOPIC, TYPE);
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

    writer
        .write(&common::PlainMessage {
            content: "durability-blocked".to_string(),
        })
        .unwrap();

    std::thread::sleep(Duration::from_millis(250));
    assert!(
        reader.read_next().is_err(),
        "Volatile writer cannot satisfy TransientLocal reader"
    );
}

#[test]
fn e2e_liveliness_mismatch_blocks_wire_delivery() {
    let mut writer_qos = DataWriterQos::default();
    writer_qos.liveliness.kind = LivelinessKind::Automatic;
    writer_qos.liveliness.lease_duration = DdsDuration::INFINITE;
    let mut reader_qos = DataReaderQos::default();
    reader_qos.liveliness.kind = LivelinessKind::ManualByTopic;
    reader_qos.liveliness.lease_duration = DdsDuration::INFINITE;
    assert!(!check_qos_compatibility(&writer_qos, &reader_qos));

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
    reader_qos.reliability.kind = ReliabilityKind::Reliable;
    let reader = subscriber
        .create_datareader(&sub_topic, reader_qos.clone(), ts.clone())
        .unwrap();

    spawn_receivers(&pair);

    let publisher = pair
        .pub_participant
        .create_publisher(PublisherQos::default())
        .unwrap();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts)
        .unwrap();

    let mut wire = default_wire(TOPIC, TYPE);
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

    writer
        .write(&common::PlainMessage {
            content: "liveliness-blocked".to_string(),
        })
        .unwrap();

    std::thread::sleep(Duration::from_millis(250));
    assert!(
        reader.read_next().is_err(),
        "Liveliness mismatch should prevent matchmaking"
    );
}

#[test]
fn e2e_destination_order_mismatch_blocks_wire_delivery() {
    let mut writer_qos = DataWriterQos::default();
    writer_qos.destination_order.kind = DestinationOrderKind::ByReceptionTimestamp;
    let mut reader_qos = DataReaderQos::default();
    reader_qos.destination_order.kind = DestinationOrderKind::BySourceTimestamp;
    assert!(!check_qos_compatibility(&writer_qos, &reader_qos));

    let pair = create_participant_pair(DOMAIN + 4);
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
    reader_qos.reliability.kind = ReliabilityKind::Reliable;
    let reader = subscriber
        .create_datareader(&sub_topic, reader_qos.clone(), ts.clone())
        .unwrap();

    spawn_receivers(&pair);

    let publisher = pair
        .pub_participant
        .create_publisher(PublisherQos::default())
        .unwrap();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts)
        .unwrap();

    let mut wire = default_wire(TOPIC, TYPE);
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

    writer
        .write(&common::PlainMessage {
            content: "destination-order-blocked".to_string(),
        })
        .unwrap();

    std::thread::sleep(Duration::from_millis(250));
    assert!(
        reader.read_next().is_err(),
        "DestinationOrder mismatch should prevent matchmaking"
    );
}

#[test]
fn e2e_latency_budget_mismatch_blocks_wire_delivery() {
    let mut writer_qos = DataWriterQos::default();
    writer_qos.latency_budget.duration = DdsDuration::from_millis(500);
    let mut reader_qos = DataReaderQos::default();
    reader_qos.latency_budget.duration = DdsDuration::from_millis(100);
    assert!(!check_qos_compatibility(&writer_qos, &reader_qos));

    let pair = create_participant_pair(DOMAIN + 5);
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
    reader_qos.reliability.kind = ReliabilityKind::Reliable;
    let reader = subscriber
        .create_datareader(&sub_topic, reader_qos.clone(), ts.clone())
        .unwrap();

    spawn_receivers(&pair);

    let publisher = pair
        .pub_participant
        .create_publisher(PublisherQos::default())
        .unwrap();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&pub_topic, writer_qos.clone(), ts)
        .unwrap();

    let mut wire = default_wire(TOPIC, TYPE);
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

    writer
        .write(&common::PlainMessage {
            content: "latency-budget-blocked".to_string(),
        })
        .unwrap();

    std::thread::sleep(Duration::from_millis(250));
    assert!(
        reader.read_next().is_err(),
        "LatencyBudget mismatch should prevent matchmaking"
    );
}
