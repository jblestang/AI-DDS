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

use dds_types::guid::{EntityId, EntityKind, Guid, GuidPrefix, SequenceNumber};
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

/// PID for Liveliness QoS
pub const PID_LIVELINESS: u16 = 0x001B;

/// PID for Partition QoS
pub const PID_PARTITION: u16 = 0x0029;

/// PID for Participant GUID
pub const PID_PARTICIPANT_GUID: u16 = 0x0050;

/// PID for BuiltinEndpointSet (required by CycloneDDS SPDP).
pub const PID_BUILTIN_ENDPOINT_SET: u16 = 0x0058;

/// PID for PropertyList (CycloneDDS includes this in SPDP).
pub const PID_PROPERTY_LIST: u16 = 0x0059;

/// PID for Endpoint GUID
pub const PID_ENDPOINT_GUID: u16 = 0x005A;

/// PID for Lease Duration
pub const PID_LEASE_DURATION: u16 = 0x0002;

/// PID for Domain ID
pub const PID_DOMAIN_ID: u16 = 0x000F;

/// PID for Protocol Version
pub const PID_PROTOCOL_VERSION: u16 = 0x0015;

/// PID for Vendor ID
pub const PID_VENDOR_ID: u16 = 0x0016;

/// PID for Unicast Locator (endpoint)
pub const PID_UNICAST_LOCATOR: u16 = 0x002F;

/// PID for Multicast Locator (endpoint)
pub const PID_MULTICAST_LOCATOR: u16 = 0x0030;

/// PID for Default Unicast Locator
pub const PID_DEFAULT_UNICAST_LOCATOR: u16 = 0x0031;

/// PID for Metatraffic Unicast Locator
pub const PID_METATRAFFIC_UNICAST_LOCATOR: u16 = 0x0032;

/// PID for Metatraffic Multicast Locator
pub const PID_METATRAFFIC_MULTICAST_LOCATOR: u16 = 0x0033;

/// PID for Default Multicast Locator
pub const PID_DEFAULT_MULTICAST_LOCATOR: u16 = 0x0048;

/// PID for DataRepresentation QoS (XTypes)
pub const PID_DATA_REPRESENTATION: u16 = 0x0073;

/// PID for TypeInformation (XTypes, XCDR2 blob)
pub const PID_TYPE_INFORMATION: u16 = 0x0075;

/// XCDR1 little-endian data representation id (DDS CDR LE).
pub const DATA_REPRESENTATION_XCDR1: u16 = 0;

/// XCDR2 little-endian data representation id.
pub const DATA_REPRESENTATION_XCDR2: u16 = 2;

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
    /// Unicast locators advertised for user data on this endpoint.
    pub unicast_locators: Vec<Locator>,
    /// Metatraffic unicast locators for discovery replies (SEDP/SPDP).
    pub metatraffic_unicast_locators: Vec<Locator>,
    /// Multicast locators advertised for this endpoint.
    pub multicast_locators: Vec<Locator>,
    pub type_info: Option<dds_xtypes::TypeInformation>,
    /// Pre-serialized XCDR2 TypeInformation blob for SEDP (`PID_TYPE_INFORMATION`).
    pub type_information_wire: Option<Vec<u8>>,
}

