//! # dds-discovery — SPDP/SEDP Discovery Protocols
//!
//! Implements participant and endpoint discovery for DDS using the
//! Simple Participant Discovery Protocol (SPDP) and Simple Endpoint
//! Discovery Protocol (SEDP).
//!
//! Reference: RTPS §8.5

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
    clippy::pub_use,
    clippy::indexing_slicing,
    clippy::string_slice,
    clippy::absolute_paths,
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::missing_inline_in_public_items,
    clippy::shadow_reuse,
    clippy::shadow_same,
    clippy::shadow_unrelated,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::wildcard_imports,
    clippy::integer_division,
    clippy::integer_division_remainder_used,
    clippy::single_call_fn,
    clippy::default_numeric_fallback,
    clippy::arithmetic_side_effects,
    clippy::std_instead_of_core,
    clippy::std_instead_of_alloc,
    clippy::alloc_instead_of_core,
    clippy::arbitrary_source_item_ordering,
    clippy::min_ident_chars,
    clippy::exhaustive_enums,
    clippy::exhaustive_structs,
    clippy::module_name_repetitions,
    clippy::question_mark_used,
    clippy::single_char_lifetime_names,
    clippy::panic_in_result_fn,
    clippy::unwrap_used,
    clippy::unwrap_in_result,
    clippy::cognitive_complexity,
    clippy::tests_outside_test_module,
    clippy::missing_docs_in_private_items,
    clippy::pattern_type_mismatch,
    clippy::redundant_pub_crate,
    clippy::similar_names,
    clippy::else_if_without_else,
    clippy::unseparated_literal_suffix,
    clippy::separated_literal_suffix,
    reason = "DDS Discovery implementation requires standard library conversions, standard returns, and discovery state structures."
)]

use dds_types::guid::{EntityId, EntityKind, Guid, GuidPrefix};
use dds_types::locator::Locator;
use dds_types::time::Duration;
use std::collections::HashMap;
use std::sync::Arc;

use dds_types::qos::{DataReaderQos, DataWriterQos};

// ──────────────────────────────────────────────────────────────────────────────
// Discovery Parameter IDs (PIDs)
// ──────────────────────────────────────────────────────────────────────────────

/// PID for Topic Name
pub const PID_TOPIC_NAME: u16 = 0x0005;

/// PID for Type Name
pub const PID_TYPE_NAME: u16 = 0x0007;

/// PID for Reliability QoS
pub const PID_RELIABILITY: u16 = 0x001A;

/// PID for Durability QoS
pub const PID_DURABILITY: u16 = 0x001D;

/// PID for History QoS
pub const PID_HISTORY: u16 = 0x0040;

/// PID for Partition QoS
pub const PID_PARTITION: u16 = 0x0029;

/// PID for Participant GUID
pub const PID_PARTICIPANT_GUID: u16 = 0x0050;

/// PID for Endpoint GUID
pub const PID_ENDPOINT_GUID: u16 = 0x005A;

/// PID for Lease Duration
pub const PID_LEASE_DURATION: u16 = 0x0002;

/// PID for Default Unicast Locator
pub const PID_DEFAULT_UNICAST_LOCATOR: u16 = 0x0031;

/// PID for Default Multicast Locator
pub const PID_DEFAULT_MULTICAST_LOCATOR: u16 = 0x0048;

/// RTPS Base Port number (PB)
pub const PORT_BASE: u16 = 7400;

/// RTPS Domain ID Gain (DG)
pub const DOMAIN_ID_GAIN: u16 = 250;

/// Default Multicast IP for SPDP
pub const DEFAULT_MULTICAST_IP: [u8; 4] = [239, 255, 0, 1];

/// Localhost IP (for loopback networking in tests)
pub const LOCALHOST_IP: &str = "127.0.0.1";

/// Returns the standard SPDP/SEDP metatraffic multicast locator for a domain.
#[must_use]
pub fn metatraffic_multicast_locator(domain_id: u32) -> Locator {
    let multicast_port = PORT_BASE + DOMAIN_ID_GAIN * domain_id as u16;
    Locator::udpv4(
        std::net::Ipv4Addr::new(
            DEFAULT_MULTICAST_IP[0],
            DEFAULT_MULTICAST_IP[1],
            DEFAULT_MULTICAST_IP[2],
            DEFAULT_MULTICAST_IP[3],
        ),
        multicast_port as u32,
    )
}

/// Classify an endpoint entity as a DataReader.
#[must_use]
pub const fn is_reader_entity(entity_id: &EntityId) -> bool {
    matches!(
        entity_id.kind(),
        EntityKind::ReaderNoKey
            | EntityKind::ReaderWithKey
            | EntityKind::BuiltinReaderNoKey
            | EntityKind::BuiltinReaderWithKey
    )
}

/// Classify an endpoint entity as a DataWriter.
#[must_use]
pub const fn is_writer_entity(entity_id: &EntityId) -> bool {
    matches!(
        entity_id.kind(),
        EntityKind::WriterNoKey
            | EntityKind::WriterWithKey
            | EntityKind::BuiltinWriterNoKey
            | EntityKind::BuiltinWriterWithKey
    )
}

// ──────────────────────────────────────────────────────────────────────────────

/// Participant row for monitor UIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorParticipant {
    pub guid_prefix: GuidPrefix,
    pub alive: bool,
    pub lease_duration: Duration,
    pub unicast_locators: Vec<Locator>,
}

/// Endpoint row for monitor UIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorEndpoint {
    pub guid: Guid,
    pub topic_name: String,
    pub type_name: String,
    pub is_writer: bool,
    pub partition: Vec<String>,
}

/// Point-in-time discovery state for monitoring tools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorSnapshot {
    pub participants: Vec<MonitorParticipant>,
    pub endpoints: Vec<MonitorEndpoint>,
}

/// Represents a remote `DataWriter` or `DataReader` discovered via SEDP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredEndpoint {
    pub guid: Guid,
    pub topic_name: String,
    pub type_name: String,
    pub qos_writer: Option<DataWriterQos>,
    pub qos_reader: Option<DataReaderQos>,
    /// Partition names from SEDP (publisher/subscriber level).
    pub partition: Vec<String>,
    pub type_info: Option<dds_xtypes::TypeInformation>,
}

