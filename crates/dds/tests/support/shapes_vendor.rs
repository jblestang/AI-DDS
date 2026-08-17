//! Shared live Shapes Demo test bodies for external DDS vendor peer applications.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::shapes_common::{
    output_contains_shapes_publish, output_contains_shapes_receive, shapes_available,
    shapes_base_domain, spawn_shapes_publisher, spawn_shapes_subscriber, ShapeType,
    ShapeTypeSupport, SHAPES_TOPICS, SHAPES_TYPE,
};
use super::interop_common::{vendor_display_name, wait_output, InteropVendor};
use dds::core::DomainParticipantFactory;
use dds::types::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, PublisherQos, ReliabilityKind,
    SubscriberQos, TopicQos,
};
use std::sync::Arc;
use std::time::Duration;

fn wait_until(timeout: Duration, mut predicate: impl FnMut() -> bool) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if predicate() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

pub fn skip_if_no_shapes(vendor: InteropVendor) {
    if !shapes_available(vendor) {
        eprintln!(
            "SKIP: {} shapes binaries not found. Rebuild peer apps with ShapeType.idl.",
            vendor_display_name(vendor)
        );
    }
}

pub fn vendor_publishes_shapes_aidds_receives(
    vendor: InteropVendor,
    topic: &str,
    domain_offset: u32,
) {
    skip_if_no_shapes(vendor);
    if !shapes_available(vendor) {
        return;
    }

    let domain = shapes_base_domain(vendor) + domain_offset;
    let shape = ShapeType::sample(topic, vendor.slug());
    let ts = Arc::new(ShapeTypeSupport);
    let participant =
        DomainParticipantFactory::create_participant(domain, DomainParticipantQos::default())
            .expect("participant");
    participant.register_type(SHAPES_TYPE, ts.clone()).unwrap();
    let topic_handle = participant
        .create_topic(topic, SHAPES_TYPE, TopicQos::default())
        .unwrap();

    let mut reader_qos = DataReaderQos::default();
    reader_qos.reliability.kind = ReliabilityKind::Reliable;
    let subscriber = participant.create_subscriber(SubscriberQos::default()).unwrap();
    let reader = subscriber
        .create_datareader(&topic_handle, reader_qos, ts.clone())
        .unwrap();
    let _rx = participant.spawn_receiver_loop();
    participant.run_matchmaking();

    std::thread::sleep(Duration::from_millis(800));

    let child = spawn_shapes_publisher(vendor, domain, topic, &shape).expect("spawn publisher");
    let output = wait_output(child, Duration::from_secs(25)).expect("publisher finished");
    assert!(
        output_contains_shapes_publish(&output, topic, &shape),
        "{} shapes publisher stdout: {}",
        vendor_display_name(vendor),
        String::from_utf8_lossy(&output.stdout)
    );

    participant.run_matchmaking();

    let mut received = None;
    assert!(
        wait_until(Duration::from_secs(10), || {
            participant.run_matchmaking();
            if let Ok(boxed) = reader.read_next() {
                if let Some(s) = boxed.downcast_ref::<ShapeType>() {
                    received = Some(s.clone());
                    return true;
                }
            }
            false
        }),
        "AI-DDS should receive {} on topic {} from {}",
        shape.color,
        topic,
        vendor_display_name(vendor)
    );

    assert_eq!(received.unwrap(), shape);
}

pub fn aidds_publishes_shapes_vendor_receives(
    vendor: InteropVendor,
    topic: &str,
    domain_offset: u32,
) {
    skip_if_no_shapes(vendor);
    if !shapes_available(vendor) {
        return;
    }

    let shape = ShapeType::sample(topic, "aidds");
    let domain = shapes_base_domain(vendor) + domain_offset;

    let ts = Arc::new(ShapeTypeSupport);
    let participant =
        DomainParticipantFactory::create_participant(domain, DomainParticipantQos::default())
            .expect("participant");
    participant.register_type(SHAPES_TYPE, ts.clone()).unwrap();
    let topic_handle = participant
        .create_topic(topic, SHAPES_TYPE, TopicQos::default())
        .unwrap();
    let _rx = participant.spawn_receiver_loop();

    let publisher = participant.create_publisher(PublisherQos::default()).unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.reliability.kind = ReliabilityKind::Reliable;
    let writer = publisher
        .create_datawriter(&topic_handle, writer_qos, ts.clone())
        .unwrap();
    participant.run_matchmaking();

    let sub_child =
        spawn_shapes_subscriber(vendor, domain, topic, Some(&shape.color)).expect("spawn subscriber");

    assert!(
        wait_until(Duration::from_secs(15), || {
            participant.run_matchmaking();
            participant
                .monitor_snapshot()
                .endpoints
                .iter()
                .any(|e| e.topic_name == topic && !e.is_writer)
        }),
        "AI-DDS should discover remote {} DataReader on topic {} before writing",
        vendor_display_name(vendor),
        topic
    );

    std::thread::sleep(Duration::from_millis(400));

    writer.register_instance(&shape).unwrap();
    writer.write(&shape).unwrap();
    participant.run_matchmaking();

    let output = wait_output(sub_child, Duration::from_secs(20)).expect("subscriber finished");
    assert!(
        output_contains_shapes_receive(&output, topic, &shape),
        "{} shapes subscriber stdout: {}\nstderr: {}",
        vendor_display_name(vendor),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn all_shapes_topics_vendor_to_aidds(vendor: InteropVendor, domain_offset: u32) {
    for (i, topic) in SHAPES_TOPICS.iter().enumerate() {
        vendor_publishes_shapes_aidds_receives(vendor, topic, domain_offset + i as u32);
    }
}

pub fn all_shapes_topics_aidds_to_vendor(vendor: InteropVendor, domain_offset: u32) {
    for (i, topic) in SHAPES_TOPICS.iter().enumerate() {
        aidds_publishes_shapes_vendor_receives(vendor, topic, domain_offset + i as u32);
    }
}
