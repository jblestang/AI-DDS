//! Shared helpers for end-to-end integration tests in the `dds` crate.

use dds::cdr::{CdrDeserialize, CdrSerialize, CdrDeserializer, CdrSerializer, CdrResult};
use dds::core::{DomainParticipantFactory, TypeSupport};
use dds::types::guid::{Guid, GuidPrefix};
use dds::types::locator::Locator;
use dds::types::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos,
};
use dds::xtypes::{
    ExtensibilityKind, Member, StructureType, TypeIdentifier, TypeInformation, TypeObject,
};
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

// ── Standard wire message types ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireMessage {
    pub id: u32,
    pub payload: String,
}

impl CdrSerialize for WireMessage {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_u32(self.id);
        serializer.serialize_str(&self.payload);
        Ok(())
    }
}

impl CdrDeserialize for WireMessage {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        Ok(Self {
            id: deserializer.deserialize_u32()?,
            payload: deserializer.deserialize_str()?,
        })
    }
}

pub struct WireMessageTypeSupport;

impl TypeSupport for WireMessageTypeSupport {
    fn get_type_name(&self) -> &str {
        "WireMessage"
    }

    fn serialize(&self, value: &dyn Any) -> dds::types::return_code::DdsResult<Vec<u8>> {
        let msg = value
            .downcast_ref::<WireMessage>()
            .ok_or_else(|| dds::types::return_code::DdsError::BadParameter("cast failed".into()))?;
        Ok(
            dds::cdr::serialize_to_bytes(msg, dds::cdr::Endianness::LittleEndian)
                .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?
                .to_vec(),
        )
    }

    fn deserialize(&self, bytes: &[u8]) -> dds::types::return_code::DdsResult<Box<dyn Any>> {
        let msg: WireMessage =
            dds::cdr::deserialize_from_slice(bytes, dds::cdr::Endianness::LittleEndian)
                .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?;
        Ok(Box::new(msg))
    }

