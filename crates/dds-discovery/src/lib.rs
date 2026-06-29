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

use dds_types::guid::GuidPrefix;
use dds_types::locator::Locator;
use dds_types::time::Duration;
use std::collections::HashMap;

use dds_types::guid::Guid;
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

// ──────────────────────────────────────────────────────────────────────────────

/// Represents a remote `DataWriter` or `DataReader` discovered via SEDP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredEndpoint {
    pub guid: Guid,
    pub topic_name: String,
    pub type_name: String,
    pub qos_writer: Option<DataWriterQos>,
    pub qos_reader: Option<DataReaderQos>,
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
    // Mapping built-in or user-defined topics to active endpoints
    builtin_mappings: HashMap<String, Vec<Guid>>,
}

impl DiscoveryManager {
    #[must_use]
    pub fn new(local_prefix: GuidPrefix) -> Self {
        Self {
            local_prefix,
            discovered_participants: HashMap::new(),
            discovered_endpoints: HashMap::new(),
            builtin_mappings: HashMap::new(),
        }
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
        // Only accept endpoints belonging to known participants or our own
        if endpoint.guid.prefix == self.local_prefix
            || self
                .discovered_participants
                .contains_key(&endpoint.guid.prefix)
        {
            self.discovered_endpoints
                .insert(endpoint.guid, endpoint.clone());
            self.builtin_mappings
                .entry(endpoint.topic_name.clone())
                .or_default()
                .push(endpoint.guid);
        }
    }

    /// Spawn SPDP announcer background thread
    pub fn spawn_spdp_announcer(
        &self,
        interval: core::time::Duration,
        transport: std::sync::Arc<dds_rtps::UdpTransport>,
        domain_id: u32,
        destination_locator: Option<dds_types::locator::Locator>,
    ) -> std::thread::JoinHandle<()> {
        let local_prefix = self.local_prefix;
        std::thread::spawn(move || {
            // Multicast address: 239.255.0.1
            let multicast_port = 7400 + 250 * domain_id;
            let dest_locator = destination_locator.unwrap_or_else(|| {
                dds_types::locator::Locator::udpv4(
                    std::net::Ipv4Addr::new(239, 255, 0, 1),
                    multicast_port,
                )
            });
            loop {
                let info = DiscoveredParticipant {
                    guid_prefix: local_prefix,
                    unicast_locators: vec![],
                    multicast_locators: vec![],
                    lease_duration: Duration::from_secs(100),
                    last_contact: std::time::Instant::now(),
                };
                if let Ok(bytes) = spdp_to_plcdr(&info) {
                    let _ = transport.send(&bytes, &dest_locator);
                }
                std::thread::sleep(interval);
            }
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

    /// Simulates a `TypeLookup` service request to retrieve a complete `TypeObject` for a discovered type.
    #[must_use]
    pub fn lookup_type_object(
        &self,
        endpoint_guid: &Guid,
        db: &HashMap<dds_xtypes::TypeIdentifier, dds_xtypes::TypeObject>,
    ) -> Option<dds_xtypes::TypeObject> {
        let ep = self.discovered_endpoints.get(endpoint_guid)?;
        let info = ep.type_info.as_ref()?;
        db.get(&info.type_id).cloned()
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
                    
                    if guid.entity_id.0[3] & 0x02 != 0 {
                        // Reader
                        let mut qr = dds_types::qos::DataReaderQos::default();
                        qr.reliability.kind = dds_types::qos::ReliabilityKind::Reliable;
                        qos_reader = Some(qr);
                    } else {
                        // Writer
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
    fn test_discovery_manager() {
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
            type_info: None,
        };
        manager.process_sedp_endpoint(endpoint);

        let mappings = manager.builtin_mappings.get("Position").unwrap();
        assert_eq!(mappings.len(), 1);

        // Test spawning the SPDP announcer
        let transport = Arc::new(dds_rtps::UdpTransport::bind(0).unwrap());
        let domain_id = 12;

        let receiver = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let receiver_port = receiver.local_addr().unwrap().port();
        receiver.set_nonblocking(true).unwrap();

        let dest_locator = dds_types::locator::Locator::udpv4(
            std::net::Ipv4Addr::new(127, 0, 0, 1),
            u32::from(receiver_port),
        );

        let _handle = manager.spawn_spdp_announcer(
            std::time::Duration::from_millis(10),
            transport,
            domain_id,
            Some(dest_locator),
        );

        // Wait a tiny bit and confirm packet is received and parsed
        std::thread::sleep(std::time::Duration::from_millis(50));
        let mut buf = [0u8; 1024];
        let mut received = false;
        if let Ok((len, _)) = receiver.recv_from(&mut buf) {
            if let Some(parsed) = parse_spdp_packet(&buf[..len]) {
                assert_eq!(parsed.guid_prefix, local_prefix);
                received = true;
            }
        }
        assert!(received, "Should have received SPDP announcement over UDP");
    }

    #[test]
    fn test_spdp_roundtrip() {
        let participant = DiscoveredParticipant {
            guid_prefix: GuidPrefix::new([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
            unicast_locators: vec![
                Locator::udpv4(std::net::Ipv4Addr::new(127, 0, 0, 1), 7410),
            ],
            multicast_locators: vec![
                Locator::udpv4(std::net::Ipv4Addr::new(239, 255, 0, 1), 7400),
            ],
            lease_duration: Duration::from_secs(120),
            last_contact: std::time::Instant::now(),
        };

        let bytes = spdp_to_plcdr(&participant).unwrap();
        let decoded = parse_spdp_packet(&bytes).unwrap();

        assert_eq!(decoded.guid_prefix, participant.guid_prefix);
        assert_eq!(decoded.lease_duration, participant.lease_duration);
        assert_eq!(decoded.unicast_locators.len(), 1);
        assert_eq!(decoded.unicast_locators[0].port, 7410);
        assert_eq!(decoded.multicast_locators.len(), 1);
        assert_eq!(decoded.multicast_locators[0].port, 7400);
    }

    #[test]
    fn test_typelookup_service() {
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

        // Define a type, compute type information and type_id
        let r_obj = dds_xtypes::TypeObject::Complete(dds_xtypes::StructureType {
            name: "Dummy".to_string(),
            extensibility: dds_xtypes::ExtensibilityKind::Final,
            members: vec![],
        });
        let r_id = r_obj.get_identifier();
        let type_info = dds_xtypes::TypeInformation {
            type_name: "MyInt".to_string(),
            type_id: r_id.clone(),
        };

        let endpoint_guid = Guid::new(remote_prefix, EntityId::new([0, 0, 1, 4]));
        let endpoint = DiscoveredEndpoint {
            guid: endpoint_guid,
            topic_name: "Position".into(),
            type_name: "Geometry::Point".into(),
            qos_writer: None,
            qos_reader: None,
            type_info: Some(type_info),
        };
        manager.process_sedp_endpoint(endpoint);

        // Store complete TypeObject in a global/local type database
        let mut db = HashMap::new();
        db.insert(r_id, r_obj.clone());

        // Perform TypeLookup
        let lookup_res = manager.lookup_type_object(&endpoint_guid, &db).unwrap();
        assert_eq!(lookup_res, r_obj);
    }
}