/// Discovery participant representation holding contact details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredParticipant {
    pub guid_prefix: GuidPrefix,
    /// Default (user-traffic) unicast locators (`PID_DEFAULT_UNICAST_LOCATOR`).
    pub unicast_locators: Vec<Locator>,
    /// Metatraffic unicast locators (`PID_METATRAFFIC_UNICAST_LOCATOR`).
    pub metatraffic_unicast_locators: Vec<Locator>,
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
    /// Sequence numbers assigned to local endpoints on each built-in SEDP writer.
    sedp_endpoint_sn: HashMap<Guid, SequenceNumber>,
    next_sedp_publication_sn: SequenceNumber,
    next_sedp_subscription_sn: SequenceNumber,
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
            sedp_endpoint_sn: HashMap::new(),
            next_sedp_publication_sn: SequenceNumber(1),
            next_sedp_subscription_sn: SequenceNumber(1),
        }
    }

    /// Register a local DataWriter or DataReader for SEDP announcement.
    pub fn register_local_endpoint(&mut self, endpoint: DiscoveredEndpoint) {
        self.builtin_mappings
            .entry(endpoint.topic_name.clone())
            .or_default()
            .push(endpoint.guid);
        if endpoint.qos_writer.is_some() {
            let sn = self.next_sedp_publication_sn;
            self.next_sedp_publication_sn = SequenceNumber(sn.0 + 1);
            self.sedp_endpoint_sn.insert(endpoint.guid, sn);
        } else if endpoint.qos_reader.is_some() {
            let sn = self.next_sedp_subscription_sn;
            self.next_sedp_subscription_sn = SequenceNumber(sn.0 + 1);
            self.sedp_endpoint_sn.insert(endpoint.guid, sn);
        }
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
        if self.local_endpoints.contains_key(&endpoint.guid) {
            return;
        }
        self.discovered_endpoints
            .insert(endpoint.guid, endpoint.clone());
        self.builtin_mappings
            .entry(endpoint.topic_name.clone())
            .or_default()
            .push(endpoint.guid);
    }

    /// Keep only usable UDP unicast locators (FastDDS may include INVALID placeholders).
    #[must_use]
    pub fn filter_valid_unicast_locators(locators: &[Locator]) -> Vec<Locator> {
        locators
            .iter()
            .copied()
            .filter(|loc| {
                loc.kind == dds_types::locator::LocatorKind::UdpV4
                    && loc.port != 0
                    && loc.to_ipv4().is_some()
            })
            .collect()
    }

    /// Pick the best locator for sending user/metatraffic data to a remote endpoint.
    #[must_use]
    pub fn select_unicast_locator(candidates: &[Locator]) -> Option<Locator> {
        let valid = Self::filter_valid_unicast_locators(candidates);
        valid
            .iter()
            .find(|loc| loc.to_ipv4().is_some_and(|ip| !ip.is_loopback()))
            .or_else(|| valid.first())
            .copied()
    }

    /// Collect usable locators for user DATA to a remote endpoint.
    #[must_use]
    pub fn user_traffic_locators(
        endpoint: &DiscoveredEndpoint,
        participant: &DiscoveredParticipant,
    ) -> Vec<Locator> {
        let mut locators = Self::filter_valid_unicast_locators(&endpoint.unicast_locators);
        locators.extend(Self::filter_valid_unicast_locators(
            &endpoint.metatraffic_unicast_locators,
        ));
        if locators.is_empty() {
            locators.extend(Self::filter_valid_unicast_locators(
                &participant.unicast_locators,
            ));
        }
        if locators.is_empty() {
            locators.extend(Self::filter_valid_unicast_locators(
                &participant.metatraffic_unicast_locators,
            ));
        }
        // OpenDDS SPDP often advertises 127.0.0.1:12345 as a placeholder.
        locators.retain(|loc| {
            !loc.to_ipv4().is_some_and(|ip| ip.is_loopback() && loc.port == 12345)
        });
        locators.sort_by_key(|loc| loc.port);
        locators.dedup();
        locators
    }

    /// FastDDS often advertises 127.0.0.1 in SEDP while sending from a real interface.
    pub fn remap_loopback_locators(endpoint: &mut DiscoveredEndpoint, source: std::net::Ipv4Addr) {
        if source.is_loopback() {
            return;
        }
        for locators in [
            &mut endpoint.unicast_locators,
            &mut endpoint.metatraffic_unicast_locators,
            &mut endpoint.multicast_locators,
        ] {
            for loc in locators.iter_mut() {
                if loc.kind == dds_types::locator::LocatorKind::UdpV4
                    && loc.to_ipv4().is_some_and(|ip| ip.is_loopback())
                {
                    *loc = Locator::udpv4(source, loc.port);
                }
            }
        }
    }

    /// Collect unicast destinations for built-in SEDP requests to a remote participant.
    fn sedp_request_destinations(participant: &DiscoveredParticipant) -> Vec<Locator> {
        let mut dests =
            Self::filter_valid_unicast_locators(&participant.metatraffic_unicast_locators);
        if dests.is_empty() {
            dests = Self::filter_valid_unicast_locators(&participant.unicast_locators);
        }
        dests
    }

    /// Ask a remote participant to (re)send built-in SEDP samples (late-joiner recovery).
    pub fn request_remote_sedp(
        &self,
        transport: &Arc<dds_rtps::UdpTransport>,
        remote_prefix: GuidPrefix,
        dest: &Locator,
    ) {
        use dds_rtps::{serialize_rtps_message, AckNack, Endianness, InfoDst, RtpsHeader, Submessage};

        let header = RtpsHeader::new(self.local_prefix);
        let pairs = [
            (
                EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
                EntityId::SEDP_BUILTIN_PUBLICATIONS_READER,
            ),
            (
                EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER,
                EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER,
            ),
        ];
        for (writer_id, reader_id) in pairs {
            let ack = AckNack {
                reader_id,
                writer_id,
                reader_sn_state: vec![dds_types::guid::SequenceNumber(1)],
                count: 1,
            };
            let msg = serialize_rtps_message(
                &header,
                &[
                    Submessage::InfoDst(InfoDst {
                        guid_prefix: remote_prefix,
                    }),
                    Submessage::AckNack(ack),
                ],
                Endianness::LittleEndian,
            );
            let _ = transport.send(&msg, dest);
        }
    }

    /// Request built-in SEDP from a remote participant on every advertised metatraffic locator.
    pub fn request_remote_sedp_for_participant(
        &self,
        transport: &Arc<dds_rtps::UdpTransport>,
        remote_prefix: GuidPrefix,
        participant: &DiscoveredParticipant,
    ) {
        for dest in Self::sedp_request_destinations(participant) {
            self.request_remote_sedp(transport, remote_prefix, &dest);
        }
    }

    /// Announce the local participant via SPDP on metatraffic multicast.
    pub fn announce_local_participant(
        &self,
        transport: &Arc<dds_rtps::UdpTransport>,
        domain_id: u32,
        unicast_locators: &[Locator],
        metatraffic_unicast_locators: &[Locator],
        multicast_locators: &[Locator],
    ) -> Result<(), String> {
        let info = DiscoveredParticipant {
            guid_prefix: self.local_prefix,
            unicast_locators: unicast_locators.to_vec(),
            metatraffic_unicast_locators: metatraffic_unicast_locators.to_vec(),
            multicast_locators: multicast_locators.to_vec(),
            lease_duration: Duration::from_secs(100),
            last_contact: std::time::Instant::now(),
        };
        let payload = spdp_to_plcdr(&info, domain_id).map_err(|e| format!("{e:?}"))?;
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
        metatraffic_unicast_locators: Vec<Locator>,
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
                    metatraffic_unicast_locators: metatraffic_unicast_locators.clone(),
                    multicast_locators: multicast_locators.clone(),
                    lease_duration: Duration::from_secs(100),
                    last_contact: std::time::Instant::now(),
                };
                if let Ok(payload) = spdp_to_plcdr(&info, domain_id) {
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

    /// Announce a single local endpoint via SEDP on metatraffic multicast and optionally unicast.
    pub fn announce_endpoint(
        &self,
        transport: &Arc<dds_rtps::UdpTransport>,
        domain_id: u32,
        endpoint: &DiscoveredEndpoint,
    ) -> Result<(), String> {
        self.announce_sedp_endpoint(
            transport,
            domain_id,
            endpoint,
            &metatraffic_multicast_locator(domain_id),
        )?;
        for (remote_prefix, participant) in &self.discovered_participants {
            for dest in Self::sedp_request_destinations(participant) {
                let _ = self.announce_sedp_endpoint(transport, domain_id, endpoint, &dest);
            }
            self.request_remote_sedp_for_participant(transport, *remote_prefix, participant);
            self.announce_sedp_heartbeats_to_participant(transport, *remote_prefix, participant);
        }
        Ok(())
    }

    /// Announce all local endpoints to a newly discovered remote participant (unicast SEDP).
    pub fn announce_local_endpoints_to_participant(
        &self,
        transport: &Arc<dds_rtps::UdpTransport>,
        domain_id: u32,
        remote_prefix: GuidPrefix,
    ) {
        let Some(remote) = self.discovered_participants.get(&remote_prefix) else {
            return;
        };
        self.request_remote_sedp_for_participant(transport, remote_prefix, remote);
        let endpoints: Vec<DiscoveredEndpoint> = self.local_endpoints.values().cloned().collect();
        for endpoint in endpoints {
            for dest in Self::sedp_request_destinations(remote) {
                let _ = self.announce_sedp_endpoint(transport, domain_id, &endpoint, &dest);
            }
        }
        self.announce_sedp_heartbeats_to_participant(transport, remote_prefix, remote);
    }

    fn announce_sedp_endpoint(
        &self,
        transport: &Arc<dds_rtps::UdpTransport>,
        domain_id: u32,
        endpoint: &DiscoveredEndpoint,
        dest: &Locator,
    ) -> Result<(), String> {
        self.send_sedp_endpoint(
            transport,
            domain_id,
            endpoint,
            dest,
            EntityId::UNKNOWN,
            self.sedp_sn_for_endpoint(endpoint),
        )
    }

    fn send_sedp_endpoint(
        &self,
        transport: &Arc<dds_rtps::UdpTransport>,
        domain_id: u32,
        endpoint: &DiscoveredEndpoint,
        dest: &Locator,
        directed_reader: EntityId,
        writer_sn: SequenceNumber,
    ) -> Result<(), String> {
        use bytes::Bytes;
        use dds_rtps::{serialize_rtps_message, Data, Endianness, Submessage};

        let payload = sedp_to_plcdr(endpoint, domain_id).map_err(|e| format!("{e:?}"))?;
        let writer_id = if endpoint.qos_writer.is_some() {
            EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER
        } else {
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER
        };
        let data_sub = Data {
            reader_id: directed_reader,
            writer_id,
            writer_sn,
            inline_qos: None,
            serialized_payload: Bytes::from(payload),
        };
        let header = dds_rtps::RtpsHeader::new(self.local_prefix);
        let msg = serialize_rtps_message(
            &header,
            &[Submessage::Data(data_sub)],
            Endianness::LittleEndian,
        );
        transport.send(&msg, dest).map_err(|e| format!("{e:?}"))
    }

    fn sedp_sn_for_endpoint(&self, endpoint: &DiscoveredEndpoint) -> SequenceNumber {
        self.sedp_endpoint_sn
            .get(&endpoint.guid)
            .copied()
            .unwrap_or(SequenceNumber(1))
    }

    /// Re-send local SEDP endpoint samples in response to a built-in SEDP AckNack.
    ///
    /// OpenDDS (and some other stacks) only deliver endpoint discovery over reliable
    /// built-in writers after the remote reader NACKs missing sequence numbers.
    pub fn reply_to_builtin_sedp_acknack(
        &self,
        transport: &Arc<dds_rtps::UdpTransport>,
        domain_id: u32,
        remote_prefix: GuidPrefix,
        ack: &dds_rtps::AckNack,
        reply_to: &Locator,
    ) -> Result<(), String> {
        let is_publications = ack.writer_id == EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER;
        let is_subscriptions = ack.writer_id == EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER;
        if !is_publications && !is_subscriptions {
            return Ok(());
        }

        let mut endpoints: Vec<&DiscoveredEndpoint> = self
            .local_endpoints
            .values()
            .filter(|ep| {
                if is_publications {
                    ep.qos_writer.is_some()
                } else {
                    ep.qos_reader.is_some()
                }
            })
            .collect();
        endpoints.sort_by_key(|ep| ep.guid.to_bytes());

        for endpoint in endpoints {
            let sn = self.sedp_sn_for_endpoint(endpoint);
            if !ack.reader_sn_state.is_empty() && !ack.reader_sn_state.contains(&sn) {
                continue;
            }
            self.send_sedp_endpoint_directed(
                transport,
                domain_id,
                endpoint,
                reply_to,
                remote_prefix,
                ack.reader_id,
                sn,
            )?;
        }
        Ok(())
    }

    fn send_sedp_endpoint_directed(
        &self,
        transport: &Arc<dds_rtps::UdpTransport>,
        domain_id: u32,
        endpoint: &DiscoveredEndpoint,
        dest: &Locator,
        remote_prefix: GuidPrefix,
        directed_reader: EntityId,
        writer_sn: SequenceNumber,
    ) -> Result<(), String> {
        use bytes::Bytes;
        use dds_rtps::{serialize_rtps_message, Data, Endianness, InfoDst, Submessage};

        let payload = sedp_to_plcdr(endpoint, domain_id).map_err(|e| format!("{e:?}"))?;
        let writer_id = if endpoint.qos_writer.is_some() {
            EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER
        } else {
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER
        };
        let data_sub = Data {
            reader_id: directed_reader,
            writer_id,
            writer_sn,
            inline_qos: None,
            serialized_payload: Bytes::from(payload),
        };
        let header = dds_rtps::RtpsHeader::new(self.local_prefix);
        let msg = serialize_rtps_message(
            &header,
            &[
                Submessage::InfoDst(InfoDst {
                    guid_prefix: remote_prefix,
                }),
                Submessage::Data(data_sub),
            ],
            Endianness::LittleEndian,
        );
        transport.send(&msg, dest).map_err(|e| format!("{e:?}"))
    }

    fn send_sedp_heartbeat(
        &self,
        transport: &Arc<dds_rtps::UdpTransport>,
        writer_id: EntityId,
        directed_reader: EntityId,
        remote_prefix: GuidPrefix,
        dest: &Locator,
        first_sn: SequenceNumber,
        last_sn: SequenceNumber,
        count: i32,
    ) -> Result<(), String> {
        use dds_rtps::{serialize_rtps_message, Endianness, Heartbeat, InfoDst, RtpsHeader, Submessage};

        if last_sn.0 < first_sn.0 {
            return Ok(());
        }
        let hb = Heartbeat {
            reader_id: directed_reader,
            writer_id,
            first_sn,
            last_sn,
            count,
            flags: dds_rtps::FLAG_FINAL,
        };
        let header = RtpsHeader::new(self.local_prefix);
        let msg = serialize_rtps_message(
            &header,
            &[
                Submessage::InfoDst(InfoDst {
                    guid_prefix: remote_prefix,
                }),
                Submessage::Heartbeat(hb),
            ],
            Endianness::LittleEndian,
        );
        transport.send(&msg, dest).map_err(|e| format!("{e:?}"))
    }

    fn sedp_writer_range(
        &self,
        publications: bool,
    ) -> (SequenceNumber, SequenceNumber) {
        let mut sns: Vec<SequenceNumber> = self
            .local_endpoints
            .values()
            .filter(|ep| {
                if publications {
                    ep.qos_writer.is_some()
                } else {
                    ep.qos_reader.is_some()
                }
            })
            .map(|ep| self.sedp_sn_for_endpoint(ep))
            .collect();
        if sns.is_empty() {
            return (SequenceNumber(1), SequenceNumber(0));
        }
        sns.sort_by_key(|sn| sn.0);
        (*sns.first().unwrap(), *sns.last().unwrap())
    }

    /// Notify a remote participant that local built-in SEDP writers have samples.
    pub fn announce_sedp_heartbeats_to_participant(
        &self,
        transport: &Arc<dds_rtps::UdpTransport>,
        remote_prefix: GuidPrefix,
        participant: &DiscoveredParticipant,
    ) {
        static PUB_HB_COUNT: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(1);
        static SUB_HB_COUNT: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(1);
        let (pub_first, pub_last) = self.sedp_writer_range(true);
        let (sub_first, sub_last) = self.sedp_writer_range(false);
        for dest in Self::sedp_request_destinations(participant) {
            if pub_last.0 >= pub_first.0 {
                let _ = self.send_sedp_heartbeat(
                    transport,
                    EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
                    EntityId::SEDP_BUILTIN_PUBLICATIONS_READER,
                    remote_prefix,
                    &dest,
                    pub_first,
                    pub_last,
                    PUB_HB_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                );
            }
            if sub_last.0 >= sub_first.0 {
                let _ = self.send_sedp_heartbeat(
                    transport,
                    EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER,
                    EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER,
                    remote_prefix,
                    &dest,
                    sub_first,
                    sub_last,
                    SUB_HB_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                );
            }
        }
    }

    /// Spawn a background thread that periodically re-announces all local endpoints via SEDP.
    pub fn spawn_sedp_announcer(
        discovery: Arc<std::sync::Mutex<Self>>,
        interval: core::time::Duration,
        transport: Arc<dds_rtps::UdpTransport>,
        domain_id: u32,
    ) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || loop {
            let (endpoints, remote_participants) = {
                let disc = discovery.lock().unwrap();
                let endpoints: Vec<DiscoveredEndpoint> =
                    disc.local_endpoints.values().cloned().collect();
                let remote_participants: Vec<(GuidPrefix, DiscoveredParticipant)> = disc
                    .discovered_participants
                    .iter()
                    .map(|(prefix, participant)| (*prefix, participant.clone()))
                    .collect();
                (endpoints, remote_participants)
            };
            for endpoint in &endpoints {
                let disc = discovery.lock().unwrap();
                let _ = disc.announce_sedp_endpoint(
                    &transport,
                    domain_id,
                    endpoint,
                    &metatraffic_multicast_locator(domain_id),
                );
                for (remote_prefix, participant) in &remote_participants {
                    for dest in DiscoveryManager::sedp_request_destinations(participant) {
                        let _ = disc.announce_sedp_endpoint(&transport, domain_id, endpoint, &dest);
                    }
                    disc.request_remote_sedp_for_participant(&transport, *remote_prefix, participant);
                }
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
    pub fn remove_participant(&mut self, prefix: &GuidPrefix) -> Vec<(Guid, DiscoveredEndpoint)> {
        let removed_endpoints: Vec<(Guid, DiscoveredEndpoint)> = self
            .discovered_endpoints
            .iter()
            .filter(|(guid, _)| &guid.prefix == prefix)
            .map(|(guid, ep)| (*guid, ep.clone()))
            .collect();
        self.discovered_participants.remove(prefix);
        self.discovered_endpoints
            .retain(|guid, _| &guid.prefix != prefix);
        for guids in self.builtin_mappings.values_mut() {
            guids.retain(|guid| &guid.prefix != prefix);
        }
        removed_endpoints
    }

    /// Clean up expired participants based on lease duration.
    ///
    /// Returns removed participant prefixes and their endpoints for unmatch handling.
    pub fn check_lease_timeouts(&mut self) -> Vec<(GuidPrefix, Vec<(Guid, DiscoveredEndpoint)>)> {
        let now = std::time::Instant::now();
        let mut expired = Vec::new();

        for (prefix, participant) in &self.discovered_participants {
            if let Some(std_dur) = participant.lease_duration.to_std() {
                if now.duration_since(participant.last_contact) > std_dur {
                    expired.push(*prefix);
                }
            }
        }

        let mut removed = Vec::new();
        for prefix in expired {
            let endpoints = self.remove_participant(&prefix);
            removed.push((prefix, endpoints));
        }
        removed
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

/// Serialize parameters using standard DDSI-RTPS §9.4.2 PL-CDR (CycloneDDS-compatible).
fn serialize_rtps_plcdr(parameters: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    // PlCdrLe encapsulation header (0x0003) — match CycloneDDS wire order [0x00, 0x03, 0x00, 0x00].
    out.extend_from_slice(&[0x00, 0x03, 0x00, 0x00]);
    for (pid, value) in parameters {
        let padded = (value.len() + 3) & !3;
        out.extend_from_slice(&pid.to_le_bytes());
        out.extend_from_slice(&(padded as u16).to_le_bytes());
        out.extend_from_slice(value);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
    }
    out.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
    out
}

fn append_locator_param(parameters: &mut Vec<(u16, Vec<u8>)>, pid: u16, locator: &Locator) {
    let mut loc_bytes = Vec::new();
    loc_bytes.extend_from_slice(&(locator.kind as i32).to_le_bytes());
    loc_bytes.extend_from_slice(&locator.port.to_le_bytes());
    loc_bytes.extend_from_slice(&locator.address);
    parameters.push((pid, loc_bytes));
}

fn append_plcdr_string(parameters: &mut Vec<(u16, Vec<u8>)>, pid: u16, value: &str) {
    let mut bytes = Vec::new();
    append_plcdr_string_value(&mut bytes, value);
    parameters.push((pid, bytes));
}

fn append_plcdr_string_value(out: &mut Vec<u8>, value: &str) {
    let len = u32::try_from(value.len() + 1).unwrap_or(u32::MAX);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    out.push(0);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

fn append_plcdr_string_sequence(out: &mut Vec<u8>, values: &[String]) {
    out.extend_from_slice(&(values.len() as u32).to_le_bytes());
    for value in values {
        append_plcdr_string_value(out, value);
    }
}

fn parse_plcdr_string(bytes: &[u8]) -> Option<String> {
    if bytes.len() >= 4 {
        let len = u32::from_le_bytes(bytes[0..4].try_into().ok()?) as usize;
        if len >= 1 && 4 + len <= bytes.len() {
            return Some(String::from_utf8_lossy(&bytes[4..4 + len - 1]).into_owned());
        }
    }
    std::ffi::CStr::from_bytes_until_nul(bytes)
        .ok()
        .map(|s| s.to_string_lossy().into_owned())
}

fn append_participant_guid(parameters: &mut Vec<(u16, Vec<u8>)>, prefix: GuidPrefix) {
    let mut guid_bytes = Vec::new();
    guid_bytes.extend_from_slice(prefix.as_bytes());
    guid_bytes.extend_from_slice(&EntityId::PARTICIPANT.0);
    parameters.push((PID_PARTICIPANT_GUID, guid_bytes));
}

fn append_protocol_vendor_domain(
    parameters: &mut Vec<(u16, Vec<u8>)>,
    domain_id: u32,
) {
    parameters.push((PID_PROTOCOL_VERSION, [2u8, 1, 0, 0].to_vec()));
    parameters.push((
        PID_VENDOR_ID,
        dds_types::vendor::VendorId::THIS_IMPLEMENTATION.0.to_vec(),
    ));
    parameters.push((PID_DOMAIN_ID, domain_id.to_le_bytes().to_vec()));
}

/// Serializes a `DiscoveredParticipant` to a PL-CDR parameter list.
///
/// Reference: RTPS §9.6.3 — ParameterList values
pub fn spdp_to_plcdr(
    participant: &DiscoveredParticipant,
    domain_id: u32,
) -> dds_cdr::CdrResult<Vec<u8>> {
    let mut parameters = Vec::new();

    append_protocol_vendor_domain(&mut parameters, domain_id);

    // BuiltinEndpointSet — CycloneDDS rejects SPDP without this (see ddsi_discovery_spdp.c).
    const BUILTIN_ENDPOINT_SET: u32 = 0x0000_FC3F;
    parameters.push((
        PID_BUILTIN_ENDPOINT_SET,
        BUILTIN_ENDPOINT_SET.to_le_bytes().to_vec(),
    ));

    // Participant GUID (0x0050)
    let mut guid_bytes = Vec::new();
    guid_bytes.extend_from_slice(participant.guid_prefix.as_bytes());
    guid_bytes.extend_from_slice(&dds_types::guid::EntityId::PARTICIPANT.0);
    parameters.push((PID_PARTICIPANT_GUID, guid_bytes));

    // Lease Duration (0x0002)
    let mut lease_bytes = Vec::new();
    append_rtps_duration_bytes(&mut lease_bytes, participant.lease_duration);
    parameters.push((PID_LEASE_DURATION, lease_bytes));

    // Unicast / multicast locators
    for locator in &participant.metatraffic_unicast_locators {
        append_locator_param(&mut parameters, PID_METATRAFFIC_UNICAST_LOCATOR, locator);
    }
    for locator in &participant.unicast_locators {
        append_locator_param(&mut parameters, PID_DEFAULT_UNICAST_LOCATOR, locator);
    }

    for locator in &participant.multicast_locators {
        append_locator_param(&mut parameters, PID_DEFAULT_MULTICAST_LOCATOR, locator);
        append_locator_param(&mut parameters, PID_METATRAFFIC_MULTICAST_LOCATOR, locator);
    }

    Ok(serialize_rtps_plcdr(&parameters))
}

/// Strip RTPS PL-CDR encapsulation header when present (CycloneDDS / FastDDS).
fn strip_plcdr_encapsulation(bytes: &[u8]) -> &[u8] {
    if bytes.len() >= 4 {
        let le = u16::from_le_bytes([bytes[0], bytes[1]]);
        let be = u16::from_be_bytes([bytes[0], bytes[1]]);
        if le <= 0x0013 || be <= 0x0013 {
            return &bytes[4..];
        }
    }
    bytes
}

/// Parse DDSI-RTPS §9.4.2 parameter list wire format from external implementations.
fn parse_rtps_parameter_list(bytes: &[u8]) -> Option<dds_cdr::ParameterList> {
    use dds_cdr::{ParameterId, ParameterList};

    let body = strip_plcdr_encapsulation(bytes);
    let mut offset = 0;
    let mut plist = ParameterList::new();

    while offset + 4 <= body.len() {
        let pid = u16::from_le_bytes(body[offset..offset + 2].try_into().ok()?);
        let length = u16::from_le_bytes(body[offset + 2..offset + 4].try_into().ok()?) as usize;
        offset += 4;

        if pid == 0x0001 {
            break;
        }
        if pid == 0x0000 {
            let padded = (length + 3) & !3;
            offset = offset.saturating_add(padded);
            continue;
        }
        if offset + length > body.len() {
            return None;
        }
        let value = body[offset..offset + length].to_vec();
        plist.add(ParameterId(pid), value);
        offset += (length + 3) & !3;
    }

    if plist.parameters.is_empty() {
        None
    } else {
        Some(plist)
    }
}

fn decode_discovery_plcdr(bytes: &[u8]) -> Option<dds_cdr::ParameterList> {
    parse_rtps_parameter_list(bytes)
}

fn parse_locator_param(value: &[u8]) -> Option<Locator> {
    if value.len() >= 24 {
        let kind_val = i32::from_le_bytes(value[0..4].try_into().ok()?);
        let port = u32::from_le_bytes(value[4..8].try_into().ok()?);
        let mut address = [0u8; 16];
        address.copy_from_slice(&value[8..24]);
        Some(Locator {
            kind: dds_types::locator::LocatorKind::from_i32(kind_val),
            port,
            address,
        })
    } else {
        None
    }
}

/// Parses a `DiscoveredParticipant` from a PL-CDR parameter list byte buffer.
pub fn parse_spdp_packet(bytes: &[u8]) -> Option<DiscoveredParticipant> {
    use dds_cdr::ParameterList;

    let plist: ParameterList = decode_discovery_plcdr(bytes)?;

    let mut guid_prefix = GuidPrefix::UNKNOWN;
    let mut lease_duration = Duration::INFINITE;
    let mut unicast_locators = Vec::new();
    let mut metatraffic_unicast_locators = Vec::new();
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
                    let fraction = u32::from_le_bytes(param.value[4..8].try_into().ok()?);
                    lease_duration = Duration::from_rtps_wire(seconds, fraction);
                }
            }
            PID_DEFAULT_UNICAST_LOCATOR | PID_UNICAST_LOCATOR => {
                if let Some(loc) = parse_locator_param(&param.value) {
                    unicast_locators.push(loc);
                }
            }
            PID_METATRAFFIC_UNICAST_LOCATOR => {
                if let Some(loc) = parse_locator_param(&param.value) {
                    metatraffic_unicast_locators.push(loc);
                }
            }
            PID_DEFAULT_MULTICAST_LOCATOR | PID_MULTICAST_LOCATOR => {
                if let Some(loc) = parse_locator_param(&param.value) {
                    multicast_locators.push(loc);
                }
            }
            PID_METATRAFFIC_MULTICAST_LOCATOR => {
                if let Some(loc) = parse_locator_param(&param.value) {
                    multicast_locators.push(loc);
                }
            }
            _ => {}
        }
    }

    if guid_prefix.is_unknown() {
        return None;
    }

    unicast_locators = DiscoveryManager::filter_valid_unicast_locators(&unicast_locators);
    metatraffic_unicast_locators =
        DiscoveryManager::filter_valid_unicast_locators(&metatraffic_unicast_locators);
    multicast_locators = DiscoveryManager::filter_valid_unicast_locators(&multicast_locators);

    if metatraffic_unicast_locators.is_empty() && !unicast_locators.is_empty() {
        metatraffic_unicast_locators = unicast_locators.clone();
    }
    if unicast_locators.is_empty() && !metatraffic_unicast_locators.is_empty() {
        unicast_locators = metatraffic_unicast_locators.clone();
    }
    multicast_locators.sort_by_key(|l| (l.port, l.address));
    multicast_locators.dedup();

    Some(DiscoveredParticipant {
        guid_prefix,
        unicast_locators,
        metatraffic_unicast_locators,
        multicast_locators,
        lease_duration,
        last_contact: std::time::Instant::now(),
    })
}

fn append_rtps_duration_bytes(bytes: &mut Vec<u8>, duration: Duration) {
    let (seconds, fraction) = duration.to_rtps_wire();
    bytes.extend_from_slice(&seconds.to_le_bytes());
    bytes.extend_from_slice(&fraction.to_le_bytes());
}

fn append_reliability_qos(parameters: &mut Vec<(u16, Vec<u8>)>, qos: &dds_types::qos::Reliability) {
    let reliability_val = match qos.kind {
        dds_types::qos::ReliabilityKind::BestEffort => 1u32,
        dds_types::qos::ReliabilityKind::Reliable => 2,
    };
    let mut rel_bytes = Vec::new();
    rel_bytes.extend_from_slice(&reliability_val.to_le_bytes());
    append_rtps_duration_bytes(&mut rel_bytes, qos.max_blocking_time);
    parameters.push((PID_RELIABILITY, rel_bytes));
}

fn append_durability_qos(parameters: &mut Vec<(u16, Vec<u8>)>, kind: dds_types::qos::DurabilityKind) {
    let durability_val = match kind {
        dds_types::qos::DurabilityKind::Volatile => 0u32,
        dds_types::qos::DurabilityKind::TransientLocal => 1,
        dds_types::qos::DurabilityKind::Transient => 2,
        dds_types::qos::DurabilityKind::Persistent => 3,
    };
    parameters.push((PID_DURABILITY, durability_val.to_le_bytes().to_vec()));
}

fn append_history_qos(parameters: &mut Vec<(u16, Vec<u8>)>, history: &dds_types::qos::History) {
    let kind_val = match history.kind {
        dds_types::qos::HistoryKind::KeepLast => 0u32,
        dds_types::qos::HistoryKind::KeepAll => 1,
    };
    let mut hist_bytes = Vec::new();
    hist_bytes.extend_from_slice(&kind_val.to_le_bytes());
    hist_bytes.extend_from_slice(&history.depth.to_le_bytes());
    parameters.push((PID_HISTORY, hist_bytes));
}

fn append_liveliness_qos(parameters: &mut Vec<(u16, Vec<u8>)>, liveliness: &dds_types::qos::Liveliness) {
    let kind_val = match liveliness.kind {
        dds_types::qos::LivelinessKind::Automatic => 0u32,
        dds_types::qos::LivelinessKind::ManualByParticipant => 1,
        dds_types::qos::LivelinessKind::ManualByTopic => 2,
    };
    let mut live_bytes = Vec::new();
    live_bytes.extend_from_slice(&kind_val.to_le_bytes());
    append_rtps_duration_bytes(&mut live_bytes, liveliness.lease_duration);
    parameters.push((PID_LIVELINESS, live_bytes));
}

fn append_data_representation_qos(parameters: &mut Vec<(u16, Vec<u8>)>) {
    // Match CycloneDDS default: XCDR1 + XCDR2.
    let ids = [DATA_REPRESENTATION_XCDR1, DATA_REPRESENTATION_XCDR2];
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(ids.len() as u32).to_le_bytes());
    for id in ids {
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.extend_from_slice(&[0u8, 0u8]); // align uint16 to 4 bytes
    }
    parameters.push((PID_DATA_REPRESENTATION, bytes));
}