/// Discovery participant representation holding contact details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredParticipant {
    pub guid_prefix: GuidPrefix,
    pub unicast_locators: Vec<Locator>,
    pub multicast_locators: Vec<Locator>,
    pub lease_duration: Duration,
    pub last_contact: std::time::Instant,
}

/// The discovery manager that orchestrates SPDP/SEDP.
#[derive(Debug)]
pub struct DiscoveryManager {
    local_prefix: GuidPrefix,
    discovered_participants: HashMap<GuidPrefix, DiscoveredParticipant>,
    discovered_endpoints: HashMap<Guid, DiscoveredEndpoint>,
    /// Locally created endpoints announced via SEDP.
    local_endpoints: HashMap<Guid, DiscoveredEndpoint>,
    // Mapping built-in or user-defined topics to active endpoints
    builtin_mappings: HashMap<String, Vec<Guid>>,
    /// Local TypeObject database for TypeLookup replies.
    type_object_db: HashMap<dds_xtypes::TypeIdentifier, dds_xtypes::TypeObject>,
    /// Sequence number for TypeLookup reply samples.
    type_lookup_reply_sn: dds_types::guid::SequenceNumber,
    /// Replies received on the TypeLookup reply builtin endpoint.
    type_lookup_replies: Vec<dds_xtypes::TypeLookupReply>,
}

impl DiscoveryManager {
    #[must_use]
    pub fn new(local_prefix: GuidPrefix) -> Self {
        Self {
            local_prefix,
            discovered_participants: HashMap::new(),
            discovered_endpoints: HashMap::new(),
            local_endpoints: HashMap::new(),
            builtin_mappings: HashMap::new(),
            type_object_db: HashMap::new(),
            type_lookup_reply_sn: dds_types::guid::SequenceNumber(1),
            type_lookup_replies: Vec::new(),
        }
    }

    /// Register a local DataWriter or DataReader for SEDP announcement.
    pub fn register_local_endpoint(&mut self, endpoint: DiscoveredEndpoint) {
        self.builtin_mappings
            .entry(endpoint.topic_name.clone())
            .or_default()
            .push(endpoint.guid);
        self.local_endpoints.insert(endpoint.guid, endpoint);
    }

    /// Return locally registered endpoints.
    #[must_use]
    pub const fn local_endpoints(&self) -> &HashMap<Guid, DiscoveredEndpoint> {
        &self.local_endpoints
    }

    /// Look up the topic name for a discovered or local endpoint GUID.
    #[must_use]
    pub fn topic_for_endpoint(&self, guid: &Guid) -> Option<&str> {
        self.discovered_endpoints
            .get(guid)
            .or_else(|| self.local_endpoints.get(guid))
            .map(|e| e.topic_name.as_str())
    }

    /// Process a newly received SPDP discovery packet.
    pub fn process_spdp_packet(&mut self, mut participant: DiscoveredParticipant) {
        if participant.guid_prefix != self.local_prefix {
            participant.last_contact = std::time::Instant::now();
            self.discovered_participants
                .insert(participant.guid_prefix, participant);
        }
    }

    /// Process a newly received SEDP endpoint discovery packet.
    pub fn process_sedp_endpoint(&mut self, endpoint: DiscoveredEndpoint) {
        if endpoint.guid.prefix == self.local_prefix {
            return;
        }
        self.discovered_endpoints
            .insert(endpoint.guid, endpoint.clone());
        self.builtin_mappings
            .entry(endpoint.topic_name.clone())
            .or_default()
            .push(endpoint.guid);
    }

    /// Announce the local participant via SPDP on metatraffic multicast.
    pub fn announce_local_participant(
        &self,
        transport: &Arc<dds_rtps::UdpTransport>,
        domain_id: u32,
        unicast_locators: &[Locator],
        multicast_locators: &[Locator],
    ) -> Result<(), String> {
        let info = DiscoveredParticipant {
            guid_prefix: self.local_prefix,
            unicast_locators: unicast_locators.to_vec(),
            multicast_locators: multicast_locators.to_vec(),
            lease_duration: Duration::from_secs(100),
            last_contact: std::time::Instant::now(),
        };
        let payload = spdp_to_plcdr(&info).map_err(|e| format!("{e:?}"))?;
        let data_sub = dds_rtps::Data {
            reader_id: EntityId::UNKNOWN,
            writer_id: EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER,
            writer_sn: dds_types::guid::SequenceNumber(1),
            inline_qos: None,
            serialized_payload: bytes::Bytes::from(payload),
        };
        let header = dds_rtps::RtpsHeader::new(self.local_prefix);
        let msg = dds_rtps::serialize_rtps_message(
            &header,
            &[dds_rtps::Submessage::Data(data_sub)],
            dds_rtps::Endianness::LittleEndian,
        );
        transport
            .send(&msg, &metatraffic_multicast_locator(domain_id))
            .map_err(|e| format!("{e:?}"))
    }

    /// Spawn SPDP announcer background thread.
    pub fn spawn_spdp_announcer(
        &self,
        interval: core::time::Duration,
        transport: Arc<dds_rtps::UdpTransport>,
        domain_id: u32,
        unicast_locators: Vec<Locator>,
        multicast_locators: Vec<Locator>,
        destination_locator: Option<Locator>,
    ) -> std::thread::JoinHandle<()> {
        let local_prefix = self.local_prefix;
        std::thread::spawn(move || {
            let dest_locator =
                destination_locator.unwrap_or_else(|| metatraffic_multicast_locator(domain_id));
            loop {
                let info = DiscoveredParticipant {
                    guid_prefix: local_prefix,
                    unicast_locators: unicast_locators.clone(),
                    multicast_locators: multicast_locators.clone(),
                    lease_duration: Duration::from_secs(100),
                    last_contact: std::time::Instant::now(),
                };
                if let Ok(payload) = spdp_to_plcdr(&info) {
                    let data_sub = dds_rtps::Data {
                        reader_id: EntityId::UNKNOWN,
                        writer_id: EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER,
                        writer_sn: dds_types::guid::SequenceNumber(1),
                        inline_qos: None,
                        serialized_payload: bytes::Bytes::from(payload),
                    };
                    let header = dds_rtps::RtpsHeader::new(local_prefix);
                    let msg = dds_rtps::serialize_rtps_message(
                        &header,
                        &[dds_rtps::Submessage::Data(data_sub)],
                        dds_rtps::Endianness::LittleEndian,
                    );
                    let _ = transport.send(&msg, &dest_locator);
                }
                std::thread::sleep(interval);
            }
        })
    }