    fn get_key_hash(
        &self,
        value: &dyn Any,
    ) -> dds::types::return_code::DdsResult<dds::types::instance::InstanceHandle> {
        let msg = value
            .downcast_ref::<WireMessage>()
            .ok_or_else(|| dds::types::return_code::DdsError::BadParameter("cast failed".into()))?;
        Ok(dds::types::instance::InstanceHandle::from_key_bytes(
            &msg.id.to_le_bytes(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlainMessage {
    pub content: String,
}

impl CdrSerialize for PlainMessage {
    fn serialize(&self, serializer: &mut CdrSerializer) -> CdrResult<()> {
        serializer.serialize_str(&self.content);
        Ok(())
    }
}

impl CdrDeserialize for PlainMessage {
    fn deserialize(deserializer: &mut CdrDeserializer<'_>) -> CdrResult<Self> {
        Ok(Self {
            content: deserializer.deserialize_str()?,
        })
    }
}

pub struct PlainMessageTypeSupport;

impl TypeSupport for PlainMessageTypeSupport {
    fn get_type_name(&self) -> &str {
        "PlainMessage"
    }

    fn serialize(&self, value: &dyn Any) -> dds::types::return_code::DdsResult<Vec<u8>> {
        let msg = value
            .downcast_ref::<PlainMessage>()
            .ok_or_else(|| dds::types::return_code::DdsError::BadParameter("cast failed".into()))?;
        Ok(
            dds::cdr::serialize_to_bytes(msg, dds::cdr::Endianness::LittleEndian)
                .map_err(|e| dds::types::return_code::DdsError::Error(e.to_string()))?
                .to_vec(),
        )
    }

    fn deserialize(&self, bytes: &[u8]) -> dds::types::return_code::DdsResult<Box<dyn Any>> {
        let msg: PlainMessage =
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

// ── Type objects ──────────────────────────────────────────────────────────────

pub fn wire_message_type_object() -> TypeObject {
    TypeObject::Complete(StructureType {
        name: "WireMessage".to_string(),
        extensibility: ExtensibilityKind::Appendable,
        members: vec![
            Member {
                name: "id".to_string(),
                type_id: TypeIdentifier::TkUint32,
                is_key: true,
                is_optional: false,
            },
            Member {
                name: "payload".to_string(),
                type_id: TypeIdentifier::TiString8Large { bound: 0 },
                is_key: false,
                is_optional: false,
            },
        ],
    })
}

pub fn type_info_for(obj: &TypeObject) -> TypeInformation {
    TypeInformation {
        type_name: match obj {
            TypeObject::Complete(s) => s.name.clone(),
            TypeObject::Minimal(_) => String::new(),
        },
        type_id: obj.get_identifier(),
    }
}

pub fn type_with_nested_dependency() -> (TypeObject, TypeIdentifier) {
    let nested_dep = TypeIdentifier::TiCompleteConstructed([0xAB; 14]);
    let obj = TypeObject::Complete(StructureType {
        name: "WithNestedDep".to_string(),
        extensibility: ExtensibilityKind::Final,
        members: vec![Member {
            name: "nested".to_string(),
            type_id: nested_dep.clone(),
            is_key: false,
            is_optional: false,
        }],
    });
    (obj, nested_dep)
}

// ── Timing / locators ─────────────────────────────────────────────────────────

pub fn localhost_locator(port: u32) -> Locator {
    Locator::udpv4(std::net::Ipv4Addr::LOCALHOST, port)
}

pub fn wait_until(timeout: Duration, mut pred: impl FnMut() -> bool) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if pred() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    false
}

pub fn read_next_plain(
    reader: &dds::core::DataReader,
    timeout: Duration,
) -> Option<PlainMessage> {
    let mut received = None;
    wait_until(timeout, || {
        if let Ok(boxed) = reader.read_next() {
            if let Some(msg) = boxed.downcast_ref::<PlainMessage>() {
                received = Some(msg.clone());
                return true;
            }
        }
        false
    });
    received
}

pub fn read_next_wire(reader: &dds::core::DataReader, timeout: Duration) -> Option<WireMessage> {
    let mut received = None;
    wait_until(timeout, || {
        if let Ok(boxed) = reader.read_next() {
            if let Some(msg) = boxed.downcast_ref::<WireMessage>() {
                received = Some(msg.clone());
                return true;
            }
        }
        false
    });
    received
}

// ── Discovery wiring ──────────────────────────────────────────────────────────

pub struct DiscoveryWire {
    pub topic: String,
    pub type_name: String,
    pub partition: Vec<String>,
    pub type_info: Option<TypeInformation>,
    pub writer_qos: Option<DataWriterQos>,
    pub reader_qos: Option<DataReaderQos>,
}

pub fn inject_remote_participant(
    local: &dds::core::DomainParticipant,
    remote_prefix: GuidPrefix,
    remote_unicast: Locator,
) {
    let mut disc = local.discovery.lock().unwrap();
    disc.process_spdp_packet(dds::discovery::DiscoveredParticipant {
        guid_prefix: remote_prefix,
        unicast_locators: vec![remote_unicast],
        multicast_locators: vec![],
        lease_duration: dds::types::time::Duration::from_secs(120),
        last_contact: std::time::Instant::now(),
    });
}

pub fn inject_remote_endpoint(
    local: &dds::core::DomainParticipant,
    remote_prefix: GuidPrefix,
    remote_unicast: Locator,
    endpoint: dds::discovery::DiscoveredEndpoint,
) {
    inject_remote_participant(local, remote_prefix, remote_unicast);
    local
        .discovery
        .lock()
        .unwrap()
        .process_sedp_endpoint(endpoint);
}

pub fn wire_bidirectional_discovery(
    pub_participant: &dds::core::DomainParticipant,
    sub_participant: &dds::core::DomainParticipant,
    sub_subscriber_port: u32,
    writer_guid: Guid,
    reader_guid: Guid,
    wire: &DiscoveryWire,
) {
    let pub_locator = localhost_locator(pub_participant.unicast_port());
    let sub_locator = localhost_locator(sub_subscriber_port);

    inject_remote_endpoint(
        sub_participant,
        pub_participant.guid_prefix(),
        pub_locator,
        dds::discovery::DiscoveredEndpoint {
            guid: writer_guid,
            topic_name: wire.topic.clone(),
            type_name: wire.type_name.clone(),
            qos_writer: wire.writer_qos.clone(),
            qos_reader: None,
            partition: wire.partition.clone(),
            type_info: wire.type_info.clone(),
        },
    );

    inject_remote_endpoint(
        pub_participant,
        sub_participant.guid_prefix(),
        sub_locator,
        dds::discovery::DiscoveredEndpoint {
            guid: reader_guid,
            topic_name: wire.topic.clone(),
            type_name: wire.type_name.clone(),
            qos_writer: None,
            qos_reader: wire.reader_qos.clone(),
            partition: wire.partition.clone(),
            type_info: wire.type_info.clone(),
        },
    );

    sub_participant.run_matchmaking();
    pub_participant.run_matchmaking();
}

/// Inject remote reader into publisher's discovery view (pub → sub direction).
pub fn wire_pub_to_sub_reader(
    pub_participant: &dds::core::DomainParticipant,
    sub_prefix: GuidPrefix,
    sub_locator: Locator,
    reader_guid: Guid,
    wire: &DiscoveryWire,
) {
    inject_remote_endpoint(
        pub_participant,
        sub_prefix,
        sub_locator,
        dds::discovery::DiscoveredEndpoint {
            guid: reader_guid,
            topic_name: wire.topic.clone(),
            type_name: wire.type_name.clone(),
            qos_writer: None,
            qos_reader: wire.reader_qos.clone(),
            partition: wire.partition.clone(),
            type_info: wire.type_info.clone(),
        },
    );
    pub_participant.run_matchmaking();
}

/// Inject remote writer into subscriber's discovery view (sub ← pub direction).
pub fn wire_sub_from_pub_writer(
    sub_participant: &dds::core::DomainParticipant,
    pub_prefix: GuidPrefix,
    pub_locator: Locator,
    writer_guid: Guid,
    wire: &DiscoveryWire,
) {
    inject_remote_endpoint(
        sub_participant,
        pub_prefix,
        pub_locator,
        dds::discovery::DiscoveredEndpoint {
            guid: writer_guid,
            topic_name: wire.topic.clone(),
            type_name: wire.type_name.clone(),
            qos_writer: wire.writer_qos.clone(),
            qos_reader: None,
            partition: wire.partition.clone(),
            type_info: wire.type_info.clone(),
        },
    );
    sub_participant.run_matchmaking();
}

// ── Participant factory helpers ───────────────────────────────────────────────

pub struct ParticipantPair {
    pub pub_participant: dds::core::DomainParticipant,
    pub sub_participant: dds::core::DomainParticipant,
}

pub fn create_participant_pair(domain_id: u32) -> ParticipantPair {
    let sub_participant =
        DomainParticipantFactory::create_participant(domain_id, DomainParticipantQos::default())
            .expect("subscriber participant");
    let pub_participant =
        DomainParticipantFactory::create_participant(domain_id, DomainParticipantQos::default())
            .expect("publisher participant");
    ParticipantPair {
        pub_participant,
        sub_participant,
    }
}

pub fn default_wire(topic: &str, type_name: &str) -> DiscoveryWire {
    DiscoveryWire {
        topic: topic.to_string(),
        type_name: type_name.to_string(),
        partition: vec![],
        type_info: None,
        writer_qos: Some(DataWriterQos::default()),
        reader_qos: Some(DataReaderQos::default()),
    }
}

pub fn spawn_receivers(pair: &ParticipantPair) {
    let _ = pair.sub_participant.spawn_receiver_loop();
    let _ = pair.pub_participant.spawn_receiver_loop();
}

pub fn plain_type_support() -> Arc<PlainMessageTypeSupport> {
    Arc::new(PlainMessageTypeSupport)
}

pub fn wire_type_support() -> Arc<WireMessageTypeSupport> {
    Arc::new(WireMessageTypeSupport)
}
