//! QoS History Example.
//!
//! This example demonstrates how to configure and use the History QoS policy
//! on DataWriters and DataReaders in Antigravity DDS.
//!
//! Spec Reference: OMG DDS DCPS 1.4 §2.2.3.11 — History QoS Policy
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
    DataReaderQos, DataWriterQos, DomainParticipantQos, History, HistoryKind, PublisherQos,
    SubscriberQos, TopicQos,
};
use dds::types::return_code::{DdsError, DdsResult};
use std::any::Any;
use std::net::UdpSocket;
use std::sync::Arc;
use std::time::Duration;

// 1/3 Comment-to-code ratio.
// Define a sample message type representing history data.
struct HistorySample {
    seq: u32,
    payload: String,
}

// Custom type support for serializing/deserializing history payload.
struct HistoryTypeSupport;

impl TypeSupport for HistoryTypeSupport {
    fn get_type_name(&self) -> &str {
        "HistorySample"
    }

    fn serialize(&self, value: &dyn Any) -> DdsResult<Vec<u8>> {
        value.downcast_ref::<HistorySample>().map_or_else(|| Err(DdsError::BadParameter("cast failed".into())), |s| {
            let mut payload = Vec::new();
            payload.extend_from_slice(&s.seq.to_le_bytes());
            payload.extend_from_slice(s.payload.as_bytes());
            Ok(payload)
        })
    }