    /// Announce a single local endpoint via SEDP on the metatraffic multicast channel.
    pub fn announce_endpoint(
        &self,
        transport: &Arc<dds_rtps::UdpTransport>,
        domain_id: u32,
        endpoint: &DiscoveredEndpoint,
    ) -> Result<(), String> {
        use bytes::Bytes;
        use dds_rtps::{serialize_rtps_message, Data, Endianness, RtpsHeader, Submessage};

        let payload = sedp_to_plcdr(endpoint).map_err(|e| format!("{e:?}"))?;
        let writer_id = if endpoint.qos_writer.is_some() {
            EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER
        } else {
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER
        };
        let data_sub = Data {
            reader_id: EntityId::UNKNOWN,
            writer_id,
            writer_sn: dds_types::guid::SequenceNumber(1),
            inline_qos: None,
            serialized_payload: Bytes::from(payload),
        };
        let header = RtpsHeader::new(self.local_prefix);
        let msg = serialize_rtps_message(
            &header,
            &[Submessage::Data(data_sub)],
            Endianness::LittleEndian,
        );
        let dest = metatraffic_multicast_locator(domain_id);
        transport
            .send(&msg, &dest)
            .map_err(|e| format!("{e:?}"))
    }

    /// Spawn a background thread that periodically re-announces all local endpoints via SEDP.
    pub fn spawn_sedp_announcer(
        discovery: Arc<std::sync::Mutex<Self>>,
        interval: core::time::Duration,
        transport: Arc<dds_rtps::UdpTransport>,
        domain_id: u32,
    ) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || loop {
            let endpoints: Vec<DiscoveredEndpoint> = {
                let disc = discovery.lock().unwrap();
                disc.local_endpoints.values().cloned().collect()
            };
            let local_prefix = {
                let disc = discovery.lock().unwrap();
                disc.local_prefix
            };
            for endpoint in &endpoints {
                let payload = match sedp_to_plcdr(endpoint) {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let writer_id = if endpoint.qos_writer.is_some() {
                    EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER
                } else {
                    EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER
                };
                let data_sub = dds_rtps::Data {
                    reader_id: EntityId::UNKNOWN,
                    writer_id,
                    writer_sn: dds_types::guid::SequenceNumber(1),
                    inline_qos: None,
                    serialized_payload: bytes::Bytes::from(payload),
                };
                let header = dds_rtps::RtpsHeader::new(local_prefix);
                let msg = dds_rtps::serialize_rtps_message(
                    &header,
                    &[dds_rtps::Submessage::Data(data_sub)],
                    dds_rtps::Endianness::LittleEndian,
                );
                let dest = metatraffic_multicast_locator(domain_id);
                let _ = transport.send(&msg, &dest);
            }
            std::thread::sleep(interval);
        })
    }

    /// Retrieve the currently known remote participants.
    #[must_use]
    pub const fn discovered_participants(&self) -> &HashMap<GuidPrefix, DiscoveredParticipant> {
        &self.discovered_participants
    }

    /// Retrieve the currently known remote endpoints.
    #[must_use]
    pub const fn discovered_endpoints(&self) -> &HashMap<Guid, DiscoveredEndpoint> {
        &self.discovered_endpoints
    }

    /// Export a snapshot suitable for the `dds-monitor` application.
    #[must_use]
    pub fn monitor_snapshot(&self) -> MonitorSnapshot {
        let participants = self
            .discovered_participants
            .values()
            .map(|p| MonitorParticipant {
                guid_prefix: p.guid_prefix,
                alive: p.lease_duration != Duration::from_secs(0),
                lease_duration: p.lease_duration,
                unicast_locators: p.unicast_locators.clone(),
            })
            .collect();
        let mut endpoints = Vec::new();
        for ep in self.discovered_endpoints.values() {
            endpoints.push(MonitorEndpoint {
                guid: ep.guid,
                topic_name: ep.topic_name.clone(),
                type_name: ep.type_name.clone(),
                is_writer: ep.qos_writer.is_some(),
                partition: ep.partition.clone(),
            });
        }
        for ep in self.local_endpoints.values() {
            endpoints.push(MonitorEndpoint {
                guid: ep.guid,
                topic_name: ep.topic_name.clone(),
                type_name: ep.type_name.clone(),
                is_writer: ep.qos_writer.is_some(),
                partition: ep.partition.clone(),
            });
        }
        MonitorSnapshot {
            participants,
            endpoints,
        }
    }

    /// Store a TypeLookup reply received from the wire.
    pub fn push_type_lookup_reply(&mut self, reply: dds_xtypes::TypeLookupReply) {
        self.type_lookup_replies.push(reply);
    }

    /// Drain all pending TypeLookup replies (client-side inbox).
    #[must_use]
    pub fn drain_type_lookup_replies(&mut self) -> Vec<dds_xtypes::TypeLookupReply> {
        std::mem::take(&mut self.type_lookup_replies)
    }

    /// Register a complete TypeObject for wire TypeLookup responses.
    pub fn register_type_object(&mut self, type_object: dds_xtypes::TypeObject) {
        let id = type_object.get_identifier();
        self.type_object_db.insert(id, type_object);
    }

    /// Return the local TypeObject database (for tests and monitor tools).
    #[must_use]
    pub fn type_object_db(&self) -> &HashMap<dds_xtypes::TypeIdentifier, dds_xtypes::TypeObject> {
        &self.type_object_db
    }

    /// Parse a TypeLookup request payload and build a compliant reply.
    #[must_use]
    pub fn handle_type_lookup_request(
        &self,
        payload: &[u8],
    ) -> Option<dds_xtypes::TypeLookupReply> {
        let request = dds_xtypes::TypeLookupRequest::from_wire_bytes(payload).ok()?;
        Some(dds_xtypes::serve_type_lookup_request(
            &request,
            &self.type_object_db,
        ))
    }

    /// Send a TypeLookup reply on the metatraffic channel (XCDR2 wire format).
    pub fn send_type_lookup_reply(
        &mut self,
        transport: &Arc<dds_rtps::UdpTransport>,
        _domain_id: u32,
        reply: &dds_xtypes::TypeLookupReply,
        destination: &Locator,
    ) -> Result<(), String> {
        use bytes::Bytes;
        use dds_rtps::{serialize_rtps_message, Data, Endianness as RtpsEndianness, RtpsHeader, Submessage};

        let payload = reply
            .to_wire_bytes()
            .map_err(|e| format!("{e:?}"))?;
        let data_sub = Data {
            reader_id: EntityId::UNKNOWN,
            writer_id: EntityId::BUILTIN_TYPE_LOOKUP_REPLY_DATA_WRITER,
            writer_sn: self.type_lookup_reply_sn,
            inline_qos: None,
            serialized_payload: Bytes::from(payload),
        };
        self.type_lookup_reply_sn = dds_types::guid::SequenceNumber(self.type_lookup_reply_sn.0 + 1);
        let header = RtpsHeader::new(self.local_prefix);
        let msg = serialize_rtps_message(
            &header,
            &[Submessage::Data(data_sub)],
            RtpsEndianness::LittleEndian,
        );
        transport
            .send(&msg, destination)
            .map_err(|e| format!("{e:?}"))
    }

    /// Send a TypeLookup request (client role) to a remote TypeLookup service.
    pub fn send_type_lookup_request(
        &mut self,
        transport: &Arc<dds_rtps::UdpTransport>,
        request: &dds_xtypes::TypeLookupRequest,
        destination: &Locator,
    ) -> Result<(), String> {
        use bytes::Bytes;
        use dds_rtps::{serialize_rtps_message, Data, Endianness as RtpsEndianness, RtpsHeader, Submessage};

        let payload = request
            .to_wire_bytes()
            .map_err(|e| format!("{e:?}"))?;
        let sn = request.header.request_id.sequence_number;
        let data_sub = Data {
            reader_id: EntityId::UNKNOWN,
            writer_id: EntityId::BUILTIN_TYPE_LOOKUP_REQUEST_DATA_WRITER,
            writer_sn: sn,
            inline_qos: None,
            serialized_payload: Bytes::from(payload),
        };
        let header = RtpsHeader::new(self.local_prefix);
        let msg = serialize_rtps_message(
            &header,
            &[Submessage::Data(data_sub)],
            RtpsEndianness::LittleEndian,
        );
        transport
            .send(&msg, destination)
            .map_err(|e| format!("{e:?}"))
    }

    /// Process an incoming TypeLookup request and send the reply to `destination`.
    pub fn process_type_lookup_request(
        &mut self,
        transport: &Arc<dds_rtps::UdpTransport>,
        domain_id: u32,
        payload: &[u8],
        destination: &Locator,
    ) -> bool {
        if let Some(reply) = self.handle_type_lookup_request(payload) {
            self.send_type_lookup_reply(transport, domain_id, &reply, destination)
                .is_ok()
        } else {
            false
        }
    }

    /// Simulates a `TypeLookup` service request to retrieve a complete `TypeObject` for a discovered type.
    #[must_use]
    pub fn lookup_type_object(
        &self,
        endpoint_guid: &Guid,
        db: &HashMap<dds_xtypes::TypeIdentifier, dds_xtypes::TypeObject>,
    ) -> Option<dds_xtypes::TypeObject> {
        let ep = self.discovered_endpoints.get(endpoint_guid)?;
        let info = ep.type_info.as_ref()?;
        db.get(&info.type_id)
            .or_else(|| self.type_object_db.get(&info.type_id))
            .cloned()
    }

    /// Remove a participant and all its associated endpoints.
    pub fn remove_participant(&mut self, prefix: &GuidPrefix) {
        self.discovered_participants.remove(prefix);
        self.discovered_endpoints
            .retain(|guid, _| &guid.prefix != prefix);
    }

    /// Clean up expired participants based on lease duration.
    pub fn check_lease_timeouts(&mut self) {
        let now = std::time::Instant::now();
        let mut expired = Vec::new();

        for (prefix, participant) in &self.discovered_participants {
            if let Some(std_dur) = participant.lease_duration.to_std() {
                if now.duration_since(participant.last_contact) > std_dur {
                    expired.push(*prefix);
                }
            }
        }

        for prefix in expired {
            self.remove_participant(&prefix);
        }
    }

    /// Check if a discovered endpoint is a built-in discovery endpoint.
    #[must_use]
    pub const fn is_builtin_endpoint(entity_id: &dds_types::guid::EntityId) -> bool {
        use dds_types::guid::EntityId;
        matches!(
            *entity_id,
            EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER
                | EntityId::SPDP_BUILTIN_PARTICIPANT_READER
                | EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER
                | EntityId::SEDP_BUILTIN_PUBLICATIONS_READER
                | EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER
                | EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER
                | EntityId::BUILTIN_TYPE_LOOKUP_REQUEST_DATA_WRITER
                | EntityId::BUILTIN_TYPE_LOOKUP_REQUEST_DATA_READER
                | EntityId::BUILTIN_TYPE_LOOKUP_REPLY_DATA_WRITER
                | EntityId::BUILTIN_TYPE_LOOKUP_REPLY_DATA_READER
        )
    }
}

