//! Integration test: automatic discovery-driven endpoint matching (no manual proxy wiring).

use dds::cdr::{CdrDeserialize, CdrSerialize, CdrDeserializer, CdrSerializer, CdrResult};
use dds::core::{DomainParticipantFactory, TypeSupport};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, PublisherQos, SubscriberQos, TopicQos,
};
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscoveryMessage {
    content: String,
}

impl CdrSerialize for DiscoveryMessage {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_str(&self.content);
        Ok(())
    }
}

impl CdrDeserialize for DiscoveryMessage {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        Ok(Self {
            content: deserializer.deserialize_str()?,
        })
    }
}

struct DiscoveryMessageTypeSupport;

impl TypeSupport for DiscoveryMessageTypeSupport {
    fn get_type_name(&self) -> &str {
        "DiscoveryMessage"
    }

    fn serialize(&self, value: &dyn Any) -> dds::types::return_code::DdsResult<Vec<u8>> {
        if let Some(msg) = value.downcast_ref::<DiscoveryMessage>() {
            let bytes = dds::cdr::serialize_to_bytes(msg, dds::cdr::Endianness::LittleEndian)
                .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
            Ok(bytes.to_vec())
        } else {
            Err(dds::types::return_code::DdsError::BadParameter(
                "cast failed".into(),
            ))
        }
    }

