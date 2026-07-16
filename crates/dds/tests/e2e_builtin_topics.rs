//! E2E: builtin topics registration and monitor snapshot.

mod common;

use common::{create_participant_pair, spawn_receivers, wire_bidirectional_discovery, wire_type_support};
use dds::types::builtin_topics::{PUBLICATION_TOPIC_NAME, SUBSCRIPTION_TOPIC_NAME};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, PublisherQos, ReliabilityKind, SubscriberQos, TopicQos,
};
use std::time::Duration;

const DOMAIN: u32 = 62;
const TOPIC: &str = "E2EBuiltinTopic";
const TYPE: &str = "WireMessage";

#[test]
fn e2e_enable_builtin_topics_registers_dcps_endpoints() {
    let pair = create_participant_pair(DOMAIN);
    pair.pub_participant.enable_builtin_topics().unwrap();

    let reg = pair.pub_participant.discovery.lock().unwrap();
    let topics: Vec<_> = reg
        .local_endpoints()
        .values()
        .map(|e| e.topic_name.as_str())
        .collect();

    assert!(topics.contains(&PUBLICATION_TOPIC_NAME));
    assert!(topics.contains(&SUBSCRIPTION_TOPIC_NAME));
}

#[test]
fn e2e_monitor_snapshot_reflects_matched_endpoints() {
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

    let pub_snapshot = pair.pub_participant.monitor_snapshot();
    let sub_snapshot = pair.sub_participant.monitor_snapshot();

    assert!(
        pub_snapshot
            .endpoints
            .iter()
            .any(|e| e.topic_name == TOPIC && e.is_writer),
        "Publisher snapshot should list the DataWriter"
    );
    assert!(
        sub_snapshot
            .endpoints
            .iter()
            .any(|e| e.topic_name == TOPIC && !e.is_writer),
        "Subscriber snapshot should list the DataReader"
    );
    assert!(
        pub_snapshot
            .endpoints
            .iter()
            .any(|e| e.guid == reader.guid()),
        "Publisher should see remote reader in snapshot"
    );
}

#[test]
fn e2e_monitor_snapshot_lists_discovered_participants() {
    let pair = create_participant_pair(DOMAIN + 2);
    spawn_receivers(&pair);
    std::thread::sleep(Duration::from_millis(50));

    common::inject_remote_participant(
        &pair.pub_participant,
        pair.sub_participant.guid_prefix(),
        common::localhost_locator(pair.sub_participant.unicast_port()),
    );

    let snapshot = pair.pub_participant.monitor_snapshot();
    assert!(
        snapshot
            .participants
            .iter()
            .any(|p| p.guid_prefix == pair.sub_participant.guid_prefix()),
        "Monitor snapshot should include discovered remote participant"
    );
}