fn append_type_information_param(parameters: &mut Vec<(u16, Vec<u8>)>, wire: &[u8]) {
    parameters.push((PID_TYPE_INFORMATION, wire.to_vec()));
}

fn apply_durability_param(
    writer_qos: &mut dds_types::qos::DataWriterQos,
    reader_qos: &mut dds_types::qos::DataReaderQos,
    kind_val: u32,
) {
    let kind = match kind_val {
        0 => dds_types::qos::DurabilityKind::Volatile,
        1 => dds_types::qos::DurabilityKind::TransientLocal,
        2 => dds_types::qos::DurabilityKind::Transient,
        3 => dds_types::qos::DurabilityKind::Persistent,
        _ => dds_types::qos::DurabilityKind::Volatile,
    };
    writer_qos.durability.kind = kind;
    reader_qos.durability.kind = kind;
}

fn apply_reliability_param(
    writer_qos: &mut dds_types::qos::DataWriterQos,
    reader_qos: &mut dds_types::qos::DataReaderQos,
    value: &[u8],
) {
    if value.len() >= 4 {
        let kind_val = u32::from_le_bytes(value[0..4].try_into().unwrap_or([0; 4]));
        let kind = match kind_val {
            1 => dds_types::qos::ReliabilityKind::BestEffort,
            2 => dds_types::qos::ReliabilityKind::Reliable,
            _ => dds_types::qos::ReliabilityKind::BestEffort,
        };
        writer_qos.reliability.kind = kind;
        reader_qos.reliability.kind = kind;
    }
    if value.len() >= 12 {
        let seconds = i32::from_le_bytes(value[4..8].try_into().unwrap_or([0; 4]));
        let fraction = u32::from_le_bytes(value[8..12].try_into().unwrap_or([0; 4]));
        let duration = Duration::from_rtps_wire(seconds, fraction);
        writer_qos.reliability.max_blocking_time = duration;
        reader_qos.reliability.max_blocking_time = duration;
    }
}

