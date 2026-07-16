//! Shared live interop test bodies for external DDS vendor peer applications.

use super::{
    output_contains_interop_receive, spawn_vendor_publisher, spawn_vendor_subscriber,
    vendor_available, vendor_base_domain, vendor_display_name, wait_output, InteropMessage,
    InteropTypeSupport, InteropVendor, INTEROP_TOPIC, INTEROP_TYPE,
};

fn wait_until(timeout: std::time::Duration, mut predicate: impl FnMut() -> bool) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if predicate() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    false
}
use dds::core::DomainParticipantFactory;
use dds::types::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, PublisherQos, ReliabilityKind,
    SubscriberQos, TopicQos,
};
use std::sync::Arc;
use std::time::Duration;

pub fn skip_if_no_vendor(vendor: InteropVendor) {
    if !vendor_available(vendor) {
        eprintln!(
            "SKIP: {} interop binaries not found. Run {} with the vendor prefix env set.",
            vendor_display_name(vendor),
            vendor.build_script()
        );
    }
}

pub fn vendor_publishes_aidds_receives(vendor: InteropVendor) {
    skip_if_no_vendor(vendor);
    if !vendor_available(vendor) {
        return;
    }

    let domain = vendor_base_domain(vendor);
    let ts = Arc::new(InteropTypeSupport);
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
    let payload = format!("from-{}", vendor.slug());
    let child =
        spawn_vendor_publisher(vendor, domain, sample_id, &payload).expect("spawn publisher");
    let output = wait_output(child, Duration::from_secs(25)).expect("publisher finished");

    assert!(
        String::from_utf8_lossy(&output.stdout).contains("INTEROP_PUBLISH"),
        "{} publisher stdout: {}",
        vendor_display_name(vendor),
        String::from_utf8_lossy(&output.stdout)
    );

    let mut received = None;
    assert!(
        wait_until(Duration::from_secs(10), || {
            participant.run_matchmaking();
            if let Ok(boxed) = reader.read_next() {
                if let Some(msg) = boxed.downcast_ref::<InteropMessage>() {
                    received = Some(msg.clone());
                    return true;
                }
            }
            false
        }),
        "AI-DDS should receive sample published by {}",
        vendor_display_name(vendor)
    );

    let msg = received.unwrap();
    assert_eq!(msg.id, sample_id);
    assert_eq!(msg.payload, payload);
}

pub fn aidds_publishes_vendor_receives(vendor: InteropVendor) {
    skip_if_no_vendor(vendor);
    if !vendor_available(vendor) {
        return;
    }

    let sample_id = 9002u32;
    let payload = format!("from-aidds-{}", vendor.slug());
    let domain = vendor_base_domain(vendor) + 1;

    let ts = Arc::new(InteropTypeSupport);
    let participant =
        DomainParticipantFactory::create_participant(domain, DomainParticipantQos::default())
            .expect("participant");
    participant.register_type(INTEROP_TYPE, ts.clone()).unwrap();
    let topic = participant
        .create_topic(INTEROP_TOPIC, INTEROP_TYPE, TopicQos::default())
        .unwrap();
    let _rx = participant.spawn_receiver_loop();

    let publisher = participant.create_publisher(PublisherQos::default()).unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&topic, writer_qos, ts.clone())
        .unwrap();
    participant.run_matchmaking();

    // Start vendor subscriber after local writer exists so late-joining SEDP from the
    // vendor includes its DataReader while our participant is already discoverable.
    let sub_child = spawn_vendor_subscriber(vendor, domain, Some(sample_id)).expect("spawn subscriber");

    assert!(
        wait_until(Duration::from_secs(15), || {
            participant.run_matchmaking();
            participant
                .monitor_snapshot()
                .endpoints
                .iter()
                .any(|e| e.topic_name == INTEROP_TOPIC && !e.is_writer)
        }),
        "AI-DDS should discover remote {} DataReader via SEDP before writing",
        vendor_display_name(vendor)
    );

    std::thread::sleep(Duration::from_millis(400));

    writer
        .write(&InteropMessage {
            id: sample_id,
            payload: payload.clone(),
        })
        .unwrap();

    participant.run_matchmaking();

    let output = wait_output(sub_child, Duration::from_secs(20)).expect("subscriber finished");
    assert!(
        output_contains_interop_receive(&output, sample_id, &payload),
        "{} subscriber stdout: {}\nstderr: {}",
        vendor_display_name(vendor),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn bidirectional_discovery_matchmaking(vendor: InteropVendor) {
    skip_if_no_vendor(vendor);
    if !vendor_available(vendor) {
        return;
    }

    let sample_id = 9003u32;
    let payload = format!("discovery-{}", vendor.slug());
    let domain = vendor_base_domain(vendor) + 2;

    let ts = Arc::new(InteropTypeSupport);
    let participant =
        DomainParticipantFactory::create_participant(domain, DomainParticipantQos::default())
            .expect("participant");
    participant.register_type(INTEROP_TYPE, ts.clone()).unwrap();
    let topic = participant
        .create_topic(INTEROP_TOPIC, INTEROP_TYPE, TopicQos::default())
        .unwrap();
    let _rx = participant.spawn_receiver_loop();

    let publisher = participant.create_publisher(PublisherQos::default()).unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&topic, writer_qos, ts.clone())
        .unwrap();
    participant.run_matchmaking();

    let sub_child = spawn_vendor_subscriber(vendor, domain, Some(sample_id)).expect("spawn subscriber");

    assert!(
        wait_until(Duration::from_secs(15), || {
            participant.run_matchmaking();
            participant
                .monitor_snapshot()
                .endpoints
                .iter()
                .any(|e| e.topic_name == INTEROP_TOPIC && !e.is_writer)
        }),
        "AI-DDS should discover remote {} DataReader via SEDP before writing",
        vendor_display_name(vendor)
    );

    std::thread::sleep(Duration::from_millis(400));

    writer
        .write(&InteropMessage {
            id: sample_id,
            payload: payload.clone(),
        })
        .unwrap();

    participant.run_matchmaking();

    let output = wait_output(sub_child, Duration::from_secs(20)).expect("subscriber finished");
    assert!(output_contains_interop_receive(&output, sample_id, &payload));

    let snap = participant.monitor_snapshot();
    assert!(
        snap.endpoints
            .iter()
            .any(|e| e.topic_name == INTEROP_TOPIC && e.is_writer),
        "Monitor snapshot should list local interop writer"
    );
}