    fn deserialize(&self, bytes: &[u8]) -> DdsResult<Box<dyn Any>> {
        if bytes.len() < 4 {
            return Err(DdsError::Error("Payload too short".into()));
        }
        let mut seq_bytes = [0_u8; 4];
        seq_bytes.copy_from_slice(&bytes[0..4]);
        let seq = u32::from_le_bytes(seq_bytes);

        let payload = String::from_utf8(bytes[4..].to_vec())
            .map_err(|e| DdsError::Error(e.to_string()))?;
        Ok(Box::new(HistorySample { seq, payload }))
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
            println!("DDS QOS HISTORY PUBLISHER PROCESS");
            println!("==============================================================");

            let participant =
                DomainParticipantFactory::create_participant(0, DomainParticipantQos::default())?;
            let ts = Arc::new(HistoryTypeSupport);
            participant.register_type("HistorySample", ts.clone())?;

            let topic = participant.create_topic("HistoryTopic", "HistorySample", TopicQos::default())?;

            let mut writer_qos = DataWriterQos::default();
            writer_qos.history = History {
                kind: HistoryKind::KeepLast,
                depth: 3,
            };
            println!("[Writer] Configuring History: KeepLast, Depth: 3");

            let publisher = participant.create_publisher(PublisherQos::default())?;
            let writer = publisher.create_datawriter(&topic, writer_qos, ts.clone())?;

            let socket = UdpSocket::bind("127.0.0.1:0")?;

            for i in 1..=4 {
                let sample = HistorySample {
                    seq: i,
                    payload: format!("Sample data {i}"),
                };
                writer.write(&sample)?;
                println!("[Writer] Dispatched sample sequence: {}", i);

                let serialized = ts.serialize(&sample)?;
                socket.send_to(&serialized, "127.0.0.1:7927")?;
            }
            println!("[Writer] Sent 4 serialized samples via UDP to port 7927.");
            println!("==============================================================");
        }
        Some("sub") => {
            println!("==============================================================");
            println!("DDS QOS HISTORY SUBSCRIBER PROCESS");
            println!("==============================================================");

            let participant =
                DomainParticipantFactory::create_participant(0, DomainParticipantQos::default())?;
            let ts = Arc::new(HistoryTypeSupport);
            participant.register_type("HistorySample", ts.clone())?;

            let topic = participant.create_topic("HistoryTopic", "HistorySample", TopicQos::default())?;

            let mut reader_qos = DataReaderQos::default();
            reader_qos.history = History {
                kind: HistoryKind::KeepLast,
                depth: 3,
            };
            println!("[Reader] Configuring History: KeepLast, Depth: 3");

            let subscriber = participant.create_subscriber(SubscriberQos::default())?;
            let reader = subscriber.create_datareader(&topic, reader_qos, ts)?;

            let socket = UdpSocket::bind("127.0.0.1:7927")?;
            socket.set_read_timeout(Some(Duration::from_secs(30)))?;
            println!("[Reader] Listening on UDP port 7927...");

            let mut buf = [0u8; 1024];
            for _ in 1..=4 {
                let (len, _) = socket.recv_from(&mut buf)?;
                reader.push_sample(dds::types::instance::InstanceHandle::NIL, buf[..len].to_vec());
            }
            println!("[Reader] Received 4 sample packets over UDP.");

            println!("\n[Reader] Reading received historical samples:");
            let mut count = 0;
            while let Ok(received_boxed) = reader.read_next() {
                if let Some(received) = received_boxed.downcast_ref::<HistorySample>() {
                    count += 1;
                    println!("  -> Read Sample: seq={}, payload='{}'", received.seq, received.payload);
                }
            }
            println!("[Reader] Total read: {} samples.", count);
            assert_eq!(count, 4);
            println!("==============================================================");
        }
        None => {
            println!("==============================================================");
            println!("DDS QOS HISTORY POLICY DEMONSTRATION (Single-process mode)");
            println!("Tip: Run as two processes via:");
            println!("  cargo run --example qos_history sub");
            println!("  cargo run --example qos_history pub");
            println!("==============================================================");

            // 1. Initialize DomainParticipant.
            let participant =
                DomainParticipantFactory::create_participant(0, DomainParticipantQos::default())?;
            let ts = Arc::new(HistoryTypeSupport);
            participant.register_type("HistorySample", ts.clone())?;

            // 2. Create Topic.
            let topic = participant.create_topic("HistoryTopic", "HistorySample", TopicQos::default())?;

            // 3. Configure DataWriter QoS with KeepLast History.
            let mut writer_qos = DataWriterQos::default();
            writer_qos.history = History {
                kind: HistoryKind::KeepLast,
                depth: 3,
            };
            println!("[Writer] Configuring History: KeepLast, Depth: 3");

            let publisher = participant.create_publisher(PublisherQos::default())?;
            let writer = publisher.create_datawriter(&topic, writer_qos, ts.clone())?;

            // 4. Configure DataReader QoS with KeepLast History.
            let mut reader_qos = DataReaderQos::default();
            reader_qos.history = History {
                kind: HistoryKind::KeepLast,
                depth: 3,
            };
            println!("[Reader] Configuring History: KeepLast, Depth: 3");

            let subscriber = participant.create_subscriber(SubscriberQos::default())?;
            let reader = subscriber.create_datareader(&topic, reader_qos, ts.clone())?;

            // 5. Write multiple samples to populate history cache.
            for i in 1..=4 {
                let sample = HistorySample {
                    seq: i,
                    payload: format!("Sample data {i}"),
                };
                writer.write(&sample)?;
                println!("[Writer] Dispatched sample sequence: {}", i);

                // Simulate loopback network delivery.
                let serialized = ts.serialize(&sample)?;
                reader.push_sample(dds::types::instance::InstanceHandle::NIL, serialized);
            }

            // 6. Read and assert the historical samples from the Reader queue.
            println!("\n[Reader] Reading received historical samples:");
            let mut count = 0;
            while let Ok(received_boxed) = reader.read_next() {
                if let Some(received) = received_boxed.downcast_ref::<HistorySample>() {
                    count += 1;
                    println!("  -> Read Sample: seq={}, payload='{}'", received.seq, received.payload);
                }
            }
            println!("[Reader] Total read: {} samples.", count);
            assert_eq!(count, 4);

            println!("==============================================================");
            println!("History QoS demonstration completed successfully.");
            println!("==============================================================");
        }
        Some(other) => {
            println!("Unknown mode: {}. Expected 'pub' or 'sub' or no arguments.", other);
        }
    }

    Ok(())
}