/// Serializes a `DiscoveredParticipant` to a PL-CDR parameter list.
///
/// Reference: RTPS §9.6.3 — ParameterList values
pub fn spdp_to_plcdr(participant: &DiscoveredParticipant) -> dds_cdr::CdrResult<Vec<u8>> {
    use dds_cdr::{ParameterList, ParameterId, serialize_to_bytes, Endianness};

    let mut plist = ParameterList::new();

    // 1. Participant GUID (0x0050)
    let mut guid_bytes = Vec::new();
    guid_bytes.extend_from_slice(participant.guid_prefix.as_bytes());
    guid_bytes.extend_from_slice(&dds_types::guid::EntityId::PARTICIPANT.0);
    plist.add(ParameterId(PID_PARTICIPANT_GUID), guid_bytes);

    // 2. Lease Duration (0x0002)
    let mut lease_bytes = Vec::new();
    lease_bytes.extend_from_slice(&participant.lease_duration.seconds.to_le_bytes());
    lease_bytes.extend_from_slice(&participant.lease_duration.nanoseconds.to_le_bytes());
    plist.add(ParameterId(PID_LEASE_DURATION), lease_bytes);

    // 3. Unicast Locators (0x0031)
    for locator in &participant.unicast_locators {
        let mut loc_bytes = Vec::new();
        loc_bytes.extend_from_slice(&(locator.kind as i32).to_le_bytes());
        loc_bytes.extend_from_slice(&locator.port.to_le_bytes());
        loc_bytes.extend_from_slice(&locator.address);
        plist.add(ParameterId(PID_DEFAULT_UNICAST_LOCATOR), loc_bytes);
    }

    // 4. Multicast Locators (0x0048)
    for locator in &participant.multicast_locators {
        let mut loc_bytes = Vec::new();
        loc_bytes.extend_from_slice(&(locator.kind as i32).to_le_bytes());
        loc_bytes.extend_from_slice(&locator.port.to_le_bytes());
        loc_bytes.extend_from_slice(&locator.address);
        plist.add(ParameterId(PID_DEFAULT_MULTICAST_LOCATOR), loc_bytes);
    }

    let serialized = serialize_to_bytes(&plist, Endianness::LittleEndian)?;
    Ok(serialized.to_vec())
}