fn apply_history_param(
    writer_qos: &mut dds_types::qos::DataWriterQos,
    reader_qos: &mut dds_types::qos::DataReaderQos,
    value: &[u8],
) {
    if value.len() >= 4 {
        let kind_val = u32::from_le_bytes(value[0..4].try_into().unwrap_or([0; 4]));
        let kind = match kind_val {
            0 => dds_types::qos::HistoryKind::KeepLast,
            1 => dds_types::qos::HistoryKind::KeepAll,
            _ => dds_types::qos::HistoryKind::KeepLast,
        };
        writer_qos.history.kind = kind;
        reader_qos.history.kind = kind;
    }
    if value.len() >= 8 {
        let depth = i32::from_le_bytes(value[4..8].try_into().unwrap_or([0; 4]));
        writer_qos.history.depth = depth;
        reader_qos.history.depth = depth;
    }
}

fn apply_liveliness_param(
    writer_qos: &mut dds_types::qos::DataWriterQos,
    reader_qos: &mut dds_types::qos::DataReaderQos,
    value: &[u8],
) {
    if value.len() >= 4 {
        let kind_val = u32::from_le_bytes(value[0..4].try_into().unwrap_or([0; 4]));
        let kind = match kind_val {
            0 => dds_types::qos::LivelinessKind::Automatic,
            1 => dds_types::qos::LivelinessKind::ManualByParticipant,
            2 => dds_types::qos::LivelinessKind::ManualByTopic,
            _ => dds_types::qos::LivelinessKind::Automatic,
        };
        writer_qos.liveliness.kind = kind;
        reader_qos.liveliness.kind = kind;
    }
    if value.len() >= 12 {
        let seconds = i32::from_le_bytes(value[4..8].try_into().unwrap_or([0; 4]));
        let fraction = u32::from_le_bytes(value[8..12].try_into().unwrap_or([0; 4]));
        let duration = Duration::from_rtps_wire(seconds, fraction);
        writer_qos.liveliness.lease_duration = duration;
        reader_qos.liveliness.lease_duration = duration;
    }
}

