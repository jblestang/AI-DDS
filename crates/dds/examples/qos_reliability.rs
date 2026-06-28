//! QoS Reliability Example.
//!
//! This example demonstrates how to configure and use the Reliability QoS policy
//! on DataWriters and DataReaders in Antigravity DDS.
//!
//! Spec Reference: OMG DDS DCPS 1.4 §2.2.3.9 — Reliability QoS Policy
#![forbid(unsafe_code)]
#![warn(
    rust_2018_idioms,
    nonstandard_style,
    future_incompatible,
    clippy::all,
    clippy::restriction,
    clippy::pedantic,
    clippy::nursery
)]
#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::implicit_return,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::string_slice,
    clippy::absolute_paths,
    clippy::missing_docs_in_private_items,
    clippy::std_instead_of_core,
    clippy::std_instead_of_alloc,
    clippy::alloc_instead_of_core,
    clippy::single_call_fn,
    clippy::default_numeric_fallback,
    clippy::shadow_reuse,
    clippy::question_mark_used,
    clippy::use_debug,
    clippy::unnecessary_debug_formatting,
    clippy::missing_assert_message,
    clippy::uninlined_format_args,
    clippy::integer_division,
    clippy::integer_division_remainder_used,
    clippy::cast_possible_truncation,
    clippy::as_conversions,
    clippy::too_many_lines,
    clippy::panic,
    clippy::field_reassign_with_default,
    clippy::clone_on_ref_ptr,
    clippy::arithmetic_side_effects,
    clippy::panic_in_result_fn,
    clippy::string_add,
    clippy::str_to_string,
    clippy::doc_markdown,
    clippy::arbitrary_source_item_ordering,
    clippy::unnecessary_literal_bound,
    clippy::option_if_let_else,
    clippy::min_ident_chars,
    clippy::little_endian_bytes,
    clippy::unused_trait_names,
    clippy::separated_literal_suffix,
    clippy::unseparated_literal_suffix,
    clippy::missing_asserts_for_indexing,
    reason = "DDS examples require print logging, panic unwraps, and simplified structural configurations for demonstration purposes."
)]
use dds::core::{DomainParticipantFactory, TypeSupport};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, PublisherQos, Reliability, ReliabilityKind,
    SubscriberQos, TopicQos,
};
use dds::types::return_code::{DdsError, DdsResult};
use dds::types::time::Duration;
use std::any::Any;
use std::net::UdpSocket;
use std::sync::Arc;
use std::time::Duration as StdDuration;

// 1/3 Comment-to-code ratio.
// Define a sample message type representing high-reliability telemetry.
struct ReliableTelemetry {
    id: u32,
    value: f64,
}

// Custom type support for serializing/deserializing telemetry payload.
struct TelemetryTypeSupport;

impl TypeSupport for TelemetryTypeSupport {
    fn get_type_name(&self) -> &str {
        "ReliableTelemetry"
    }

    fn serialize(&self, value: &dyn Any) -> DdsResult<Vec<u8>> {
        if let Some(t) = value.downcast_ref::<ReliableTelemetry>() {
            let mut payload = Vec::new();
            payload.extend_from_slice(&t.id.to_le_bytes());
            payload.extend_from_slice(&t.value.to_le_bytes());
            Ok(payload)
        } else {
            Err(DdsError::BadParameter("cast failed".into()))
        }
    }

    fn deserialize(&self, bytes: &[u8]) -> DdsResult<Box<dyn Any>> {
        if bytes.len() < 12 {
            return Err(DdsError::Error("Payload too short".into()));
        }
        let mut id_bytes = [0_u8; 4];
        id_bytes.copy_from_slice(&bytes[0..4]);
        let id = u32::from_le_bytes(id_bytes);

        let mut val_bytes = [0_u8; 8];
        val_bytes.copy_from_slice(&bytes[4..12]);
        let value = f64::from_le_bytes(val_bytes);

        Ok(Box::new(ReliableTelemetry { id, value }))
    }

    fn get_key_hash(&self, _value: &dyn Any) -> DdsResult<dds::types::instance::InstanceHandle> {
        Ok(dds::types::instance::InstanceHandle::NIL)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str);