/// Parses a `DiscoveredParticipant` from a PL-CDR parameter list byte buffer.
pub fn parse_spdp_packet(bytes: &[u8]) -> Option<DiscoveredParticipant> {
    use dds_cdr::{ParameterList, deserialize_from_slice, Endianness};

    let plist: ParameterList = deserialize_from_slice(bytes, Endianness::LittleEndian).ok()?;

    let mut guid_prefix = GuidPrefix::UNKNOWN;
    let mut lease_duration = Duration::INFINITE;
    let mut unicast_locators = Vec::new();
    let mut multicast_locators = Vec::new();

    for param in &plist.parameters {
        match param.parameter_id.0 {
            PID_PARTICIPANT_GUID => {
                if param.value.len() >= 16 {
                    let mut prefix_bytes = [0u8; 12];
                    prefix_bytes.copy_from_slice(&param.value[0..12]);
                    guid_prefix = GuidPrefix::new(prefix_bytes);
                }
            }
            PID_LEASE_DURATION => {
                if param.value.len() >= 8 {
                    let seconds = i32::from_le_bytes(param.value[0..4].try_into().ok()?);
                    let nanoseconds = u32::from_le_bytes(param.value[4..8].try_into().ok()?);
                    lease_duration = Duration::new(seconds, nanoseconds);
                }
            }
            PID_DEFAULT_UNICAST_LOCATOR => {
                if param.value.len() >= 24 {
                    let kind_val = i32::from_le_bytes(param.value[0..4].try_into().ok()?);
                    let port = u32::from_le_bytes(param.value[4..8].try_into().ok()?);
                    let mut address = [0u8; 16];
                    address.copy_from_slice(&param.value[8..24]);
                    unicast_locators.push(Locator {
                        kind: dds_types::locator::LocatorKind::from_i32(kind_val),
                        port,
                        address,
                    });
                }
            }
            PID_DEFAULT_MULTICAST_LOCATOR => {
                if param.value.len() >= 24 {
                    let kind_val = i32::from_le_bytes(param.value[0..4].try_into().ok()?);
                    let port = u32::from_le_bytes(param.value[4..8].try_into().ok()?);
                    let mut address = [0u8; 16];
                    address.copy_from_slice(&param.value[8..24]);
                    multicast_locators.push(Locator {
                        kind: dds_types::locator::LocatorKind::from_i32(kind_val),
                        port,
                        address,
                    });
                }
            }
            _ => {}
        }
    }

    if guid_prefix.is_unknown() {
        return None;
    }

    Some(DiscoveredParticipant {
        guid_prefix,
        unicast_locators,
        multicast_locators,
        lease_duration,
        last_contact: std::time::Instant::now(),
    })
}

/// Serializes a `DiscoveredEndpoint` to a PL-CDR parameter list.
pub fn sedp_to_plcdr(endpoint: &DiscoveredEndpoint) -> dds_cdr::CdrResult<Vec<u8>> {
    use dds_cdr::{ParameterList, ParameterId, Parameter, serialize_to_bytes, Endianness};
    let mut plist = ParameterList::new();

    // PID_TOPIC_NAME (0x0005)
    let mut topic_name_bytes = endpoint.topic_name.as_bytes().to_vec();
    topic_name_bytes.push(0); // null terminator
    // Padding to 4 bytes
    while topic_name_bytes.len() % 4 != 0 {
        topic_name_bytes.push(0);
    }
    plist.parameters.push(Parameter {
        parameter_id: ParameterId(PID_TOPIC_NAME),
        value: topic_name_bytes,
    });

    // PID_TYPE_NAME (0x0007)
    let mut type_name_bytes = endpoint.type_name.as_bytes().to_vec();
    type_name_bytes.push(0);
    while type_name_bytes.len() % 4 != 0 {
        type_name_bytes.push(0);
    }
    plist.parameters.push(Parameter {
        parameter_id: ParameterId(PID_TYPE_NAME),
        value: type_name_bytes,
    });

    // PID_ENDPOINT_GUID (0x005A)
    let mut guid_bytes = Vec::with_capacity(16);
    guid_bytes.extend_from_slice(&endpoint.guid.prefix.0);
    guid_bytes.extend_from_slice(&endpoint.guid.entity_id.0);
    plist.parameters.push(Parameter {
        parameter_id: ParameterId(PID_ENDPOINT_GUID),
        value: guid_bytes,
    });

    if let Some(ref qos) = endpoint.qos_writer {
        let durability_val = match qos.durability.kind {
            dds_types::qos::DurabilityKind::Volatile => 0u32,
            dds_types::qos::DurabilityKind::TransientLocal => 1,
            dds_types::qos::DurabilityKind::Transient => 2,
            dds_types::qos::DurabilityKind::Persistent => 3,
        };
        plist.parameters.push(Parameter {
            parameter_id: ParameterId(PID_DURABILITY),
            value: durability_val.to_le_bytes().to_vec(),
        });
        let reliability_val = match qos.reliability.kind {
            dds_types::qos::ReliabilityKind::BestEffort => 1u32,
            dds_types::qos::ReliabilityKind::Reliable => 2,
        };
        plist.parameters.push(Parameter {
            parameter_id: ParameterId(PID_RELIABILITY),
            value: reliability_val.to_le_bytes().to_vec(),
        });
    }

    if let Some(ref qos) = endpoint.qos_reader {
        let durability_val = match qos.durability.kind {
            dds_types::qos::DurabilityKind::Volatile => 0u32,
            dds_types::qos::DurabilityKind::TransientLocal => 1,
            dds_types::qos::DurabilityKind::Transient => 2,
            dds_types::qos::DurabilityKind::Persistent => 3,
        };
        plist.parameters.push(Parameter {
            parameter_id: ParameterId(PID_DURABILITY),
            value: durability_val.to_le_bytes().to_vec(),
        });
        let reliability_val = match qos.reliability.kind {
            dds_types::qos::ReliabilityKind::BestEffort => 1u32,
            dds_types::qos::ReliabilityKind::Reliable => 2,
        };
        plist.parameters.push(Parameter {
            parameter_id: ParameterId(PID_RELIABILITY),
            value: reliability_val.to_le_bytes().to_vec(),
        });
    }

    if !endpoint.partition.is_empty() {
        let mut partition_bytes = Vec::new();
        for name in &endpoint.partition {
            let mut name_bytes = name.as_bytes().to_vec();
            name_bytes.push(0);
            while name_bytes.len() % 4 != 0 {
                name_bytes.push(0);
            }
            partition_bytes.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
            partition_bytes.extend_from_slice(&name_bytes);
        }
        plist.parameters.push(Parameter {
            parameter_id: ParameterId(PID_PARTITION),
            value: partition_bytes,
        });
    }

    serialize_to_bytes(&plist, Endianness::LittleEndian).map(|b| b.to_vec())
}

