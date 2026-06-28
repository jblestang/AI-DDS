//! Hello World pub/sub example using the `dds` façade.
//!
//! Demonstrates creating a DomainParticipant, registering a type support,
//! creating a Topic, Publisher, Subscriber, DataWriter, DataReader, and
//! round-tripping a message payload.
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
    reason = "DDS examples require print logging, panic unwraps, and simplified structural configurations for demonstration purposes."
)]
use dds::core::{DomainParticipantFactory, TypeSupport};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, PublisherQos, SubscriberQos, TopicQos,
};
use dds::types::return_code::{DdsError, DdsResult};
use std::any::Any;
use std::net::UdpSocket;
use std::sync::Arc;
use std::time::Duration;

// Define a structured HelloWorld payload
struct HelloWorld {
    id: u32,
    msg: String,
}

// Implement TypeSupport for HelloWorld to serialize/deserialize via CDR
struct HelloWorldTypeSupport;

impl TypeSupport for HelloWorldTypeSupport {
    fn get_type_name(&self) -> &str {
        "HelloWorld"
    }

    fn serialize(&self, value: &dyn Any) -> DdsResult<Vec<u8>> {
        if let Some(hw) = value.downcast_ref::<HelloWorld>() {
            let mut payload = Vec::new();
            payload.extend_from_slice(&hw.id.to_le_bytes());
            payload.extend_from_slice(hw.msg.as_bytes());
            Ok(payload)
        } else {
            Err(DdsError::BadParameter("cast failed".into()))
        }
    }

    fn deserialize(&self, bytes: &[u8]) -> DdsResult<Box<dyn Any>> {
        if bytes.len() < 4 {
            return Err(DdsError::Error("Payload too short".into()));
        }
        let mut id_bytes = [0_u8; 4];
        id_bytes.copy_from_slice(&bytes[0..4]);
        let id = u32::from_le_bytes(id_bytes);
        let msg =
            String::from_utf8(bytes[4..].to_vec()).map_err(|e| DdsError::Error(e.to_string()))?;
        Ok(Box::new(HelloWorld { id, msg }))
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
            println!("DDS HELLO WORLD PUBLISHER PROCESS");
            println!("==============================================================");

            let participant =
                DomainParticipantFactory::create_participant(0, DomainParticipantQos::default())?;
            let ts = Arc::new(HelloWorldTypeSupport);
            participant.register_type("HelloWorld", ts.clone())?;

            let topic = participant.create_topic("HelloTopic", "HelloWorld", TopicQos::default())?;
            let publisher = participant.create_publisher(PublisherQos::default())?;
            let writer = publisher.create_datawriter(&topic, DataWriterQos::default(), ts.clone())?;
            println!("[Publisher] Created Participant, Topic, Publisher, and DataWriter.");

            let sample = HelloWorld {
                id: 42,
                msg: "Hello from Antigravity DDS!".to_string(),
            };
            writer.write(&sample)?;
            println!("[Publisher] Wrote sample: id={}, msg='{}'", sample.id, sample.msg);

            // Serialize payload and send it over UDP to Subscriber process.
            let serialized = ts.serialize(&sample)?;
            let socket = UdpSocket::bind("127.0.0.1:0")?;
            socket.send_to(&serialized, "127.0.0.1:7925")?;
            println!("[Publisher] Sent serialized sample via UDP to port 7925.");
            println!("==============================================================");
        }
        Some("sub") => {
            println!("==============================================================");
            println!("DDS HELLO WORLD SUBSCRIBER PROCESS");
            println!("==============================================================");

            let participant =
                DomainParticipantFactory::create_participant(0, DomainParticipantQos::default())?;
            let ts = Arc::new(HelloWorldTypeSupport);
            participant.register_type("HelloWorld", ts.clone())?;

            let topic = participant.create_topic("HelloTopic", "HelloWorld", TopicQos::default())?;
            let subscriber = participant.create_subscriber(SubscriberQos::default())?;
            let reader = subscriber.create_datareader(&topic, DataReaderQos::default(), ts.clone())?;
            println!("[Subscriber] Created Participant, Topic, Subscriber, and DataReader.");

            // Bind to receive packet from Publisher process.
            let socket = UdpSocket::bind("127.0.0.1:7925")?;
            socket.set_read_timeout(Some(Duration::from_secs(30)))?;
            println!("[Subscriber] Listening on UDP port 7925...");

            let mut buf = [0_u8; 1024];
            let (len, _) = socket.recv_from(&mut buf)?;
            println!("[Subscriber] Received sample packet over UDP (len = {}).", len);

            reader.push_sample(dds::types::instance::InstanceHandle::NIL, buf[..len].to_vec());

            let received_boxed = reader.read_next()?;
            if let Some(received) = received_boxed.downcast_ref::<HelloWorld>() {
                println!(
                    "[Subscriber] Successfully received & decoded sample! id={}, msg='{}'",
                    received.id, received.msg
                );
                assert_eq!(received.id, 42);
                assert_eq!(received.msg, "Hello from Antigravity DDS!");
            } else {
                panic!("Received sample is not a HelloWorld struct!");
            }
            println!("==============================================================");
        }
        None => {
            println!("Starting DDS Hello World Publish/Subscribe loop (Single-process mode)...");
            println!("Tip: Run as two processes via:");
            println!("  cargo run --example hello_world sub");
            println!("  cargo run --example hello_world pub");
            println!("--------------------------------------------------------------");

            // 1. Create a DomainParticipant
            let participant =
                DomainParticipantFactory::create_participant(0, DomainParticipantQos::default())?;
            println!("Created DomainParticipant.");

            // 2. Register type support
            let ts = Arc::new(HelloWorldTypeSupport);
            participant.register_type("HelloWorld", ts.clone())?;
            println!("Registered type: HelloWorld");

            // 3. Create Topic
            let topic = participant.create_topic("HelloTopic", "HelloWorld", TopicQos::default())?;
            println!("Created Topic: HelloTopic");

            // 4. Create Publisher and DataWriter
            let publisher = participant.create_publisher(PublisherQos::default())?;
            let writer = publisher.create_datawriter(&topic, DataWriterQos::default(), ts.clone())?;
            println!("Created Publisher and DataWriter.");

            // 5. Create Subscriber and DataReader
            let subscriber = participant.create_subscriber(SubscriberQos::default())?;
            let reader = subscriber.create_datareader(&topic, DataReaderQos::default(), ts.clone())?;
            println!("Created Subscriber and DataReader.");

            // 6. Write a sample
            let sample = HelloWorld {
                id: 42,
                msg: "Hello from Antigravity DDS!".to_string(),
            };
            writer.write(&sample)?;
            println!("Wrote sample: id={}, msg='{}'", sample.id, sample.msg);

            // 7. Manually simulate delivery to the Reader's queue
            let serialized_bytes = ts.serialize(&sample)?;
            reader.push_sample(dds::types::instance::InstanceHandle::NIL, serialized_bytes);

            // 8. Read the sample
            let received_boxed = reader.read_next()?;
            if let Some(received) = received_boxed.downcast_ref::<HelloWorld>() {
                println!(
                    "Successfully received sample! id={}, msg='{}'",
                    received.id, received.msg
                );
                assert_eq!(received.id, 42);
                assert_eq!(received.msg, "Hello from Antigravity DDS!");
            } else {
                panic!("Received sample is not a HelloWorld struct!");
            }

            println!("DDS Pub/Sub Hello World execution succeeded!");
        }
        Some(other) => {
            println!("Unknown mode: {}. Expected 'pub' or 'sub' or no arguments.", other);
        }
    }

    Ok(())
}
}