/// Serializes a `DiscoveredEndpoint` to a PL-CDR parameter list.
pub fn sedp_to_plcdr(
    endpoint: &DiscoveredEndpoint,
    _domain_id: u32,
) -> dds_cdr::CdrResult<Vec<u8>> {
    let mut parameters = Vec::new();

    append_plcdr_string(&mut parameters, PID_TOPIC_NAME, &endpoint.topic_name);
    if let Some(ref wire) = endpoint.type_information_wire {
        append_type_information_param(&mut parameters, wire);
    }
    append_plcdr_string(&mut parameters, PID_TYPE_NAME, &endpoint.type_name);

    if let Some(ref qos) = endpoint.qos_writer {
        append_reliability_qos(&mut parameters, &qos.reliability);
    } else if let Some(ref qos) = endpoint.qos_reader {
        append_reliability_qos(&mut parameters, &qos.reliability);
    }

    append_data_representation_qos(&mut parameters);

    let mut guid_bytes = Vec::with_capacity(16);
    guid_bytes.extend_from_slice(&endpoint.guid.prefix.0);
    guid_bytes.extend_from_slice(&endpoint.guid.entity_id.0);
    parameters.push((PID_ENDPOINT_GUID, guid_bytes));

    for locator in &endpoint.multicast_locators {
        append_locator_param(&mut parameters, PID_MULTICAST_LOCATOR, locator);
    }
    for locator in &endpoint.unicast_locators {
        append_locator_param(&mut parameters, PID_UNICAST_LOCATOR, locator);
        append_locator_param(&mut parameters, PID_UNICAST_LOCATOR, locator);
    }

    Ok(serialize_rtps_plcdr(&parameters))
}

