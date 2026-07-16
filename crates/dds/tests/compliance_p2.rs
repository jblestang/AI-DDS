//! P2 compliance: durability retransmit, instance lifecycle, builtin topics.

use dds::cdr::{CdrDeserialize, CdrSerialize, CdrDeserializer, CdrSerializer, CdrResult};
use dds::core::{DomainParticipantFactory, TypeSupport};
use dds::types::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, DurabilityKind, PublisherQos,
    SubscriberQos, TopicQos,
};
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
struct KeyedMessage {
    id: u32,
    content: String,
}

impl CdrSerialize for KeyedMessage {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_u32(self.id);
        serializer.serialize_str(&self.content);
        Ok(())
    }
}

impl CdrDeserialize for KeyedMessage {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        Ok(Self {
            id: deserializer.deserialize_u32()?,
            content: deserializer.deserialize_str()?,
        })
    }
}

struct KeyedTypeSupport;

impl TypeSupport for KeyedTypeSupport {
    fn get_type_name(&self) -> &str {
        "KeyedMessage"
    }

    fn serialize(&self, value: &dyn Any) -> dds::types::return_code::DdsResult<Vec<u8>> {
        if let Some(msg) = value.downcast_ref::<KeyedMessage>() {
            let bytes = dds::cdr::serialize_to_bytes(msg, dds::cdr::Endianness::LittleEndian)
                .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
            Ok(bytes.to_vec())
        } else {
            Err(dds::types::return_code::DdsError::BadParameter("cast failed".into()))
        }
    }

    fn deserialize(&self, bytes: &[u8]) -> dds::types::return_code::DdsResult<Box<dyn Any>> {
        let msg: KeyedMessage =
            dds::cdr::deserialize_from_slice(bytes, dds::cdr::Endianness::LittleEndian)
                .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
        Ok(Box::new(msg))
    }

    fn get_key_hash(
        &self,
        value: &dyn Any,
    ) -> dds::types::return_code::DdsResult<dds::types::instance::InstanceHandle> {
        if let Some(msg) = value.downcast_ref::<KeyedMessage>() {
            Ok(dds::types::instance::InstanceHandle::from_key_bytes(
                &msg.id.to_le_bytes(),
            ))
        } else {
            Err(dds::types::return_code::DdsError::BadParameter("cast failed".into()))
        }
    }
}

#[test]
fn test_durability_retransmit_on_late_joiner() {
    let participant_sub =
        DomainParticipantFactory::create_participant(40, DomainParticipantQos::default()).unwrap();
    let ts = Arc::new(KeyedTypeSupport);
    participant_sub.register_type("KeyedMessage", ts.clone()).unwrap();
    let topic_sub = participant_sub
        .create_topic("DurabilityTopic", "KeyedMessage", TopicQos::default())
        .unwrap();
    let subscriber = participant_sub.create_subscriber(SubscriberQos::default()).unwrap();
    let reader = subscriber
        .create_datareader(&topic_sub, DataReaderQos::default(), ts.clone())
        .unwrap();
    let _sub_rx = participant_sub.spawn_receiver_loop();

    let participant_pub =
        DomainParticipantFactory::create_participant(40, DomainParticipantQos::default()).unwrap();
    participant_pub.register_type("KeyedMessage", ts.clone()).unwrap();
    let topic_pub = participant_pub
        .create_topic("DurabilityTopic", "KeyedMessage", TopicQos::default())
        .unwrap();
    let _pub_rx = participant_pub.spawn_receiver_loop();
    let publisher = participant_pub.create_publisher(PublisherQos::default()).unwrap();
    let mut writer_qos = DataWriterQos::default();
    writer_qos.durability.kind = DurabilityKind::TransientLocal;
    let writer = publisher
        .create_datawriter(&topic_pub, writer_qos, ts.clone())
        .unwrap();

    let sample = KeyedMessage {
        id: 1,
        content: "durability cached".to_string(),
    };
    writer.register_instance(&sample).unwrap();
    writer.write(&sample).unwrap();

    let reader_locator = dds::types::locator::Locator::udpv4(
        std::net::Ipv4Addr::LOCALHOST,
        subscriber.unicast_port(),
    );
    {
        let mut disc = participant_pub.discovery.lock().unwrap();
        disc.process_spdp_packet(dds_discovery::DiscoveredParticipant {
            guid_prefix: participant_sub.guid_prefix(),
            unicast_locators: vec![reader_locator],
            multicast_locators: vec![],
            lease_duration: dds::types::time::Duration::from_secs(100),
            last_contact: std::time::Instant::now(),
        });
        disc.process_sedp_endpoint(dds_discovery::DiscoveredEndpoint {
            guid: reader.guid(),
            topic_name: "DurabilityTopic".to_string(),
            type_name: "KeyedMessage".to_string(),
            qos_writer: None,
            qos_reader: Some(DataReaderQos::default()),
            partition: vec![],
            type_info: None,
        });
    }
    participant_pub.run_matchmaking();

    let start = std::time::Instant::now();
    let mut received = None;
    while start.elapsed() < Duration::from_secs(3) {
        if let Ok(boxed) = reader.read_next() {
            if let Some(msg) = boxed.downcast_ref::<KeyedMessage>() {
                received = Some(msg.clone());
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        received,
        Some(sample),
        "Durability service should retransmit cached samples to late joiner"
    );
}

#[test]
fn test_instance_lifecycle() {
    let participant =
        DomainParticipantFactory::create_participant(41, DomainParticipantQos::default()).unwrap();
    let ts = Arc::new(KeyedTypeSupport);
    participant.register_type("KeyedMessage", ts.clone()).unwrap();
    let topic = participant
        .create_topic("InstTopic", "KeyedMessage", TopicQos::default())
        .unwrap();
    let publisher = participant.create_publisher(PublisherQos::default()).unwrap();
    let writer = publisher
        .create_datawriter(&topic, DataWriterQos::default(), ts)
        .unwrap();

    let sample = KeyedMessage {
        id: 7,
        content: "inst".to_string(),
    };
    assert!(writer.write(&sample).is_err());
    let handle = writer.register_instance(&sample).unwrap();
    writer.write(&sample).unwrap();
    writer.dispose(handle).unwrap();
    assert!(writer.write(&sample).is_err());
    writer.unregister_instance(handle).unwrap();
}

#[test]
fn test_enable_builtin_topics() {
    let participant =
        DomainParticipantFactory::create_participant(42, DomainParticipantQos::default()).unwrap();
    participant.enable_builtin_topics().unwrap();
    let reg = participant.discovery.lock().unwrap();
    assert!(reg.local_endpoints().values().any(|e| e.topic_name == "DCPSPublication"));
}
