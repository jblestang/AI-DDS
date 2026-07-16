//! Live interoperability tests against CycloneDDS (optional; requires built binaries).
//!
//! Build CycloneDDS peer apps:
//! ```bash
//! export CYCLONEDDS_PREFIX=/path/to/cyclonedds-install
//! interop/scripts/build-cyclonedds-apps.sh
//! cargo test -p dds --test interop_cyclonedds -- --ignored --nocapture
//! ```

mod common;
mod interop_common;

use common::{wait_until, DiscoveryWire};
use interop_common::{
    cyclonedds_available, output_contains_interop_receive, spawn_cyclonedds_publisher,
    spawn_cyclonedds_subscriber, wait_output, InteropMessage, InteropTypeSupport, INTEROP_DOMAIN,
    INTEROP_TOPIC, INTEROP_TYPE,
};
use dds::core::DomainParticipantFactory;
use dds::types::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, PublisherQos, ReliabilityKind,
    SubscriberQos, TopicQos,
};
use std::sync::Arc;
use std::time::Duration;

fn skip_if_no_cyclonedds() {
    if !cyclonedds_available() {
        eprintln!(
            "SKIP: CycloneDDS interop binaries not found. \
             Run interop/scripts/build-cyclonedds-apps.sh with CYCLONEDDS_PREFIX set."
        );
    }
}

