use dds::cdr::{CdrDeserialize, CdrSerialize, CdrDeserializer, CdrSerializer, CdrResult};
use dds::core::{DomainParticipantFactory, TypeSupport};
use dds::types::qos::{
    DomainParticipantQos, TopicQos, PublisherQos, SubscriberQos, DataWriterQos, DataReaderQos,
};
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;


#[derive(Debug, Clone, PartialEq, Eq)]
struct LoopbackMessage {
    content: String,
}

impl CdrSerialize for LoopbackMessage {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_str(&self.content);
        Ok(())
    }
}

impl CdrDeserialize for LoopbackMessage {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        let content = deserializer.deserialize_str()?;
        Ok(Self { content })
    }
}

struct LoopbackMessageTypeSupport;

impl TypeSupport for LoopbackMessageTypeSupport {
    fn get_type_name(&self) -> &str {
        "LoopbackMessage"
    }

    fn serialize(&self, value: &dyn Any) -> dds::types::return_code::DdsResult<Vec<u8>> {
        if let Some(msg) = value.downcast_ref::<LoopbackMessage>() {
            let bytes = dds::cdr::serialize_to_bytes(msg, dds::cdr::Endianness::LittleEndian)
                .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
            Ok(bytes.to_vec())
        } else {
            Err(dds::types::return_code::DdsError::BadParameter("cast failed".into()))
        }
    }

    fn deserialize(&self, bytes: &[u8]) -> dds::types::return_code::DdsResult<Box<dyn Any>> {
        let msg: LoopbackMessage = dds::cdr::deserialize_from_slice(bytes, dds::cdr::Endianness::LittleEndian)
            .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
        Ok(Box::new(msg))
    }

    fn get_key_hash(
        &self,
        _value: &dyn Any,
    ) -> dds::types::return_code::DdsResult<dds::types::instance::InstanceHandle> {
        Ok(dds::types::instance::InstanceHandle::NIL)
    }
}