/// Parses an SEDP PL-CDR parameter list into a `DiscoveredEndpoint`.
pub fn parse_sedp_packet(bytes: &[u8]) -> Option<DiscoveredEndpoint> {
    use dds_cdr::ParameterList;
    let plist: ParameterList = decode_discovery_plcdr(bytes)?;

    let mut guid = dds_types::guid::Guid::new(
        GuidPrefix::new([0; 12]),
        dds_types::guid::EntityId::new([0; 4]),
    );
    let mut topic_name = String::new();
    let mut type_name = String::new();
    let mut writer_qos = dds_types::qos::DataWriterQos::default();
    let mut reader_qos = dds_types::qos::DataReaderQos::default();
    let mut partition = Vec::new();
    let mut unicast_locators = Vec::new();
    let mut metatraffic_unicast_locators = Vec::new();
    let mut multicast_locators = Vec::new();

    for param in &plist.parameters {
        match param.parameter_id.0 {
            PID_ENDPOINT_GUID => {
                if param.value.len() >= 16 {
                    let mut prefix_bytes = [0u8; 12];
                    prefix_bytes.copy_from_slice(&param.value[0..12]);
                    let mut entity_bytes = [0u8; 4];
                    entity_bytes.copy_from_slice(&param.value[12..16]);
                    guid = dds_types::guid::Guid::new(
                        GuidPrefix::new(prefix_bytes),
                        dds_types::guid::EntityId::new(entity_bytes),
                    );
                }
            }
            PID_TOPIC_NAME => {
                if let Some(name) = parse_plcdr_string(&param.value) {
                    topic_name = name;
                }
            }
            PID_TYPE_NAME => {
                if let Some(name) = parse_plcdr_string(&param.value) {
                    type_name = name;
                }
            }
            PID_DURABILITY => {
                if param.value.len() >= 4 {
                    let kind_val = u32::from_le_bytes([
                        param.value[0],
                        param.value[1],
                        param.value[2],
                        param.value[3],
                    ]);
                    apply_durability_param(&mut writer_qos, &mut reader_qos, kind_val);
                }
            }
            PID_RELIABILITY => {
                apply_reliability_param(&mut writer_qos, &mut reader_qos, &param.value);
            }
            PID_HISTORY => {
                apply_history_param(&mut writer_qos, &mut reader_qos, &param.value);
            }
            PID_LIVELINESS => {
                apply_liveliness_param(&mut writer_qos, &mut reader_qos, &param.value);
            }
            PID_PARTITION => {
                let mut offset = 0;
                if param.value.len() >= 4 {
                    let count = u32::from_le_bytes([
                        param.value[0],
                        param.value[1],
                        param.value[2],
                        param.value[3],
                    ]) as usize;
                    offset = 4;
                    for _ in 0..count {
                        if offset + 4 > param.value.len() {
                            break;
                        }
                        if let Some(name) = parse_plcdr_string(&param.value[offset..]) {
                            let len = u32::from_le_bytes([
                                param.value[offset],
                                param.value[offset + 1],
                                param.value[offset + 2],
                                param.value[offset + 3],
                            ]) as usize;
                            partition.push(name);
                            offset += ((4 + len + 3) / 4) * 4;
                        } else {
                            break;
                        }
                    }
                }
            }
            PID_UNICAST_LOCATOR | PID_DEFAULT_UNICAST_LOCATOR => {
                if let Some(loc) = parse_locator_param(&param.value) {
                    unicast_locators.push(loc);
                }
            }
            PID_METATRAFFIC_UNICAST_LOCATOR => {
                if let Some(loc) = parse_locator_param(&param.value) {
                    metatraffic_unicast_locators.push(loc);
                }
            }
            PID_MULTICAST_LOCATOR
            | PID_DEFAULT_MULTICAST_LOCATOR
            | PID_METATRAFFIC_MULTICAST_LOCATOR => {
                if let Some(loc) = parse_locator_param(&param.value) {
                    multicast_locators.push(loc);
                }
            }
            _ => {}
        }
    }

    if topic_name.is_empty() || type_name.is_empty() {
        return None;
    }

    let (qos_writer, qos_reader) = if is_reader_entity(&guid.entity_id) {
        (None, Some(reader_qos))
    } else if is_writer_entity(&guid.entity_id) {
        (Some(writer_qos), None)
    } else {
        (None, None)
    };

    let unicast_locators = DiscoveryManager::filter_valid_unicast_locators(&unicast_locators);
    let metatraffic_unicast_locators =
        DiscoveryManager::filter_valid_unicast_locators(&metatraffic_unicast_locators);
    let multicast_locators = DiscoveryManager::filter_valid_unicast_locators(&multicast_locators);

    Some(DiscoveredEndpoint {
        guid,
        topic_name,
        type_name,
        qos_writer,
        qos_reader,
        partition,
        unicast_locators,
        metatraffic_unicast_locators,
        multicast_locators,
        type_info: None,
        type_information_wire: None,
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
            metatraffic_unicast_locators: vec![],
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
            metatraffic_unicast_locators: vec![],
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
            unicast_locators: vec![],
            metatraffic_unicast_locators: vec![],
            multicast_locators: vec![],
            type_info: None,
            type_information_wire: None,
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
            metatraffic_unicast_locators: vec![],
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
            metatraffic_unicast_locators: vec![],
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
            unicast_locators: vec![],
            metatraffic_unicast_locators: vec![],
            multicast_locators: vec![],
            type_info: None,
            type_information_wire: None,
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
                Locator::udpv4(std::net::Ipv4Addr::new(127, 0, 0, 1), PORT_BASE as u32 + 11),
            ],
            metatraffic_unicast_locators: vec![
                Locator::udpv4(std::net::Ipv4Addr::new(127, 0, 0, 1), PORT_BASE as u32 + 10),
            ],
            multicast_locators: vec![
                Locator::udpv4(std::net::Ipv4Addr::new(DEFAULT_MULTICAST_IP[0], DEFAULT_MULTICAST_IP[1], DEFAULT_MULTICAST_IP[2], DEFAULT_MULTICAST_IP[3]), PORT_BASE as u32),
            ],
            lease_duration: Duration::from_secs(120),
            last_contact: std::time::Instant::now(),
        };

        let bytes = spdp_to_plcdr(&participant, 0).unwrap();
        assert_eq!(bytes[0..4], [0x00, 0x03, 0x00, 0x00]);
        let decoded = parse_spdp_packet(&bytes).unwrap();

        assert_eq!(decoded.guid_prefix, participant.guid_prefix);
        assert_eq!(decoded.lease_duration, participant.lease_duration);
        assert_eq!(decoded.unicast_locators.len(), 1);
        assert_eq!(decoded.unicast_locators[0].port, PORT_BASE as u32 + 11);
        assert_eq!(decoded.metatraffic_unicast_locators.len(), 1);
        assert_eq!(decoded.metatraffic_unicast_locators[0].port, PORT_BASE as u32 + 10);
        assert_eq!(decoded.multicast_locators.len(), 1);
        assert_eq!(decoded.multicast_locators[0].port, PORT_BASE as u32);
    }

    #[test]
    fn test_plcdr_encapsulation_header_is_little_endian() {
        let participant = DiscoveredParticipant {
            guid_prefix: GuidPrefix::new([9; 12]),
            unicast_locators: vec![],
            metatraffic_unicast_locators: vec![],
            multicast_locators: vec![],
            lease_duration: Duration::from_secs(30),
            last_contact: std::time::Instant::now(),
        };
        let bytes = spdp_to_plcdr(&participant, 0).unwrap();
        assert_eq!(bytes[0..4], [0x00, 0x03, 0x00, 0x00]);
    }

    #[test]
    fn test_sedp_roundtrip_history_and_liveliness() {
        let mut writer_qos = dds_types::qos::DataWriterQos::default();
        writer_qos.history = dds_types::qos::History {
            kind: dds_types::qos::HistoryKind::KeepAll,
            depth: 7,
        };
        writer_qos.liveliness = dds_types::qos::Liveliness {
            kind: dds_types::qos::LivelinessKind::ManualByTopic,
            lease_duration: Duration::from_secs(5),
        };
        let endpoint = DiscoveredEndpoint {
            guid: Guid::new(
                GuidPrefix::new([3; 12]),
                dds_types::guid::EntityId::new([0, 0, 1, 0x02]),
            ),
            topic_name: "HistTopic".into(),
            type_name: "HistType".into(),
            qos_writer: Some(writer_qos.clone()),
            qos_reader: None,
            partition: vec![],
            unicast_locators: vec![],
            metatraffic_unicast_locators: vec![],
            multicast_locators: vec![],
            type_info: None,
            type_information_wire: None,
        };
        let bytes = sedp_to_plcdr(&endpoint, 0).unwrap();
        let decoded = parse_sedp_packet(&bytes).unwrap();
        let decoded_qos = decoded.qos_writer.expect("writer qos");
        assert_eq!(decoded_qos.history.kind, dds_types::qos::HistoryKind::KeepAll);
        assert_eq!(decoded_qos.history.depth, 7);
        assert_eq!(
            decoded_qos.liveliness.kind,
            dds_types::qos::LivelinessKind::ManualByTopic
        );
        assert_eq!(decoded_qos.liveliness.lease_duration.seconds, 5);
    }

    #[test]
    fn test_sedp_qos_before_guid_still_applied() {
        let mut bytes = vec![0x00, 0x03, 0x00, 0x00]; // PlCdrLe (Cyclone wire order)
        // PID_HISTORY KeepAll depth 3
        bytes.extend_from_slice(&PID_HISTORY.to_le_bytes());
        bytes.extend_from_slice(&8u16.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&3i32.to_le_bytes());
        // PID_ENDPOINT_GUID reader
        bytes.extend_from_slice(&PID_ENDPOINT_GUID.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        let mut guid = [0u8; 16];
        guid[11] = 4;
        guid[15] = 0x07;
        bytes.extend_from_slice(&guid);
        // PID_TOPIC_NAME
        bytes.extend_from_slice(&PID_TOPIC_NAME.to_le_bytes());
        bytes.extend_from_slice(&14u16.to_le_bytes());
        bytes.extend_from_slice(b"QosFirstTopic\0");
        bytes.extend_from_slice(&[0, 0]); // pad topic to 16 bytes
        // PID_TYPE_NAME
        bytes.extend_from_slice(&PID_TYPE_NAME.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"QosFirstType\0");
        bytes.extend_from_slice(&[0, 0, 0]); // pad type to 16 bytes
        bytes.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // PID_SENTINEL

        let decoded = parse_sedp_packet(&bytes).unwrap();
        let qos = decoded.qos_reader.expect("reader qos");
        assert_eq!(qos.history.kind, dds_types::qos::HistoryKind::KeepAll);
        assert_eq!(qos.history.depth, 3);
    }

    #[test]
    fn test_parse_fastdds_interop_sedp_capture() {
        let path = std::path::Path::new("/tmp/fastdds_sedp_sub.bin");
        if !path.exists() {
            return;
        }
        let bytes = std::fs::read(path).expect("read capture");
        let decoded = parse_sedp_packet(&bytes).expect("parse FastDDS SEDP");
        assert_eq!(decoded.topic_name, "AiDdsInteropMessage");
        assert_eq!(decoded.type_name, "AiDdsInterop::Message");
        assert!(decoded.qos_reader.is_some());
        assert!(is_reader_entity(&decoded.guid.entity_id));
    }

    #[test]
    fn test_typelookup_service() {
        let local_prefix = GuidPrefix::new([1; 12]);
        let mut manager = DiscoveryManager::new(local_prefix);
        let remote_prefix = GuidPrefix::new([2; 12]);

        let remote_participant = DiscoveredParticipant {
            guid_prefix: remote_prefix,
            unicast_locators: vec![],
            metatraffic_unicast_locators: vec![],
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
            unicast_locators: vec![],
            metatraffic_unicast_locators: vec![],
            multicast_locators: vec![],
            type_info: Some(dds_xtypes::TypeInformation {
                type_name: "MyInt".to_string(),
                type_id: r_id.clone(),
            }),
            type_information_wire: None,
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
    fn test_sedp_reliability_uses_rtps_duration_fraction() {
        let mut qos = DataWriterQos::default();
        qos.reliability.kind = dds_types::qos::ReliabilityKind::Reliable;
        qos.reliability.max_blocking_time = Duration::from_millis(100);
        let endpoint = DiscoveredEndpoint {
            guid: Guid::new(GuidPrefix::new([4; 12]), EntityId::new([0, 0, 1, 0x03])),
            topic_name: "RelTopic".into(),
            type_name: "RelType".into(),
            qos_writer: Some(qos),
            qos_reader: None,
            partition: vec![],
            unicast_locators: vec![],
            metatraffic_unicast_locators: vec![],
            multicast_locators: vec![],
            type_info: None,
            type_information_wire: None,
        };
        let bytes = sedp_to_plcdr(&endpoint, 0).unwrap();
        let parsed = parse_sedp_packet(&bytes).unwrap();
        let writer_qos = parsed.qos_writer.expect("writer qos");
        assert_eq!(
            writer_qos.reliability.max_blocking_time,
            Duration::from_millis(100)
        );
        assert_eq!(writer_qos.reliability.kind, dds_types::qos::ReliabilityKind::Reliable);
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
            unicast_locators: vec![],
            metatraffic_unicast_locators: vec![],
            multicast_locators: vec![],
            type_info: None,
            type_information_wire: None,
        };
        let bytes = sedp_to_plcdr(&endpoint, 0).unwrap();
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
            metatraffic_unicast_locators: vec![],
            multicast_locators: vec![],
            lease_duration: Duration::from_secs(10),
            last_contact: std::time::Instant::now(),
        });
        let snapshot = manager.monitor_snapshot();
        assert_eq!(snapshot.participants.len(), 1);
        assert!(snapshot.endpoints.is_empty());
    }
}
