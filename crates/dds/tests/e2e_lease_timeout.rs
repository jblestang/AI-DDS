//! E2E: SPDP lease expiry removes stale remote participants from discovery.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{
    create_participant_pair, default_wire, inject_remote_participant_with_lease,
    plain_type_support, read_next_plain, spawn_receivers, wire_pub_to_sub_reader, PlainMessage,
};
use dds::types::guid::GuidPrefix;
use dds::types::qos::{
    DataReaderQos, DataWriterQos, PublisherQos, ReliabilityKind, SubscriberQos, TopicQos,
};
use std::time::Duration;

const DOMAIN: u32 = 86;
const TOPIC: &str = "E2ELeaseTopic";
const TYPE: &str = "PlainMessage";

#[test]
fn e2e_expired_participant_lease_removed_from_discovery() {
    let pair = create_participant_pair(DOMAIN);
    let stale_prefix = GuidPrefix::new([0xCD; 12]);
    let locator = common::localhost_locator(54321);

    inject_remote_participant_with_lease(
        &pair.sub_participant,
        stale_prefix,
        locator,
        dds::types::time::Duration::from_secs(1),
    );

    {
        let disc = pair.sub_participant.discovery.lock().unwrap();
        assert!(
            disc.discovered_participants().contains_key(&stale_prefix),
            "Injected participant should be present before lease expiry"
        );
    }

    std::thread::sleep(Duration::from_millis(1100));

    {
        let mut disc = pair.sub_participant.discovery.lock().unwrap();
        disc.check_lease_timeouts();
        assert!(
            !disc.discovered_participants().contains_key(&stale_prefix),
            "Expired lease should remove remote participant"
        );
    }
}

#[test]
fn e2e_fresh_lease_keeps_participant_alive() {
    let pair = create_participant_pair(DOMAIN + 1);
    let remote_prefix = GuidPrefix::new([0xAB; 12]);
    let locator = common::localhost_locator(12345);

    inject_remote_participant_with_lease(
        &pair.sub_participant,
        remote_prefix,
        locator,
        dds::types::time::Duration::from_secs(30),
    );

    {
        let mut disc = pair.sub_participant.discovery.lock().unwrap();
        disc.check_lease_timeouts();
        assert!(
            disc.discovered_participants().contains_key(&remote_prefix),
            "Fresh lease should keep participant alive"
        );
    }
}

#[test]
fn e2e_short_lease_participant_delivers_before_expiry() {
    let pair = create_participant_pair(DOMAIN + 2);
    let pub_prefix = pair.pub_participant.guid_prefix();
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

    inject_remote_participant_with_lease(
        &pair.sub_participant,
        pub_prefix,
        common::localhost_locator(pair.pub_participant.unicast_port()),
        dds::types::time::Duration::from_secs(30),
    );

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

    let sample = PlainMessage {
        content: "within-lease-window".to_string(),
    };
    writer.write(&sample).unwrap();
    assert_eq!(
        read_next_plain(&reader, Duration::from_secs(3)).as_ref(),
        Some(&sample)
    );

    {
        let disc = pair.sub_participant.discovery.lock().unwrap();
        assert!(
            disc.discovered_participants().contains_key(&pub_prefix),
            "Active remote participant should remain while lease is valid"
        );
    }
}