/// Parses an SEDP PL-CDR parameter list into a `DiscoveredEndpoint`.
pub fn parse_sedp_packet(bytes: &[u8]) -> Option<DiscoveredEndpoint> {
    use dds_cdr::{ParameterList, deserialize_from_slice, Endianness};
    let plist: ParameterList = deserialize_from_slice(bytes, Endianness::LittleEndian).ok()?;

    let mut guid = dds_types::guid::Guid::new(
        GuidPrefix::new([0; 12]),
        dds_types::guid::EntityId::new([0; 4]),
    );
    let mut topic_name = String::new();
    let mut type_name = String::new();
    
    let mut qos_writer = None;
    let mut qos_reader = None;
    let mut partition = Vec::new();

    for param in &plist.parameters {
        match param.parameter_id.0 {
            PID_ENDPOINT_GUID => { // PID_ENDPOINT_GUID
                if param.value.len() >= 16 {
                    let mut prefix_bytes = [0u8; 12];
                    prefix_bytes.copy_from_slice(&param.value[0..12]);
                    let mut entity_bytes = [0u8; 4];
                    entity_bytes.copy_from_slice(&param.value[12..16]);
                    guid = dds_types::guid::Guid::new(
                        GuidPrefix::new(prefix_bytes),
                        dds_types::guid::EntityId::new(entity_bytes),
                    );
                    
                    if is_reader_entity(&guid.entity_id) {
                        let mut qr = dds_types::qos::DataReaderQos::default();
                        qr.reliability.kind = dds_types::qos::ReliabilityKind::Reliable;
                        qos_reader = Some(qr);
                    } else if is_writer_entity(&guid.entity_id) {
                        let mut qw = dds_types::qos::DataWriterQos::default();
                        qw.reliability.kind = dds_types::qos::ReliabilityKind::Reliable;
                        qos_writer = Some(qw);
                    }
                }
            }
            PID_TOPIC_NAME => { // PID_TOPIC_NAME
                if let Ok(s) = std::ffi::CStr::from_bytes_until_nul(&param.value) {
                    topic_name = s.to_string_lossy().into_owned();
                }
            }
            PID_TYPE_NAME => { // PID_TYPE_NAME
                if let Ok(s) = std::ffi::CStr::from_bytes_until_nul(&param.value) {
                    type_name = s.to_string_lossy().into_owned();
                }
            }
            PID_DURABILITY => { // PID_DURABILITY
                if param.value.len() >= 4 {
                    let kind_val = u32::from_le_bytes([param.value[0], param.value[1], param.value[2], param.value[3]]);
                    let kind = match kind_val {
                        0 => dds_types::qos::DurabilityKind::Volatile,
                        1 => dds_types::qos::DurabilityKind::TransientLocal,
                        2 => dds_types::qos::DurabilityKind::Transient,
                        3 => dds_types::qos::DurabilityKind::Persistent,
                        _ => dds_types::qos::DurabilityKind::Volatile,
                    };
                    if let Some(ref mut qw) = qos_writer { qw.durability.kind = kind; }
                    if let Some(ref mut qr) = qos_reader { qr.durability.kind = kind; }
                }
            }
            PID_RELIABILITY => { // PID_RELIABILITY
                if param.value.len() >= 4 {
                    let kind_val = u32::from_le_bytes([param.value[0], param.value[1], param.value[2], param.value[3]]);
                    let kind = match kind_val {
                        1 => dds_types::qos::ReliabilityKind::BestEffort,
                        2 => dds_types::qos::ReliabilityKind::Reliable,
                        _ => dds_types::qos::ReliabilityKind::BestEffort,
                    };
                    if let Some(ref mut qw) = qos_writer { qw.reliability.kind = kind; }
                    if let Some(ref mut qr) = qos_reader { qr.reliability.kind = kind; }
                }
            }
            PID_HISTORY => { // PID_HISTORY
                if param.value.len() >= 4 {
                    let kind_val = u32::from_le_bytes([param.value[0], param.value[1], param.value[2], param.value[3]]);
                    let kind = match kind_val {
                        0 => dds_types::qos::HistoryKind::KeepLast,
                        1 => dds_types::qos::HistoryKind::KeepAll,
                        _ => dds_types::qos::HistoryKind::KeepLast,
                    };
                    if let Some(ref mut qw) = qos_writer { qw.history.kind = kind; }
                    if let Some(ref mut qr) = qos_reader { qr.history.kind = kind; }
                }
            }
            PID_PARTITION => {
                let mut offset = 0;
                while offset + 4 <= param.value.len() {
                    let len = u32::from_le_bytes([
                        param.value[offset],
                        param.value[offset + 1],
                        param.value[offset + 2],
                        param.value[offset + 3],
                    ]) as usize;
                    offset += 4;
                    if offset + len > param.value.len() {
                        break;
                    }
                    if let Ok(s) = std::ffi::CStr::from_bytes_until_nul(&param.value[offset..offset + len]) {
                        partition.push(s.to_string_lossy().into_owned());
                    }
                    offset += len;
                }
            }
            _ => {}
        }
    }

    if topic_name.is_empty() || type_name.is_empty() {
        return None;
    }

    Some(DiscoveredEndpoint {
        guid,
        topic_name,
        type_name,
        qos_writer,
        qos_reader,
        partition,
        type_info: None,
    })
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use dds_types::guid::EntityId;
    use std::sync::Arc;

    #[test]
    fn test_entity_kind_classification() {
        use dds_types::guid::{EntityId, EntityKind};

        let reader = EntityId::new([0, 0, 1, EntityKind::ReaderWithKey as u8]);
        let writer = EntityId::new([0, 0, 1, EntityKind::WriterWithKey as u8]);
        assert!(is_reader_entity(&reader));
        assert!(!is_writer_entity(&reader));
        assert!(is_writer_entity(&writer));
        assert!(!is_reader_entity(&writer));
    }

    #[test]
    fn test_local_endpoint_registration() {
        let local_prefix = GuidPrefix::new([1; 12]);
        let mut manager = DiscoveryManager::new(local_prefix);

        let remote_prefix = GuidPrefix::new([2; 12]);
        let participant = DiscoveredParticipant {
            guid_prefix: remote_prefix,
            unicast_locators: vec![],
            multicast_locators: vec![],
            lease_duration: Duration::from_secs(100),
            last_contact: std::time::Instant::now(),
        };

        manager.process_spdp_packet(participant.clone());
        assert_eq!(manager.discovered_participants().len(), 1);

        let retrieved = manager
            .discovered_participants()
            .get(&remote_prefix)
            .unwrap();
        assert_eq!(retrieved.guid_prefix, participant.guid_prefix);

        // Verify local participant is ignored
        let local_participant = DiscoveredParticipant {
            guid_prefix: local_prefix,
            unicast_locators: vec![],
            multicast_locators: vec![],
            lease_duration: Duration::from_secs(100),
            last_contact: std::time::Instant::now(),
        };
        manager.process_spdp_packet(local_participant);
        assert_eq!(manager.discovered_participants().len(), 1);

        // Test SEDP Endpoint discovery
        let endpoint = DiscoveredEndpoint {
            guid: Guid::new(remote_prefix, EntityId::new([0, 0, 1, 4])),
            topic_name: "TestTopic".into(),
            type_name: "TestType".into(),
            qos_writer: None,
            qos_reader: None,
            partition: vec![],
            type_info: None,
        };
        manager.process_sedp_endpoint(endpoint.clone());
        assert_eq!(manager.discovered_endpoints().len(), 1);

        // Verify removing participant cleans up endpoints
        manager.remove_participant(&remote_prefix);
        assert_eq!(manager.discovered_participants().len(), 0);
        assert_eq!(manager.discovered_endpoints().len(), 0);
    }

    #[test]
    fn test_lease_timeout() {
        let local_prefix = GuidPrefix::new([1; 12]);
        let mut manager = DiscoveryManager::new(local_prefix);
        let remote_prefix = GuidPrefix::new([2; 12]);

        let participant = DiscoveredParticipant {
            guid_prefix: remote_prefix,
            unicast_locators: vec![],
            multicast_locators: vec![],
            lease_duration: Duration::from_secs(0), // instantaneous timeout
            last_contact: std::time::Instant::now() - std::time::Duration::from_secs(1),
        };

        // Directly insert to simulate expired last_contact
        manager
            .discovered_participants
            .insert(remote_prefix, participant);
        assert_eq!(manager.discovered_participants().len(), 1);

        manager.check_lease_timeouts();
        assert_eq!(manager.discovered_participants().len(), 0);
    }

    #[test]
    fn test_builtin_endpoint_checks() {
        use dds_types::guid::EntityId;
        assert!(DiscoveryManager::is_builtin_endpoint(
            &EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER
        ));
        assert!(DiscoveryManager::is_builtin_endpoint(
            &EntityId::SEDP_BUILTIN_PUBLICATIONS_READER
        ));

        let user_defined_entity = EntityId::new([0x00, 0x00, 0x01, 0x02]);
        assert!(!DiscoveryManager::is_builtin_endpoint(&user_defined_entity));
    }

    #[test]
    fn test_builtin_mappings_and_announcer() {
        let local_prefix = GuidPrefix::new([1; 12]);
        let mut manager = DiscoveryManager::new(local_prefix);
        let remote_prefix = GuidPrefix::new([2; 12]);

        // Register remote participant so we can accept its endpoints
        let remote_participant = DiscoveredParticipant {
            guid_prefix: remote_prefix,
            unicast_locators: vec![],
            multicast_locators: vec![],
            lease_duration: Duration::from_secs(100),
            last_contact: std::time::Instant::now(),
        };
        manager.process_spdp_packet(remote_participant);

        let endpoint = DiscoveredEndpoint {
            guid: Guid::new(remote_prefix, EntityId::new([0, 0, 1, 4])),
            topic_name: "Position".into(),
            type_name: "Geometry::Point".into(),
            qos_writer: None,
            qos_reader: None,
            partition: vec![],
            type_info: None,
        };
        manager.process_sedp_endpoint(endpoint);

        let mappings = manager.builtin_mappings.get("Position").unwrap();
        assert_eq!(mappings.len(), 1);

        // Test spawning the SPDP announcer
        let transport = Arc::new(dds_rtps::UdpTransport::bind(0).unwrap());
        let domain_id = 12;

        let receiver = std::net::UdpSocket::bind(format!("{LOCALHOST_IP}:0")).unwrap();
        let receiver_port = receiver.local_addr().unwrap().port();
        receiver.set_nonblocking(true).unwrap();

        let dest_locator = dds_types::locator::Locator::udpv4(
            std::net::Ipv4Addr::LOCALHOST,
            u32::from(receiver_port),
        );

        let _handle = manager.spawn_spdp_announcer(
            std::time::Duration::from_millis(10),
            transport,
            domain_id,
            vec![dest_locator],
            vec![],
            Some(dest_locator),
        );

        // Wait a tiny bit and confirm packet is received and parsed
        std::thread::sleep(std::time::Duration::from_millis(50));
        let mut buf = [0u8; 1024];
        let mut received = false;
        if let Ok((len, _)) = receiver.recv_from(&mut buf) {
            if let Ok((_header, submessages)) = dds_rtps::parse_rtps_message(&buf[..len]) {
                for sub in submessages {
                    if let dds_rtps::Submessage::Data(d) = sub {
                        if let Some(parsed) = parse_spdp_packet(&d.serialized_payload) {
                            assert_eq!(parsed.guid_prefix, local_prefix);
                            received = true;
                        }
                    }
                }
            }
        }
        assert!(received, "Should have received SPDP announcement over UDP");
    }

    #[test]
    fn test_spdp_roundtrip() {
        let participant = DiscoveredParticipant {
            guid_prefix: GuidPrefix::new([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
            unicast_locators: vec![
                Locator::udpv4(std::net::Ipv4Addr::new(127, 0, 0, 1), PORT_BASE as u32 + 10),
            ],
            multicast_locators: vec![
                Locator::udpv4(std::net::Ipv4Addr::new(DEFAULT_MULTICAST_IP[0], DEFAULT_MULTICAST_IP[1], DEFAULT_MULTICAST_IP[2], DEFAULT_MULTICAST_IP[3]), PORT_BASE as u32),
            ],
            lease_duration: Duration::from_secs(120),
            last_contact: std::time::Instant::now(),
        };

        let bytes = spdp_to_plcdr(&participant).unwrap();
        let decoded = parse_spdp_packet(&bytes).unwrap();

        assert_eq!(decoded.guid_prefix, participant.guid_prefix);
        assert_eq!(decoded.lease_duration, participant.lease_duration);
        assert_eq!(decoded.unicast_locators.len(), 1);
        assert_eq!(decoded.unicast_locators[0].port, PORT_BASE as u32 + 10);
        assert_eq!(decoded.multicast_locators.len(), 1);
        assert_eq!(decoded.multicast_locators[0].port, PORT_BASE as u32);
    }

    #[test]
    fn test_typelookup_service() {
        let local_prefix = GuidPrefix::new([1; 12]);
        let mut manager = DiscoveryManager::new(local_prefix);
        let remote_prefix = GuidPrefix::new([2; 12]);

        let remote_participant = DiscoveredParticipant {
            guid_prefix: remote_prefix,
            unicast_locators: vec![],
            multicast_locators: vec![],
            lease_duration: Duration::from_secs(100),
            last_contact: std::time::Instant::now(),
        };
        manager.process_spdp_packet(remote_participant);

        let r_obj = dds_xtypes::TypeObject::Complete(dds_xtypes::StructureType {
            name: "Dummy".to_string(),
            extensibility: dds_xtypes::ExtensibilityKind::Final,
            members: vec![],
        });
        let r_id = r_obj.get_identifier();
        manager.register_type_object(r_obj.clone());

        let endpoint_guid = Guid::new(remote_prefix, EntityId::new([0, 0, 1, 4]));
        let endpoint = DiscoveredEndpoint {
            guid: endpoint_guid,
            topic_name: "Position".into(),
            type_name: "Geometry::Point".into(),
            qos_writer: None,
            qos_reader: None,
            partition: vec![],
            type_info: Some(dds_xtypes::TypeInformation {
                type_name: "MyInt".to_string(),
                type_id: r_id.clone(),
            }),
        };
        manager.process_sedp_endpoint(endpoint);

        let lookup_res = manager.lookup_type_object(&endpoint_guid, &HashMap::new()).unwrap();
        assert_eq!(lookup_res, r_obj);

        let request = dds_xtypes::make_get_types_request(
            Guid::new(remote_prefix, EntityId::BUILTIN_TYPE_LOOKUP_REQUEST_DATA_WRITER),
            dds_types::guid::SequenceNumber(1),
            dds_xtypes::type_lookup_instance_name(&local_prefix),
            vec![r_id.clone()],
        );
        let wire = request.to_wire_bytes().unwrap();
        let reply = manager.handle_type_lookup_request(&wire).unwrap();
        match reply.return_data {
            dds_xtypes::TypeLookupReturn::GetTypes(dds_xtypes::TypeLookupGetTypesResult::Ok(out)) => {
                assert_eq!(out.types.len(), 1);
                assert_eq!(out.types[0].type_object, r_obj);
            }
            _ => panic!("expected getTypes wire reply"),
        }
    }

    #[test]
    fn test_sedp_partition_roundtrip() {
        let guid = Guid::new(GuidPrefix::new([3; 12]), EntityId::new([0, 0, 1, 4]));
        let mut qos_writer = DataWriterQos::default();
        qos_writer.reliability.kind = dds_types::qos::ReliabilityKind::Reliable;
        let endpoint = DiscoveredEndpoint {
            guid,
            topic_name: "PartitionTopic".into(),
            type_name: "MyType".into(),
            qos_writer: Some(qos_writer),
            qos_reader: None,
            partition: vec!["lab".to_string(), "test".to_string()],
            type_info: None,
        };
        let bytes = sedp_to_plcdr(&endpoint).unwrap();
        let parsed = parse_sedp_packet(&bytes).unwrap();
        assert_eq!(parsed.partition, endpoint.partition);
        assert_eq!(parsed.topic_name, "PartitionTopic");
    }

    #[test]
    fn test_monitor_snapshot() {
        let local = GuidPrefix::new([1; 12]);
        let mut manager = DiscoveryManager::new(local);
        let remote = GuidPrefix::new([2; 12]);
        manager.process_spdp_packet(DiscoveredParticipant {
            guid_prefix: remote,
            unicast_locators: vec![],
            multicast_locators: vec![],
            lease_duration: Duration::from_secs(10),
            last_contact: std::time::Instant::now(),
        });
        let snapshot = manager.monitor_snapshot();
        assert_eq!(snapshot.participants.len(), 1);
        assert!(snapshot.endpoints.is_empty());
    }
}