    fn deserialize(&self, bytes: &[u8]) -> dds::types::return_code::DdsResult<Box<dyn Any>> {
        let msg: DiscoveryMessage =
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
fn test_discovery_driven_matchmaking() {
    let participant_sub =
        DomainParticipantFactory::create_participant(16, DomainParticipantQos::default())
            .expect("Failed to create subscriber participant");

    let ts = Arc::new(DiscoveryMessageTypeSupport);
    participant_sub
        .register_type("DiscoveryMessage", ts.clone())
        .unwrap();

    let topic_sub = participant_sub
        .create_topic("DiscoveryTopic", "DiscoveryMessage", TopicQos::default())
        .unwrap();

    let subscriber = participant_sub
        .create_subscriber(SubscriberQos::default())
        .unwrap();
    let reader = subscriber
        .create_datareader(&topic_sub, DataReaderQos::default(), ts.clone())
        .unwrap();

    // Bind subscriber receive port before creating the publisher participant.
    let _sub_receiver = participant_sub.spawn_receiver_loop();

    let participant_pub =
        DomainParticipantFactory::create_participant(16, DomainParticipantQos::default())
            .expect("Failed to create publisher participant");
    participant_pub
        .register_type("DiscoveryMessage", ts.clone())
        .unwrap();

    let topic_pub = participant_pub
        .create_topic("DiscoveryTopic", "DiscoveryMessage", TopicQos::default())
        .unwrap();

    let _pub_receiver = participant_pub.spawn_receiver_loop();

    let publisher = participant_pub
        .create_publisher(PublisherQos::default())
        .unwrap();
    let writer = publisher
        .create_datawriter(&topic_pub, DataWriterQos::default(), ts)
        .unwrap();

    // Simulate SPDP/SEDP discovery data that multicast would deliver on a live network.
    let reader_guid = reader.guid();
    let reader_locator =
        dds::types::locator::Locator::udpv4(std::net::Ipv4Addr::LOCALHOST, subscriber.unicast_port());
    {
        let mut disc = participant_pub.discovery.lock().unwrap();
        disc.process_spdp_packet(dds_discovery::DiscoveredParticipant {
            guid_prefix: participant_sub.guid_prefix(),
            unicast_locators: vec![reader_locator],
            metatraffic_unicast_locators: vec![reader_locator],
            multicast_locators: vec![],
            lease_duration: dds::types::time::Duration::from_secs(100),
            last_contact: std::time::Instant::now(),
        });
        disc.process_sedp_endpoint(dds_discovery::DiscoveredEndpoint {
            guid: reader_guid,
            topic_name: "DiscoveryTopic".to_string(),
            type_name: "DiscoveryMessage".to_string(),
            qos_writer: None,
            qos_reader: Some(DataReaderQos::default()),
            partition: vec![],
            unicast_locators: vec![reader_locator],
            multicast_locators: vec![],
            type_info: None,
        });
    }
    participant_pub.run_matchmaking();

    let msg = DiscoveryMessage {
        content: "Discovery matchmaking success!".to_string(),
    };
    writer.write(&msg).expect("Failed to write sample");

    let start = std::time::Instant::now();
    let mut received: Option<DiscoveryMessage> = None;
    while start.elapsed() < Duration::from_secs(3) {
        if let Ok(boxed) = reader.read_next() {
            if let Some(decoded) = boxed.downcast_ref::<DiscoveryMessage>() {
                received = Some(decoded.clone());
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    assert_eq!(
        received,
        Some(msg),
        "Expected delivery via discovery matchmaking (no manual add_reader_proxy_to_all)"
    );
}

#[test]
fn test_incompatible_qos_blocks_matchmaking() {
    let participant_pub =
        DomainParticipantFactory::create_participant(18, DomainParticipantQos::default()).unwrap();
    let participant_sub =
        DomainParticipantFactory::create_participant(18, DomainParticipantQos::default()).unwrap();

    let ts = Arc::new(DiscoveryMessageTypeSupport);
    participant_pub.register_type("DiscoveryMessage", ts.clone()).unwrap();
    participant_sub.register_type("DiscoveryMessage", ts.clone()).unwrap();

    let topic_pub = participant_pub
        .create_topic("DiscoveryTopic", "DiscoveryMessage", TopicQos::default())
        .unwrap();
    let topic_sub = participant_sub
        .create_topic("DiscoveryTopic", "DiscoveryMessage", TopicQos::default())
        .unwrap();

    let subscriber = participant_sub.create_subscriber(SubscriberQos::default()).unwrap();
    let mut reader_qos = DataReaderQos::default();
    reader_qos.reliability.kind = dds::types::qos::ReliabilityKind::Reliable;
    let reader = subscriber
        .create_datareader(&topic_sub, reader_qos, ts.clone())
        .unwrap();

    let _sub_receiver = participant_sub.spawn_receiver_loop();

    let publisher = participant_pub.create_publisher(PublisherQos::default()).unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.reliability.kind = dds::types::qos::ReliabilityKind::BestEffort;
    let writer = publisher
        .create_datawriter(&topic_pub, writer_qos, ts)
        .unwrap();

    let reader_locator =
        dds::types::locator::Locator::udpv4(std::net::Ipv4Addr::LOCALHOST, subscriber.unicast_port());
    {
        let mut disc = participant_pub.discovery.lock().unwrap();
        disc.process_spdp_packet(dds_discovery::DiscoveredParticipant {
            guid_prefix: participant_sub.guid_prefix(),
            unicast_locators: vec![reader_locator],
            metatraffic_unicast_locators: vec![reader_locator],
            multicast_locators: vec![],
            lease_duration: dds::types::time::Duration::from_secs(100),
            last_contact: std::time::Instant::now(),
        });
        disc.process_sedp_endpoint(dds_discovery::DiscoveredEndpoint {
            guid: reader.guid(),
            topic_name: "DiscoveryTopic".to_string(),
            type_name: "DiscoveryMessage".to_string(),
            qos_writer: None,
            qos_reader: Some(reader.qos().clone()),
            partition: vec![],
            unicast_locators: vec![reader_locator],
            multicast_locators: vec![],
            type_info: None,
        });
    }
    participant_pub.run_matchmaking();

    writer
        .write(&DiscoveryMessage {
            content: "should not arrive".to_string(),
        })
        .unwrap();

    std::thread::sleep(Duration::from_millis(200));
    assert!(
        reader.read_next().is_err(),
        "Incompatible reliability QoS should prevent reader proxy creation and delivery"
    );
}
