//! P1 compliance integration tests: read/take, reverse matchmaking, partition.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dds::cdr::{CdrDeserialize, CdrSerialize, CdrDeserializer, CdrSerializer, CdrResult};
use dds::core::{
    check_partition_compatibility, DomainParticipantFactory, TypeSupport,
};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, Partition, PublisherQos, SubscriberQos,
    TopicQos,
};
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
struct P1Message {
    content: String,
}

impl CdrSerialize for P1Message {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_str(&self.content);
        Ok(())
    }
}

impl CdrDeserialize for P1Message {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        Ok(Self {
            content: deserializer.deserialize_str()?,
        })
    }
}

struct P1TypeSupport;

impl TypeSupport for P1TypeSupport {
    fn get_type_name(&self) -> &str {
        "P1Message"
    }

    fn serialize(&self, value: &dyn Any) -> dds::types::return_code::DdsResult<Vec<u8>> {
        if let Some(msg) = value.downcast_ref::<P1Message>() {
            let bytes = dds::cdr::serialize_to_bytes(msg, dds::cdr::Endianness::LittleEndian)
                .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
            Ok(bytes.to_vec())
        } else {
            Err(dds::types::return_code::DdsError::BadParameter("cast failed".into()))
        }
    }

    fn deserialize(&self, bytes: &[u8]) -> dds::types::return_code::DdsResult<Box<dyn Any>> {
        let msg: P1Message =
            dds::cdr::deserialize_from_slice(bytes, dds::cdr::Endianness::LittleEndian)
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
fn test_read_vs_take_semantics() {
    let participant =
        DomainParticipantFactory::create_participant(30, DomainParticipantQos::default()).unwrap();
    let ts = Arc::new(P1TypeSupport);
    participant.register_type("P1Message", ts.clone()).unwrap();
    let topic = participant
        .create_topic("ReadTakeTopic", "P1Message", TopicQos::default())
        .unwrap();
    let subscriber = participant.create_subscriber(SubscriberQos::default()).unwrap();
    let reader = subscriber
        .create_datareader(&topic, DataReaderQos::default(), ts)
        .unwrap();

    reader.push_sample(
        dds::types::instance::InstanceHandle::NIL,
        dds::cdr::serialize_to_bytes(
            &P1Message {
                content: "hello".to_string(),
            },
            dds::cdr::Endianness::LittleEndian,
        )
        .unwrap()
        .to_vec(),
        None,
    );

    let (boxed, info) = reader.read().unwrap();
    assert!(info.valid_data);
    assert_eq!(
        boxed.downcast_ref::<P1Message>().unwrap().content,
        "hello"
    );
    // read() leaves sample available
    let (boxed2, _) = reader.read().unwrap();
    assert_eq!(
        boxed2.downcast_ref::<P1Message>().unwrap().content,
        "hello"
    );

    let (boxed3, _) = reader.take().unwrap();
    assert_eq!(
        boxed3.downcast_ref::<P1Message>().unwrap().content,
        "hello"
    );
    assert!(reader.read().is_err());
}

#[test]
fn test_partition_blocks_matchmaking() {
    let remote = vec!["lab".to_string()];
    let mut local_match = Partition::default();
    local_match.name = vec!["lab".to_string()];
    let mut local_mismatch = Partition::default();
    local_mismatch.name = vec!["other".to_string()];
    assert!(check_partition_compatibility(&remote, &local_match));
    assert!(!check_partition_compatibility(&remote, &local_mismatch));
}

#[test]
fn test_reverse_matchmaking_subscription_matched() {
    use dds::core::DataReaderListener;
    use dds::types::status::SubscriptionMatchedStatus;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct SubMatchListener {
        matched: AtomicBool,
    }

    impl dds::core::Listener for SubMatchListener {}

    impl DataReaderListener for SubMatchListener {
        fn on_data_available(&self, _reader: &dds::core::DataReader) {}
        fn on_subscription_matched(
            &self,
            _reader: &dds::core::DataReader,
            status: SubscriptionMatchedStatus,
        ) {
            if status.current_count_change > 0 {
                self.matched.store(true, Ordering::SeqCst);
            }
        }
    }

    let participant_sub =
        DomainParticipantFactory::create_participant(31, DomainParticipantQos::default()).unwrap();
    let ts = Arc::new(P1TypeSupport);
    participant_sub.register_type("P1Message", ts.clone()).unwrap();
    let topic_sub = participant_sub
        .create_topic("ReverseTopic", "P1Message", TopicQos::default())
        .unwrap();
    let subscriber = participant_sub.create_subscriber(SubscriberQos::default()).unwrap();
    let reader = subscriber
        .create_datareader(&topic_sub, DataReaderQos::default(), ts.clone())
        .unwrap();

    let listener = Arc::new(SubMatchListener {
        matched: AtomicBool::new(false),
    });
    reader.set_listener(Some(listener.clone()));

    let participant_pub =
        DomainParticipantFactory::create_participant(31, DomainParticipantQos::default()).unwrap();
    participant_pub.register_type("P1Message", ts.clone()).unwrap();
    let topic_pub = participant_pub
        .create_topic("ReverseTopic", "P1Message", TopicQos::default())
        .unwrap();
    let publisher = participant_pub.create_publisher(PublisherQos::default()).unwrap();
    let writer = publisher
        .create_datawriter(&topic_pub, DataWriterQos::default(), ts)
        .unwrap();

    // Inject remote writer into subscriber's discovery view
    {
        let mut disc = participant_sub.discovery.lock().unwrap();
        disc.process_spdp_packet(dds_discovery::DiscoveredParticipant {
            guid_prefix: participant_pub.guid_prefix(),
            unicast_locators: vec![dds::types::locator::Locator::udpv4(
                std::net::Ipv4Addr::LOCALHOST,
                7400,
            )],
            metatraffic_unicast_locators: vec![dds::types::locator::Locator::udpv4(
                std::net::Ipv4Addr::LOCALHOST,
                7400,
            )],
            multicast_locators: vec![],
            lease_duration: dds::types::time::Duration::from_secs(100),
            last_contact: std::time::Instant::now(),
        });
        disc.process_sedp_endpoint(dds_discovery::DiscoveredEndpoint {
            guid: writer.guid(),
            topic_name: "ReverseTopic".to_string(),
            type_name: "P1Message".to_string(),
            qos_writer: Some(DataWriterQos::default()),
            qos_reader: None,
            partition: vec![],
            unicast_locators: vec![],
            metatraffic_unicast_locators: vec![],
            multicast_locators: vec![],
            type_info: None,
            type_information_wire: None,
        });
    }
    participant_sub.run_matchmaking();

    std::thread::sleep(Duration::from_millis(50));
    assert!(
        listener.matched.load(Ordering::SeqCst),
        "Reverse matchmaking should fire on_subscription_matched"
    );
}