#[test]
#[ignore = "requires CycloneDDS interop binaries (see interop/README.md)"]
fn interop_cyclonedds_publishes_aidds_receives() {
    skip_if_no_cyclonedds();
    if !cyclonedds_available() {
        return;
    }

    let ts = Arc::new(InteropTypeSupport);
    let participant =
        DomainParticipantFactory::create_participant(INTEROP_DOMAIN, DomainParticipantQos::default())
            .expect("participant");
    participant.register_type(INTEROP_TYPE, ts.clone()).unwrap();
    let topic = participant
        .create_topic(INTEROP_TOPIC, INTEROP_TYPE, TopicQos::default())
        .unwrap();

    let mut reader_qos = DataReaderQos::default();
    reader_qos.reliability.kind = ReliabilityKind::Reliable;
    let subscriber = participant.create_subscriber(SubscriberQos::default()).unwrap();
    let reader = subscriber
        .create_datareader(&topic, reader_qos, ts.clone())
        .unwrap();
    let _rx = participant.spawn_receiver_loop();

    std::thread::sleep(Duration::from_millis(500));

    let sample_id = 9001u32;
    let payload = "from-cyclonedds";
    let child = spawn_cyclonedds_publisher(sample_id, payload).expect("spawn publisher");
    let output = wait_output(child, Duration::from_secs(15)).expect("publisher finished");

    assert!(
        String::from_utf8_lossy(&output.stdout).contains("INTEROP_PUBLISH"),
        "CycloneDDS publisher stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let mut received = None;
    assert!(
        wait_until(Duration::from_secs(12), || {
            if let Ok(boxed) = reader.read_next() {
                if let Some(msg) = boxed.downcast_ref::<InteropMessage>() {
                    received = Some(msg.clone());
                    return true;
                }
            }
            false
        }),
        "AI-DDS should receive sample published by CycloneDDS"
    );

    let msg = received.unwrap();
    assert_eq!(msg.id, sample_id);
    assert_eq!(msg.payload, payload);
}

#[test]
#[ignore = "requires CycloneDDS interop binaries (see interop/README.md)"]
fn interop_aidds_publishes_cyclonedds_receives() {
    skip_if_no_cyclonedds();
    if !cyclonedds_available() {
        return;
    }

    let sample_id = 9002u32;
    let payload = "from-aidds";
    let mut sub_child = spawn_cyclonedds_subscriber(Some(sample_id)).expect("spawn subscriber");

    let ts = Arc::new(InteropTypeSupport);
    let participant =
        DomainParticipantFactory::create_participant(INTEROP_DOMAIN + 1, DomainParticipantQos::default())
            .expect("participant");
    participant.register_type(INTEROP_TYPE, ts.clone()).unwrap();
    let topic = participant
        .create_topic(INTEROP_TOPIC, INTEROP_TYPE, TopicQos::default())
        .unwrap();

    let publisher = participant.create_publisher(PublisherQos::default()).unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&topic, writer_qos, ts.clone())
        .unwrap();
    let _rx = participant.spawn_receiver_loop();

    // CycloneDDS subscriber uses INTEROP_DOMAIN from env; override for this test
    sub_child.kill().ok();
    let _ = sub_child.wait();
    let bin = interop_common::cyclonedds_subscriber().unwrap();
    sub_child = std::process::Command::new(bin)
        .arg(sample_id.to_string())
        .env("AIDDS_INTEROP_DOMAIN", (INTEROP_DOMAIN + 1).to_string())
        .env("AIDDS_INTEROP_TIMEOUT_MS", "12000")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("respawn subscriber");

    std::thread::sleep(Duration::from_millis(800));

    let sample = InteropMessage {
        id: sample_id,
        payload: payload.to_string(),
    };
    writer.write(&sample).unwrap();

    let output = wait_output(sub_child, Duration::from_secs(15)).expect("subscriber finished");
    assert!(
        output_contains_interop_receive(&output, sample_id, payload),
        "CycloneDDS subscriber stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires CycloneDDS interop binaries (see interop/README.md)"]
fn interop_bidirectional_discovery_matchmaking() {
    skip_if_no_cyclonedds();
    if !cyclonedds_available() {
        return;
    }

    // AI-DDS reader; inject CycloneDDS writer via discovery after capturing its SPDP would be ideal.
    // This test verifies our stack can match when CycloneDDS reader is running and we publish.
    let sample_id = 9003u32;
    let payload = "discovery-interop";
    let mut sub_child = spawn_cyclonedds_subscriber(Some(sample_id)).expect("spawn subscriber");

    let ts = Arc::new(InteropTypeSupport);
    let participant =
        DomainParticipantFactory::create_participant(INTEROP_DOMAIN + 2, DomainParticipantQos::default())
            .expect("participant");
    participant.register_type(INTEROP_TYPE, ts.clone()).unwrap();
    let topic = participant
        .create_topic(INTEROP_TOPIC, INTEROP_TYPE, TopicQos::default())
        .unwrap();

    let publisher = participant.create_publisher(PublisherQos::default()).unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&topic, writer_qos.clone(), ts.clone())
        .unwrap();
    let _rx = participant.spawn_receiver_loop();

    sub_child.kill().ok();
    let _ = sub_child.wait();
    let bin = interop_common::cyclonedds_subscriber().unwrap();
    sub_child = std::process::Command::new(bin)
        .arg(sample_id.to_string())
        .env("AIDDS_INTEROP_DOMAIN", (INTEROP_DOMAIN + 2).to_string())
        .env("AIDDS_INTEROP_TIMEOUT_MS", "15000")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("respawn subscriber");

    std::thread::sleep(Duration::from_secs(2));

    writer
        .write(&InteropMessage {
            id: sample_id,
            payload: payload.to_string(),
        })
        .unwrap();

    let output = wait_output(sub_child, Duration::from_secs(18)).expect("subscriber finished");
    assert!(output_contains_interop_receive(&output, sample_id, payload));

    // Verify monitor snapshot sees our writer endpoint
    let snap = participant.monitor_snapshot();
    assert!(
        snap.endpoints.iter().any(|e| e.topic_name == INTEROP_TOPIC && e.is_writer),
        "Monitor snapshot should list local interop writer"
    );

    let _ = DiscoveryWire {
        topic: INTEROP_TOPIC.to_string(),
        type_name: INTEROP_TYPE.to_string(),
        partition: vec![],
        type_info: None,
        writer_qos: Some(writer_qos),
        reader_qos: None,
    };
}