#[test]
fn test_wire_loopback() {
    // 1. Create two DomainParticipants (use domain ID 15 to avoid conflicts)
    let participant_pub = DomainParticipantFactory::create_participant(15, DomainParticipantQos::default())
        .expect("Failed to create pub participant");
    let participant_sub = DomainParticipantFactory::create_participant(15, DomainParticipantQos::default())
        .expect("Failed to create sub participant");

    // 2. Register types
    let ts_pub = Arc::new(LoopbackMessageTypeSupport);
    let ts_sub = Arc::new(LoopbackMessageTypeSupport);
    participant_pub.register_type("LoopbackMessage", ts_pub.clone()).unwrap();
    participant_sub.register_type("LoopbackMessage", ts_sub.clone()).unwrap();

    // 3. Create Topic
    let topic_pub = participant_pub
        .create_topic("LoopbackTopic", "LoopbackMessage", TopicQos::default())
        .unwrap();
    let topic_sub = participant_sub
        .create_topic("LoopbackTopic", "LoopbackMessage", TopicQos::default())
        .unwrap();

    // 4. Create Publisher & DataWriter
    let publisher = participant_pub.create_publisher(PublisherQos::default()).unwrap();
    let writer = publisher
        .create_datawriter(&topic_pub, DataWriterQos::default(), ts_pub)
        .unwrap();

    // 5. Create Subscriber & DataReader
    let subscriber = participant_sub.create_subscriber(SubscriberQos::default()).unwrap();
    let reader = subscriber
        .create_datareader(&topic_sub, DataReaderQos::default(), ts_sub)
        .unwrap();

    // 6. Match writer and reader manually to skip discovery for loopback test
    let reader_port = subscriber.unicast_port();
    let reader_locator = dds::types::locator::Locator::udpv4(
        std::net::Ipv4Addr::new(127, 0, 0, 1),
        reader_port,
    );
    let reader_guid = reader.guid();
    publisher.add_reader_proxy_to_all(reader_guid, reader_locator, "LoopbackTopic", &dds_types::qos::Partition::default());

    // 7. Start the receiver loop on the subscriber's participant
    let _receiver_handle = participant_sub.spawn_receiver_loop();

    // Give the receiver loop thread a tiny moment to spin up and bind
    std::thread::sleep(Duration::from_millis(50));

    // 8. Write message
    let msg = LoopbackMessage {
        content: "DDS Wire Loopback Success!".to_string(),
    };
    writer.write(&msg).expect("Failed to write loopback sample");

    // 9. Read back with 2 second timeout
    let start = std::time::Instant::now();
    let mut received_msg: Option<LoopbackMessage> = None;
    while start.elapsed() < Duration::from_secs(2) {
        if let Ok(boxed) = reader.read_next() {
            if let Some(m) = boxed.downcast_ref::<LoopbackMessage>() {
                received_msg = Some(m.clone());
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(received_msg.is_some(), "Timeout waiting for loopback sample");
    assert_eq!(received_msg.unwrap().content, "DDS Wire Loopback Success!");
}

#[test]
fn test_wire_loopback_fragmentation() {
    // 1. Create two DomainParticipants (use domain ID 16)
    let participant_pub = DomainParticipantFactory::create_participant(16, DomainParticipantQos::default())
        .expect("Failed to create pub participant");
    let participant_sub = DomainParticipantFactory::create_participant(16, DomainParticipantQos::default())
        .expect("Failed to create sub participant");

    // 2. Register types
    let ts_pub = Arc::new(LoopbackMessageTypeSupport);
    let ts_sub = Arc::new(LoopbackMessageTypeSupport);
    participant_pub.register_type("LoopbackMessage", ts_pub.clone()).unwrap();
    participant_sub.register_type("LoopbackMessage", ts_sub.clone()).unwrap();

    // 3. Create Topic
    let topic_pub = participant_pub
        .create_topic("FragTopic", "LoopbackMessage", TopicQos::default())
        .unwrap();
    let topic_sub = participant_sub
        .create_topic("FragTopic", "LoopbackMessage", TopicQos::default())
        .unwrap();

    // 4. Create Publisher & DataWriter
    let publisher = participant_pub.create_publisher(PublisherQos::default()).unwrap();
    let writer = publisher
        .create_datawriter(&topic_pub, DataWriterQos::default(), ts_pub)
        .unwrap();

    // 5. Create Subscriber & DataReader
    let subscriber = participant_sub.create_subscriber(SubscriberQos::default()).unwrap();
    let reader = subscriber
        .create_datareader(&topic_sub, DataReaderQos::default(), ts_sub)
        .unwrap();

    // 6. Match writer and reader manually
    let reader_port = subscriber.unicast_port();
    let reader_locator = dds::types::locator::Locator::udpv4(
        std::net::Ipv4Addr::new(127, 0, 0, 1),
        reader_port,
    );
    let reader_guid = reader.guid();
    publisher.add_reader_proxy_to_all(reader_guid, reader_locator, "FragTopic", &dds_types::qos::Partition::default());

    // 7. Start the receiver loop on the subscriber's participant
    let _receiver_handle = participant_sub.spawn_receiver_loop();

    // Give the receiver loop thread a tiny moment to spin up and bind
    std::thread::sleep(Duration::from_millis(50));

    // 8. Write large message (exceeding 1000 byte fragmentation threshold, e.g. 5000 bytes)
    let large_payload = "A".repeat(5000);
    let msg = LoopbackMessage {
        content: large_payload.clone(),
    };
    writer.write(&msg).expect("Failed to write large sample");

    // 9. Read back with 2 second timeout
    let start = std::time::Instant::now();
    let mut received_msg: Option<LoopbackMessage> = None;
    while start.elapsed() < Duration::from_secs(2) {
        if let Ok(boxed) = reader.read_next() {
            if let Some(m) = boxed.downcast_ref::<LoopbackMessage>() {
                received_msg = Some(m.clone());
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(received_msg.is_some(), "Timeout waiting for fragmented sample");
    assert_eq!(received_msg.unwrap().content, large_payload);
}