    match mode {
        Some("pub") => {
            println!("==============================================================");
            println!("DDS QOS RELIABILITY PUBLISHER PROCESS");
            println!("==============================================================");

            let participant =
                DomainParticipantFactory::create_participant(0, DomainParticipantQos::default())?;
            let ts = Arc::new(TelemetryTypeSupport);
            participant.register_type("ReliableTelemetry", ts.clone())?;

            let topic =
                participant.create_topic("TelemetryTopic", "ReliableTelemetry", TopicQos::default())?;

            let mut writer_qos = DataWriterQos::default();
            writer_qos.reliability = Reliability {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: Duration::from_millis(500),
            };
            println!("[Writer] Configuring Offered Reliability: Reliable");

            let publisher = participant.create_publisher(PublisherQos::default())?;
            let writer = publisher.create_datawriter(&topic, writer_qos, ts.clone())?;

            let sample = ReliableTelemetry {
                id: 101,
                value: 98.6,
            };
            writer.write(&sample)?;
            println!("[Writer] Dispatched reliable sample ID: {}", sample.id);

            // Send serialized sample via UDP to subscriber.
            let serialized = ts.serialize(&sample)?;
            let socket = UdpSocket::bind("127.0.0.1:0")?;
            socket.send_to(&serialized, "127.0.0.1:7926")?;
            println!("[Writer] Sent serialized sample via UDP to port 7926.");
            println!("==============================================================");
        }
        Some("sub") => {
            println!("==============================================================");
            println!("DDS QOS RELIABILITY SUBSCRIBER PROCESS");
            println!("==============================================================");

            let participant =
                DomainParticipantFactory::create_participant(0, DomainParticipantQos::default())?;
            let ts = Arc::new(TelemetryTypeSupport);
            participant.register_type("ReliableTelemetry", ts.clone())?;

            let topic =
                participant.create_topic("TelemetryTopic", "ReliableTelemetry", TopicQos::default())?;

            let mut reader_qos = DataReaderQos::default();
            reader_qos.reliability = Reliability {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: Duration::from_millis(500),
            };
            println!("[Reader] Configuring Requested Reliability: Reliable");

            let subscriber = participant.create_subscriber(SubscriberQos::default())?;
            let reader = subscriber.create_datareader(&topic, reader_qos, ts.clone())?;

            // Bind UDP to receive package.
            let socket = UdpSocket::bind("127.0.0.1:7926")?;
            socket.set_read_timeout(Some(StdDuration::from_secs(30)))?;
            println!("[Reader] Listening on UDP port 7926...");

            let mut buf = [0_u8; 1024];
            let (len, _) = socket.recv_from(&mut buf)?;
            println!("[Reader] Received sample packet over UDP (len = {}).", len);

            reader.push_sample(dds::types::instance::InstanceHandle::NIL, buf[..len].to_vec());

            let received_boxed = reader.read_next()?;
            if let Some(received) = received_boxed.downcast_ref::<ReliableTelemetry>() {
                println!("[Reader] Received sample successfully!");
                println!("  -> ID: {}", received.id);
                println!("  -> Value: {}", received.value);
                assert_eq!(received.id, 101);
            } else {
                panic!("Failed to downcast received sample!");
            }
            println!("==============================================================");
        }
        None => {
            println!("==============================================================");
            println!("DDS QOS RELIABILITY POLICY DEMONSTRATION (Single-process mode)");
            println!("Tip: Run as two processes via:");
            println!("  cargo run --example qos_reliability sub");
            println!("  cargo run --example qos_reliability pub");
            println!("==============================================================");

            // 1. Initialize DomainParticipant.
            let participant =
                DomainParticipantFactory::create_participant(0, DomainParticipantQos::default())?;
            let ts = Arc::new(TelemetryTypeSupport);
            participant.register_type("ReliableTelemetry", ts.clone())?;

            // 2. Create Topic.
            let topic =
                participant.create_topic("TelemetryTopic", "ReliableTelemetry", TopicQos::default())?;

            // 3. Configure Reliable DataWriter QoS.
            let mut writer_qos = DataWriterQos::default();
            writer_qos.reliability = Reliability {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: Duration::from_millis(500),
            };
            println!("[Writer] Configuring Offered Reliability: Reliable");

            let publisher = participant.create_publisher(PublisherQos::default())?;
            let writer = publisher.create_datawriter(&topic, writer_qos, ts.clone())?;

            // 4. Configure Reliable DataReader QoS.
            let mut reader_qos = DataReaderQos::default();
            reader_qos.reliability = Reliability {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: Duration::from_millis(500),
            };
            println!("[Reader] Configuring Requested Reliability: Reliable");

            let subscriber = participant.create_subscriber(SubscriberQos::default())?;
            let reader = subscriber.create_datareader(&topic, reader_qos, ts.clone())?;

            // 5. Write and transmit sample.
            let sample = ReliableTelemetry {
                id: 101,
                value: 98.6,
            };
            writer.write(&sample)?;
            println!("[Writer] Dispatched reliable sample ID: {}", sample.id);

            // Simulate loopback network delivery.
            let serialized = ts.serialize(&sample)?;
            reader.push_sample(dds::types::instance::InstanceHandle::NIL, serialized);

            // 6. Read and assert the reliable sample.
            let received_boxed = reader.read_next()?;
            if let Some(received) = received_boxed.downcast_ref::<ReliableTelemetry>() {
                println!("[Reader] Received sample successfully!");
                println!("  -> ID: {}", received.id);
                println!("  -> Value: {}", received.value);
                assert_eq!(received.id, 101);
            } else {
                panic!("Failed to downcast received sample!");
            }

            println!("==============================================================");
            println!("Reliability QoS demonstration completed successfully.");
            println!("==============================================================");
        }
        Some(other) => {
            println!("Unknown mode: {}. Expected 'pub' or 'sub' or no arguments.", other);
        }
    }

    Ok(())
}
// stray duplicate removed
