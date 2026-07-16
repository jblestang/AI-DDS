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

use common::wait_until;
use interop_common::{
    cyclonedds_available, output_contains_interop_receive, spawn_cyclonedds_publisher,
    wait_output, InteropMessage, InteropTypeSupport, INTEROP_DOMAIN,
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
    let domain = INTEROP_DOMAIN;
    let participant =
        DomainParticipantFactory::create_participant(domain, DomainParticipantQos::default())
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
    participant.run_matchmaking();

    std::thread::sleep(Duration::from_millis(800));

    let sample_id = 9001u32;
    let payload = "from-cyclonedds";
    let child = spawn_cyclonedds_publisher(domain, sample_id, payload).expect("spawn publisher");
    let output = wait_output(child, Duration::from_secs(15)).expect("publisher finished");

    assert!(
        String::from_utf8_lossy(&output.stdout).contains("INTEROP_PUBLISH"),
        "CycloneDDS publisher stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let mut received = None;
    assert!(
        wait_until(Duration::from_secs(20), || {
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
    let domain = INTEROP_DOMAIN + 1;

    let ts = Arc::new(InteropTypeSupport);
    let participant =
        DomainParticipantFactory::create_participant(domain, DomainParticipantQos::default())
            .expect("participant");
    participant.register_type(INTEROP_TYPE, ts.clone()).unwrap();
    let topic = participant
        .create_topic(INTEROP_TOPIC, INTEROP_TYPE, TopicQos::default())
        .unwrap();
    let _rx = participant.spawn_receiver_loop();

    let bin = interop_common::cyclonedds_subscriber().unwrap();
    let sub_child = std::process::Command::new(bin)
        .arg(sample_id.to_string())
        .env("AIDDS_INTEROP_DOMAIN", domain.to_string())
        .env("AIDDS_INTEROP_TIMEOUT_MS", "12000")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn subscriber");

    std::thread::sleep(Duration::from_millis(400));

    let publisher = participant.create_publisher(PublisherQos::default()).unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&topic, writer_qos, ts.clone())
        .unwrap();
    participant.run_matchmaking();

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

    let sample_id = 9003u32;
    let payload = "discovery-interop";
    let domain = INTEROP_DOMAIN + 2;

    let ts = Arc::new(InteropTypeSupport);
    let participant =
        DomainParticipantFactory::create_participant(domain, DomainParticipantQos::default())
            .expect("participant");
    participant.register_type(INTEROP_TYPE, ts.clone()).unwrap();
    let topic = participant
        .create_topic(INTEROP_TOPIC, INTEROP_TYPE, TopicQos::default())
        .unwrap();
    let _rx = participant.spawn_receiver_loop();

    let bin = interop_common::cyclonedds_subscriber().unwrap();
    let sub_child = std::process::Command::new(bin)
        .arg(sample_id.to_string())
        .env("AIDDS_INTEROP_DOMAIN", domain.to_string())
        .env("AIDDS_INTEROP_TIMEOUT_MS", "12000")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn subscriber");

    std::thread::sleep(Duration::from_millis(400));

    let publisher = participant.create_publisher(PublisherQos::default()).unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&topic, writer_qos, ts.clone())
        .unwrap();
    participant.run_matchmaking();

    std::thread::sleep(Duration::from_millis(800));

    writer
        .write(&InteropMessage {
            id: sample_id,
            payload: payload.to_string(),
        })
        .unwrap();

    let output = wait_output(sub_child, Duration::from_secs(15)).expect("subscriber finished");
    assert!(output_contains_interop_receive(&output, sample_id, payload));

    let snap = participant.monitor_snapshot();
    assert!(
        snap.endpoints.iter().any(|e| e.topic_name == INTEROP_TOPIC && e.is_writer),
        "Monitor snapshot should list local interop writer"
    );
}
