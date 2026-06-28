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
    pub fn spawn_spdp_announcer<F>(
        &self,
        interval: core::time::Duration,
        broadcast_fn: F,
    ) -> std::thread::JoinHandle<()>
    where
        F: Fn(DiscoveredParticipant) + Send + Sync + 'static,
    {
        let local_prefix = self.local_prefix;
        std::thread::spawn(move || loop {
            let info = DiscoveredParticipant {
                guid_prefix: local_prefix,
                unicast_locators: vec![],
                multicast_locators: vec![],
                lease_duration: Duration::from_secs(100),
                last_contact: std::time::Instant::now(),
            };
            broadcast_fn(info);
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
        )
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use dds_types::guid::EntityId;
    use std::sync::{Arc, Mutex};

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
        let count = Arc::new(Mutex::new(0));
        let count_clone = count.clone();
        let _handle =
            manager.spawn_spdp_announcer(std::time::Duration::from_millis(10), move |_p| {
                let mut c = count_clone.lock().unwrap();
                *c += 1;
            });

        // Wait a tiny bit and confirm callbacks are triggered
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(*count.lock().unwrap() > 0);
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
        let r_obj = dds_xtypes::TypeObject::Primitive("long".to_string());
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
