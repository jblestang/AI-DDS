//! # dds-core — DDS DCPS 1.4 API
//!
//! The user-facing API implementing the Data-Centric Publish-Subscribe
//! model. Provides `DomainParticipant`, Publisher, Subscriber, `DataWriter`,
//! `DataReader`, Topic, `WaitSet`, and Conditions.
//!
//! The middleware is fully type-erased — it operates on `SerializedPayload`
//! and `TypeSupport` traits, never on concrete user types.
//!
//! Reference: DCPS §2.2

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
    reason = "DDS Core implementation requires standard library conversions, standard returns, and type erasure mechanics."
)]

use dds_rtps::{
    CacheChange, ChangeKind, RtpsEngine,
    StatefulWriter, Submessage, UdpTransport,
    parse_rtps_message,
};
use dds_types::guid::{EntityId, EntityKind, Guid, GuidPrefix, SequenceNumber};
use dds_types::locator::Locator;
use dds_types::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, PublisherQos, SubscriberQos, TopicQos,
};
use dds_types::return_code::{DdsError, DdsResult};
use dds_security::{Authentication, Cryptography};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

// ──────────────────────────────────────────────────────────────────────────────
// DDS Core Constants
// ──────────────────────────────────────────────────────────────────────────────

/// RTPS Base Port number (PB)
pub const PORT_BASE: u16 = 7400;

/// RTPS Domain ID Gain (DG)
pub const DOMAIN_ID_GAIN: u16 = 250;

/// RTPS Participant ID Gain (PG)
pub const PARTICIPANT_ID_GAIN: u16 = 2;

/// RTPS User Multicast Port Offset (d0)
pub const USER_MULTICAST_OFFSET: u16 = 2;

/// RTPS User Unicast Port Offset (d1)
pub const USER_UNICAST_OFFSET: u16 = 11;

/// RTPS SPDP Multicast Port Offset (d2)
pub const SPDP_MULTICAST_OFFSET: u16 = 0;

/// RTPS SPDP Unicast Port Offset (d3)
pub const SPDP_UNICAST_OFFSET: u16 = 10;

/// Maximum UDP Payload Size
pub const UDP_MAX_PAYLOAD_SIZE: usize = 65535;

// ──────────────────────────────────────────────────────────────────────────────
// TypeSupport — Type-erased serialization bridge
// ──────────────────────────────────────────────────────────────────────────────

/// Trait to be implemented by generated or user types to allow the type-erased
/// middleware to serialize and deserialize data.
pub trait TypeSupport: Send + Sync {
    /// Return the name of the registered data type.
    fn get_type_name(&self) -> &str;

    /// Serialize the given type-erased value to CDR bytes.
    fn serialize(&self, value: &dyn core::any::Any) -> DdsResult<Vec<u8>>;

    /// Deserialize the CDR bytes into a type-erased Boxed value.
    fn deserialize(&self, bytes: &[u8]) -> DdsResult<Box<dyn core::any::Any>>;

    /// Compute the key hash/InstanceHandle for the given type-erased value.
    fn get_key_hash(
        &self,
        value: &dyn core::any::Any,
    ) -> DdsResult<dds_types::instance::InstanceHandle>;
}

// ──────────────────────────────────────────────────────────────────────────────
// Topic (DCPS §2.2.2.1.2)
// ──────────────────────────────────────────────────────────────────────────────

/// Represents a Topic in the DDS Domain.
#[derive(Debug, Clone)]
pub struct Topic {
    name: String,
    type_name: String,
    qos: TopicQos,
}

impl Topic {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    #[must_use]
    pub const fn qos(&self) -> &TopicQos {
        &self.qos
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// DataWriter (DCPS §2.2.2.4.2)
// ──────────────────────────────────────────────────────────────────────────────

/// A type-erased `DataWriter` to write samples of a specific Topic.
///
/// On `write()`, serializes the sample and inserts a `CacheChange` into the
/// underlying RTPS `StatefulWriter`. The `RtpsEngine` background thread then
/// sends it over UDP to all matched reader proxy locators.
pub struct DataWriter {
    guid: Guid,
    topic: Topic,
    qos: DataWriterQos,
    type_support: Arc<dyn TypeSupport>,
    /// Backing RTPS writer shared with RtpsEngine.
    rtps_writer: Arc<Mutex<StatefulWriter>>,
    listener: Mutex<Option<Arc<dyn DataWriterListener>>>,
    /// Monotonic sequence number counter.
    next_sn: Mutex<SequenceNumber>,
    is_enabled: std::sync::atomic::AtomicBool,
}

impl DataWriter {
    /// Serialize the sample, push it into the RTPS HistoryCache as a CacheChange.
    /// The RtpsEngine loop picks it up and sends it via UDP.
    pub fn write(&self, value: &dyn core::any::Any) -> DdsResult<()> {
        if !self.is_enabled.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(DdsError::NotEnabled);
        }
        let serialized = self.type_support.serialize(value)?;
        let key_hash = self.type_support.get_key_hash(value)?;

        let sn = {
            let mut sn = self.next_sn.lock().unwrap();
            let current = *sn;
            *sn = SequenceNumber(sn.0 + 1);
            current
        };

        let change = CacheChange {
            kind: ChangeKind::Alive,
            writer_guid: self.guid,
            instance_handle: key_hash,
            sequence_number: sn,
            data_value: bytes::Bytes::from(serialized),
            source_timestamp: None,
        };

        let mut w = self.rtps_writer.lock().unwrap();
        w.writer_cache.add_change(change);
        w.last_change_sequence_number = sn;
        Ok(())
    }

    /// Set a listener to receive callbacks.
    pub fn set_listener(&self, listener: Option<Arc<dyn DataWriterListener>>) {
        let mut l = self.listener.lock().unwrap();
        *l = listener;
    }

    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    #[must_use]
    pub const fn qos(&self) -> &DataWriterQos {
        &self.qos
    }

    #[must_use]
    pub const fn topic(&self) -> &Topic {
        &self.topic
    }

    /// Enables the DataWriter.
    pub fn enable(&self) -> DdsResult<()> {
        self.is_enabled.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}


// ──────────────────────────────────────────────────────────────────────────────
// DataReader (DCPS §2.2.2.5.3)
// ──────────────────────────────────────────────────────────────────────────────

/// A type-erased `DataReader` to read samples of a specific Topic.
pub struct DataReader {
    guid: Guid,
    topic: Topic,
    qos: DataReaderQos,
    type_support: Arc<dyn TypeSupport>,
    // Simulates received samples queue
    samples: Mutex<Vec<(dds_types::instance::InstanceHandle, Vec<u8>, Option<std::time::Instant>)>>,
    listener: Mutex<Option<Arc<dyn DataReaderListener>>>,
    pub received_sns: Mutex<std::collections::HashSet<SequenceNumber>>,
    pub acknack_count: Mutex<i32>,
    last_received_time: Mutex<Option<std::time::Instant>>,
    is_enabled: std::sync::atomic::AtomicBool,
}

impl DataReader {
    /// Read the next available sample.
    pub fn read_next(&self) -> DdsResult<Box<dyn core::any::Any>> {
        if !self.is_enabled.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(DdsError::NotEnabled);
        }
        let mut samples = self.samples.lock().unwrap();
        
        // Enforce Lifespan sample expiry
        let lifespan = self.topic.qos().lifespan.duration;
        if lifespan != dds_types::time::Duration::INFINITE {
            let std_lifespan = std::time::Duration::new(lifespan.seconds.max(0) as u64, lifespan.nanoseconds);
            let now = std::time::Instant::now();
            samples.retain(|&(_, _, ref t)| {
                if let Some(t) = t {
                    now.duration_since(*t) <= std_lifespan
                } else {
                    true
                }
            });
        }
        
        if samples.is_empty() {
            return Err(DdsError::NoData);
        }
        let (_, bytes, _) = samples.remove(0);
        self.type_support.deserialize(&bytes)
    }

    /// Set a listener to receive callbacks.
    pub fn set_listener(&self, listener: Option<Arc<dyn DataReaderListener>>) {
        let mut l = self.listener.lock().unwrap();
        *l = listener;
    }

    /// Push a received packet into the reader's cache (simulating RTPS matching).
    pub fn push_sample(&self, key: dds_types::instance::InstanceHandle, bytes: Vec<u8>) {
        if !self.is_enabled.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let now = std::time::Instant::now();
        {
            let mut last_rx = self.last_received_time.lock().unwrap();
            let min_sep = self.qos.time_based_filter.minimum_separation;
            if let Some(t) = *last_rx {
                if min_sep != dds_types::time::Duration::ZERO 
                   && min_sep != dds_types::time::Duration::INFINITE 
                {
                    let std_min_sep = std::time::Duration::new(min_sep.seconds.max(0) as u64, min_sep.nanoseconds);
                    if now.duration_since(t) < std_min_sep {
                        return; // drop sample
                    }
                }
            }
            *last_rx = Some(now);
        }

        {
            let mut samples = self.samples.lock().unwrap();
            
            // Enforce max_samples_per_instance
            if self.qos.resource_limits.max_samples_per_instance != dds_types::qos::LENGTH_UNLIMITED
                && samples.len() >= self.qos.resource_limits.max_samples_per_instance as usize
            {
                samples.remove(0);
            }

            samples.push((key, bytes, Some(now)));

            // Enforce KeepLast
            if matches!(self.qos.history.kind, dds_types::qos::HistoryKind::KeepLast) {
                while samples.len() > self.qos.history.depth as usize {
                    samples.remove(0);
                }
            }
        }

        // Trigger listener callback if registered
        let listener_opt = self.listener.lock().unwrap().clone();
        if let Some(listener) = listener_opt {
            listener.on_data_available(self);
        }
    }

    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    #[must_use]
    pub const fn qos(&self) -> &DataReaderQos {
        &self.qos
    }

    #[must_use]
    pub const fn topic(&self) -> &Topic {
        &self.topic
    }

    /// Enables the DataReader.
    pub fn enable(&self) -> DdsResult<()> {
        self.is_enabled.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    pub fn next_acknack_count(&self) -> i32 {
        let mut count = self.acknack_count.lock().unwrap();
        *count += 1;
        *count
    }

    pub fn push_sample_sn(&self, key: dds_types::instance::InstanceHandle, bytes: Vec<u8>, sn: SequenceNumber) {
        {
            let mut received = self.received_sns.lock().unwrap();
            received.insert(sn);
        }
        self.push_sample(key, bytes);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Publisher (DCPS §2.2.2.4.1)
// ──────────────────────────────────────────────────────────────────────────────

/// Publisher coordinates writing data samples for various topics.
///
/// Each `DataWriter` created here gets its own `StatefulWriter` + `RtpsEngine`
/// background thread that sends samples over UDP.
pub struct Publisher {
    guid: Guid,
    qos: PublisherQos,
    writers: Mutex<HashMap<Guid, Arc<DataWriter>>>,
    /// Transport shared with all DataWriters created by this Publisher.
    transport: Arc<UdpTransport>,
    /// Central writer registry for matched remote participant lookup.
    writer_registry: Arc<Mutex<HashMap<Guid, Arc<Mutex<dds_rtps::StatefulWriter>>>>>,
    pub security_crypto: Arc<dds_security::BuiltinCryptography>,
    pub local_crypto_handle: dds_security::ParticipantCryptoHandle,
    pub remote_crypto_handles: Arc<Mutex<HashMap<GuidPrefix, dds_security::ParticipantCryptoHandle>>>,
    is_enabled: std::sync::atomic::AtomicBool,
}

impl Publisher {
    #[must_use]
    pub const fn qos(&self) -> &PublisherQos {
        &self.qos
    }

    /// Enables the Publisher.
    pub fn enable(&self) -> DdsResult<()> {
        self.is_enabled.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    /// Deletes a DataWriter created by this Publisher.
    pub fn delete_datawriter(&self, guid: &Guid) -> DdsResult<()> {
        let mut writers = self.writers.lock().unwrap();
        if writers.remove(guid).is_some() {
            Ok(())
        } else {
            Err(DdsError::Error("DataWriter not found".into()))
        }
    }

    /// Create a `DataWriter` for the given Topic.
    ///
    /// Internally creates a `StatefulWriter` and spawns an `RtpsEngine` loop
    /// that dispatches `CacheChange`s over UDP.
    pub fn create_datawriter(
        &self,
        topic: &Topic,
        qos: DataWriterQos,
        type_support: Arc<dyn TypeSupport>,
    ) -> DdsResult<Arc<DataWriter>> {
        if qos.history.kind == dds_types::qos::HistoryKind::KeepLast {
            if qos.resource_limits.max_samples_per_instance != dds_types::qos::LENGTH_UNLIMITED
                && qos.resource_limits.max_samples_per_instance < qos.history.depth
            {
                return Err(DdsError::InconsistentPolicy("max_samples_per_instance < history depth".into()));
            }
        }

        let mut writers = self.writers.lock().unwrap();
        let writer_entity_id = EntityId::new([
            0x00,
            0x00,
            (writers.len() + 1) as u8,
            EntityKind::WriterWithKey as u8,
        ]);
        let writer_guid = Guid::new(self.guid.prefix, writer_entity_id);

        // Build the RTPS StatefulWriter
        let rtps_writer = Arc::new(Mutex::new(StatefulWriter::new(
            writer_guid,
            qos.history.kind,
            qos.history.depth,
            qos.resource_limits.max_samples_per_instance,
        )));
        self.writer_registry
            .lock()
            .unwrap()
            .insert(writer_guid, rtps_writer.clone());

        let sec_crypto = self.security_crypto.clone();
        let loc_crypto_handle = self.local_crypto_handle;
        let rem_crypto_handles = self.remote_crypto_handles.clone();

        let encrypt_fn: Option<Arc<dyn Fn(&[u8], GuidPrefix) -> Option<Vec<u8>> + Send + Sync>> = Some(Arc::new(move |raw_bytes, remote_prefix| {
            let opt_remote_crypto = rem_crypto_handles.lock().unwrap().get(&remote_prefix).copied();
            if let Some(remote_crypto_handle) = opt_remote_crypto {
                if let Ok((enc_bytes, h, f)) = sec_crypto.encrypt_payload(raw_bytes, &loc_crypto_handle, &remote_crypto_handle) {
                    let mut contig = Vec::with_capacity(28 + enc_bytes.len() + 16);
                    contig.extend_from_slice(&h.initialization_vector);
                    contig.extend_from_slice(&h.session_id);
                    contig.extend_from_slice(&enc_bytes);
                    contig.extend_from_slice(&f.mac_tag);
                    return Some(contig);
                }
            }
            None
        }));

        let engine = RtpsEngine::new(
            rtps_writer.clone(),
            self.transport.clone(),
            encrypt_fn,
        );
        // Dispatch every 10ms — low latency for local loopback
        let _engine_handle = engine.spawn_run_loop(core::time::Duration::from_millis(10));

        let writer = Arc::new(DataWriter {
            guid: writer_guid,
            topic: topic.clone(),
            qos,
            type_support,
            rtps_writer,
            listener: Mutex::new(None),
            next_sn: Mutex::new(SequenceNumber(1)),
            is_enabled: std::sync::atomic::AtomicBool::new(self.qos.entity_factory.autoenable_created_entities),
        });

        writers.insert(writer_guid, writer.clone());
        Ok(writer)
    }

    /// Add a reader locator to all DataWriters owned by this Publisher.
    /// Used by DomainParticipant to wire newly created readers to existing writers.
    pub fn add_reader_proxy_to_all(
        &self,
        reader_guid: dds_types::guid::Guid,
        reader_locator: Locator,
        topic_name: &str,
        subscriber_partition: &dds_types::qos::Partition,
    ) {
        // Enforce Partition matching
        let pub_partitions = if self.qos.partition.name.is_empty() {
            vec!["".to_string()]
        } else {
            self.qos.partition.name.clone()
        };
        let sub_partitions = if subscriber_partition.name.is_empty() {
            vec!["".to_string()]
        } else {
            subscriber_partition.name.clone()
        };

        let mut partition_matches = false;
        for p in &pub_partitions {
            if sub_partitions.contains(p) {
                partition_matches = true;
                break;
            }
        }

        if !partition_matches {
            return;
        }

        let writers = self.writers.lock().unwrap();
        for writer in writers.values() {
            if writer.topic.name() == topic_name {
                let proxy = dds_rtps::ReaderProxy {
                    remote_reader_guid: reader_guid,
                    unicast_locator_list: vec![reader_locator],
                    multicast_locator_list: vec![],
                    next_unsent_sn: SequenceNumber(1),
                };
                writer.rtps_writer.lock().unwrap().matched_reader_add(proxy);
            }
        }
    }
}


// ──────────────────────────────────────────────────────────────────────────────
// Subscriber (DCPS §2.2.2.5.1)
// ──────────────────────────────────────────────────────────────────────────────

/// Subscriber coordinates reading data samples for various topics.
///
/// Newly created `DataReader`s are registered in the participant's
/// `reader_registry` so that the UDP receiver loop can route incoming
/// RTPS DATA submessages to the correct reader by topic name.
pub struct Subscriber {
    guid: Guid,
    qos: SubscriberQos,
    readers: Mutex<HashMap<Guid, Arc<DataReader>>>,
    /// Shared registry: topic_name -> DataReader, used by receive loop.
    reader_registry: Arc<Mutex<HashMap<String, Arc<DataReader>>>>,
    /// Unicast port this participant's readers listen on.
    unicast_port: u32,
    /// Local GUID prefix for building reader GUIDs.
    guid_prefix: GuidPrefix,
    is_enabled: std::sync::atomic::AtomicBool,
}

impl Subscriber {
    #[must_use]
    pub const fn qos(&self) -> &SubscriberQos {
        &self.qos
    }

    /// Enables the Subscriber.
    pub fn enable(&self) -> DdsResult<()> {
        self.is_enabled.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    /// Deletes a DataReader created by this Subscriber.
    pub fn delete_datareader(&self, guid: &Guid) -> DdsResult<()> {
        let mut readers = self.readers.lock().unwrap();
        if readers.remove(guid).is_some() {
            Ok(())
        } else {
            Err(DdsError::Error("DataReader not found".into()))
        }
    }

    /// Create a `DataReader` for the given Topic.
    ///
    /// Registers the reader in the shared `reader_registry` so the participant's
    /// UDP receive loop can route incoming RTPS DATA submessages to it.
    pub fn create_datareader(
        &self,
        topic: &Topic,
        qos: DataReaderQos,
        type_support: Arc<dyn TypeSupport>,
    ) -> DdsResult<Arc<DataReader>> {
        if qos.history.kind == dds_types::qos::HistoryKind::KeepLast {
            if qos.resource_limits.max_samples_per_instance != dds_types::qos::LENGTH_UNLIMITED
                && qos.resource_limits.max_samples_per_instance < qos.history.depth
            {
                return Err(DdsError::InconsistentPolicy("max_samples_per_instance < history depth".into()));
            }
        }

        let mut readers = self.readers.lock().unwrap();
        let reader_entity_id = EntityId::new([
            0x00,
            0x00,
            (readers.len() + 1) as u8,
            EntityKind::ReaderWithKey as u8,
        ]);
        let reader_guid = Guid::new(self.guid_prefix, reader_entity_id);

        let reader = Arc::new(DataReader {
            guid: reader_guid,
            topic: topic.clone(),
            qos,
            type_support,
            samples: Mutex::new(Vec::new()),
            listener: Mutex::new(None),
            received_sns: Mutex::new(std::collections::HashSet::new()),
            acknack_count: Mutex::new(1),
            last_received_time: Mutex::new(None),
            is_enabled: std::sync::atomic::AtomicBool::new(self.qos.entity_factory.autoenable_created_entities),
        });

        readers.insert(reader_guid, reader.clone());
        // Register so the receive loop can find this reader by topic
        self.reader_registry
            .lock()
            .unwrap()
            .insert(topic.name().to_owned(), reader.clone());
        Ok(reader)
    }

    /// Return the unicast port this subscriber's readers listen on.
    #[must_use]
    pub const fn unicast_port(&self) -> u32 {
        self.unicast_port
    }
}


// ──────────────────────────────────────────────────────────────────────────────
// DomainParticipant (DCPS §2.2.2.2.1)
// ──────────────────────────────────────────────────────────────────────────────

/// Represents a local Participant containing topics, publishers, and subscribers.
///
/// Owns the shared UDP transport and reader registry. Provides
/// `spawn_receiver_loop()` which binds a UDP socket on the standard RTPS
/// unicast port and routes incoming `DATA` submessages to matched DataReaders.
pub struct DomainParticipant {
    guid_prefix: GuidPrefix,
    domain_id: u32,
    participant_idx: u32,
    qos: DomainParticipantQos,
    topics: Mutex<HashMap<String, Topic>>,
    types: Mutex<HashMap<String, Arc<dyn TypeSupport>>>,
    /// topic_name -> DataReader; populated when create_subscriber().create_datareader() is called.
    reader_registry: Arc<Mutex<HashMap<String, Arc<DataReader>>>>,
    /// guid -> StatefulWriter; populated when create_publisher().create_datawriter() is called.
    writer_registry: Arc<Mutex<HashMap<Guid, Arc<Mutex<dds_rtps::StatefulWriter>>>>>,
    /// Shared UDP socket used by all Publishers/DataWriters in this participant.
    transport: Arc<UdpTransport>,
    /// RTPS unicast port: PORT_BASE + DOMAIN_ID_GAIN * domain_id + SPDP_UNICAST_OFFSET + PARTICIPANT_ID_GAIN * participantId
    /// RTPS multicast port: PORT_BASE + DOMAIN_ID_GAIN * domain_id + SPDP_MULTICAST_OFFSET
    unicast_port: u32,
    pub security_auth: Arc<dds_security::BuiltinAuthentication>,
    pub security_access: Arc<dds_security::BuiltinAccessControl>,
    pub security_crypto: Arc<dds_security::BuiltinCryptography>,
    pub security_data_tagging: Arc<dds_security::BuiltinDataTagging>,
    pub local_crypto_handle: dds_security::ParticipantCryptoHandle,
    pub remote_crypto_handles: Arc<Mutex<HashMap<GuidPrefix, dds_security::ParticipantCryptoHandle>>>,
    pub discovery: Arc<Mutex<dds_discovery::DiscoveryManager>>,
    is_enabled: std::sync::atomic::AtomicBool,
}

impl DomainParticipant {
    #[must_use]
    pub fn new(guid_prefix: GuidPrefix, domain_id: u32, participant_idx: u32, qos: DomainParticipantQos, transport: Arc<UdpTransport>, unicast_port: u32) -> Self {
        let security_auth = Arc::new(dds_security::BuiltinAuthentication::new());
        let security_access = Arc::new(dds_security::BuiltinAccessControl::new());
        let security_crypto = Arc::new(dds_security::BuiltinCryptography::new());
        let security_data_tagging = Arc::new(dds_security::BuiltinDataTagging::new());

        // Bootstrap security by loading local credentials if configured
        let (local_id_handle, _) = security_auth.validate_local_identity(domain_id, &qos)
            .unwrap_or((dds_security::IdentityHandle(0), dds_security::IdentityToken { class_id: String::new(), properties: Vec::new() }));

        let local_crypto_handle = security_crypto.register_local_participant(&local_id_handle)
            .unwrap_or(dds_security::ParticipantCryptoHandle(0));

        let discovery = Arc::new(Mutex::new(dds_discovery::DiscoveryManager::new(guid_prefix)));

        if qos.entity_factory.autoenable_created_entities {
            let disc = discovery.lock().unwrap();
            disc.spawn_spdp_announcer(
                std::time::Duration::from_secs(5),
                transport.clone(),
                domain_id,
                None, // default multicast
            );
        }

        Self {
            guid_prefix,
            domain_id,
            participant_idx,
            qos: qos.clone(),
            topics: Mutex::new(HashMap::new()),
            types: Mutex::new(HashMap::new()),
            reader_registry: Arc::new(Mutex::new(HashMap::new())),
            writer_registry: Arc::new(Mutex::new(HashMap::new())),
            transport,
            unicast_port,
            security_auth,
            security_access,
            security_crypto,
            security_data_tagging,
            local_crypto_handle,
            remote_crypto_handles: Arc::new(Mutex::new(HashMap::new())),
            discovery,
            is_enabled: std::sync::atomic::AtomicBool::new(qos.entity_factory.autoenable_created_entities),
        }
    }

    /// Computes the unicast port for user traffic (DataWriter/DataReader)
    /// RTPS Formula: PB + DG * domainId + d1 + PG * participantId
    /// where PB=PORT_BASE, DG=DOMAIN_ID_GAIN, d1=USER_UNICAST_OFFSET.
    fn compute_user_unicast_port(&self) -> u16 {
        let domain_id = self.domain_id as u16;
        PORT_BASE + DOMAIN_ID_GAIN * domain_id + USER_UNICAST_OFFSET + PARTICIPANT_ID_GAIN * self.participant_idx as u16
    }

    /// Get the domain ID this participant is bound to.
    #[must_use]
    pub const fn domain_id(&self) -> u32 {
        self.domain_id
    }

    /// Enables the DomainParticipant. If autoenable is false, this must be called explicitly.
    pub fn enable(&self) -> DdsResult<()> {
        let was_enabled = self.is_enabled.swap(true, std::sync::atomic::Ordering::SeqCst);
        if !was_enabled {
            // Spawn SPDP announcer on enable
            let discovery = self.discovery.lock().unwrap();
            discovery.spawn_spdp_announcer(
                std::time::Duration::from_secs(5),
                self.transport.clone(),
                self.domain_id,
                None, // default multicast
            );
        }
        Ok(())
    }

    /// Deletes a Topic created by this DomainParticipant.
    pub fn delete_topic(&self, name: &str) -> DdsResult<()> {
        let mut topics = self.topics.lock().unwrap();
        if topics.remove(name).is_some() {
            Ok(())
        } else {
            Err(DdsError::Error(format!("Topic {} not found", name)))
        }
    }

    /// Deletes a Publisher created by this DomainParticipant.
    pub fn delete_publisher(&self, publisher: &Publisher) -> DdsResult<()> {
        let writers = publisher.writers.lock().unwrap();
        let mut reg = self.writer_registry.lock().unwrap();
        for (guid, _) in writers.iter() {
            reg.remove(guid);
        }
        Ok(())
    }

    /// Deletes a Subscriber created by this DomainParticipant.
    pub fn delete_subscriber(&self, subscriber: &Subscriber) -> DdsResult<()> {
        let readers = subscriber.readers.lock().unwrap();
        let mut reg = self.reader_registry.lock().unwrap();
        for (_, reader) in readers.iter() {
            reg.remove(reader.topic.name());
        }
        Ok(())
    }

    #[must_use]
    pub const fn qos(&self) -> &DomainParticipantQos {
        &self.qos
    }

    #[must_use]
    pub const fn guid_prefix(&self) -> GuidPrefix {
        self.guid_prefix
    }

    /// Register a type support helper.
    pub fn register_type(&self, name: &str, type_support: Arc<dyn TypeSupport>) -> DdsResult<()> {
        let mut types = self.types.lock().unwrap();
        types.insert(name.to_owned(), type_support);
        Ok(())
    }

    /// Create a Topic.
    pub fn create_topic(&self, name: &str, type_name: &str, qos: TopicQos) -> DdsResult<Topic> {
        let mut topics = self.topics.lock().unwrap();
        if topics.contains_key(name) {
            return Err(DdsError::PreconditionNotMet("topic already exists".into()));
        }

        let topic = Topic {
            name: name.to_owned(),
            type_name: type_name.to_owned(),
            qos,
        };
        topics.insert(name.to_owned(), topic.clone());
        Ok(topic)
    }

    /// Create a Publisher backed by the participant's shared UDP transport.
    pub fn create_publisher(&self, qos: PublisherQos) -> DdsResult<Publisher> {
        let pub_guid = Guid::new(
            self.guid_prefix,
            EntityId::new([0, 0, 1, EntityKind::BuiltinParticipant as u8]),
        );
        Ok(Publisher {
            guid: pub_guid,
            qos,
            writers: Mutex::new(HashMap::new()),
            transport: self.transport.clone(),
            writer_registry: self.writer_registry.clone(),
            security_crypto: self.security_crypto.clone(),
            local_crypto_handle: self.local_crypto_handle,
            remote_crypto_handles: self.remote_crypto_handles.clone(),
            is_enabled: std::sync::atomic::AtomicBool::new(self.qos.entity_factory.autoenable_created_entities),
        })
    }

    /// Create a Subscriber. DataReaders created from it are registered in the
    /// shared `reader_registry` for UDP dispatch.
    pub fn create_subscriber(&self, qos: SubscriberQos) -> DdsResult<Subscriber> {
        let sub_guid = Guid::new(
            self.guid_prefix,
            EntityId::new([0, 0, 2, EntityKind::BuiltinParticipant as u8]),
        );
        Ok(Subscriber {
            guid: sub_guid,
            qos,
            readers: Mutex::new(HashMap::new()),
            reader_registry: self.reader_registry.clone(),
            unicast_port: self.unicast_port,
            guid_prefix: self.guid_prefix,
            is_enabled: std::sync::atomic::AtomicBool::new(self.qos.entity_factory.autoenable_created_entities),
        })
    }

    #[must_use]
    pub fn spawn_receiver_loop(&self) -> std::thread::JoinHandle<()> {
        let registry = self.reader_registry.clone();
        let writer_registry = self.writer_registry.clone();
        let transport = self.transport.clone();
        let guid_prefix = self.guid_prefix;
        let port = self.unicast_port;
        let security_crypto = self.security_crypto.clone();
        let local_crypto_handle = self.local_crypto_handle;
        let remote_crypto_handles = self.remote_crypto_handles.clone();
        let domain_id = self.domain_id;
        let discovery = self.discovery.clone();
        println!("[spawn_receiver_loop] Spawning receiver loop for port {}", port);
        
        struct FragmentBuffer {
            data_size: usize,
            _fragment_size: usize,
            received_bytes: usize,
            buffer: Vec<u8>,
            received_mask: Vec<bool>,
        }

        let (_shutdown_tx, shutdown_rx) = std::sync::mpsc::channel::<()>();
        std::thread::spawn(move || {
            let mut reassembly_buffers: HashMap<(Guid, SequenceNumber), FragmentBuffer> = HashMap::new();

            // Blocking socket for the receive loop
            let socket = match std::net::UdpSocket::bind(format!("127.0.0.1:{port}")) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[DomainParticipant] recv bind failed on port {port}: {e}");
                    return;
                }
            };
            println!("[spawn_receiver_loop] Bound receiver socket to {}", socket.local_addr().unwrap());
            // Start an additional thread for the SPDP Multicast Port
            let multicast_port = PORT_BASE + DOMAIN_ID_GAIN * domain_id as u16 + SPDP_MULTICAST_OFFSET;
            let discovery_clone = discovery.clone();
            std::thread::spawn(move || {
                let mcast_socket = match std::net::UdpSocket::bind(format!("0.0.0.0:{}", multicast_port)) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[SPDP Receiver] bind failed on port {}: {}", multicast_port, e);
                        return;
                    }
                };
                let multicast_addr = Ipv4Addr::new(239, 255, 0, 1);
                let _ = mcast_socket.join_multicast_v4(&multicast_addr, &Ipv4Addr::UNSPECIFIED);
                
                let mut buf = [0u8; UDP_MAX_PAYLOAD_SIZE];
                loop {
                    if let Ok((len, _)) = mcast_socket.recv_from(&mut buf) {
                        let data = &buf[..len];
                        if let Ok((_header, submessages)) = dds_rtps::parse_rtps_message(data) {
                            for sub in submessages {
                                if let dds_rtps::Submessage::Data(d) = sub {
                                    if d.writer_id == dds_types::guid::EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER {
                                        if let Some(participant) = dds_discovery::parse_spdp_packet(&d.serialized_payload) {
                                            discovery_clone.lock().unwrap().process_spdp_packet(participant);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            });

            let mut buf = vec![0u8; UDP_MAX_PAYLOAD_SIZE];
            loop {
                if shutdown_rx.try_recv().is_ok() { break; }
                let (len, from) = match socket.recv_from(&mut buf) {
                    Ok(r) => r,
                    Err(e) => {
                        println!("[spawn_receiver_loop] recv_from failed: {:?}", e);
                        continue;
                    }
                };
                let data = &buf[..len];
                println!("[spawn_receiver_loop] Received UDP packet of length {} from {:?}", len, from);

                // Parse RTPS message
                let (header, submessages) = match parse_rtps_message(data) {
                    Ok(r) => {
                        println!("[spawn_receiver_loop] Successfully parsed RTPS message");
                        r
                    }
                    Err(e) => {
                        println!("[spawn_receiver_loop] parse_rtps_message failed: {:?}", e);
                        continue;
                    }
                };

                let reg = registry.lock().unwrap();
                println!("[spawn_receiver_loop] Processing {} submessages. Registry size = {}", submessages.len(), reg.len());
                for sub in &submessages {
                    if let Submessage::Data(d) = sub {
                        println!("[spawn_receiver_loop] Found Submessage::Data. Serialized payload len = {}", d.serialized_payload.len());
                        let mut final_payload = d.serialized_payload.to_vec();
                        let sender_prefix = header.guid_prefix;
                        let opt_remote_crypto = remote_crypto_handles.lock().unwrap().get(&sender_prefix).copied();
                        
                        if let Some(remote_crypto_handle) = opt_remote_crypto {
                            if final_payload.len() >= 44 { // 28 + 16
                                let mut iv = [0_u8; 12];
                                iv.copy_from_slice(&final_payload[0..12]);
                                let mut session_id = [0_u8; 16];
                                session_id.copy_from_slice(&final_payload[12..28]);
                                
                                let tag_start = final_payload.len() - 16;
                                let mut mac_tag = [0_u8; 16];
                                mac_tag.copy_from_slice(&final_payload[tag_start..]);
                                
                                let ciphertext = &final_payload[28..tag_start];
                                
                                let crypto_header = dds_security::CryptoHeader {
                                    initialization_vector: iv,
                                    session_id,
                                };
                                let crypto_footer = dds_security::CryptoFooter { mac_tag };
                                
                                if let Ok(dec_bytes) = security_crypto.decrypt_payload(
                                    ciphertext,
                                    &crypto_header,
                                    &crypto_footer,
                                    &local_crypto_handle,
                                    &remote_crypto_handle,
                                ) {
                                    final_payload = dec_bytes;
                                }
                            }
                        }

                        let writer_id = d.writer_id;
                        
                        if writer_id == dds_types::guid::EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER || writer_id == dds_types::guid::EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER {
                            if let Some(endpoint) = dds_discovery::parse_sedp_packet(&final_payload) {
                                discovery.lock().unwrap().process_sedp_endpoint(endpoint);
                            }
                            continue;
                        }

                        let mut delivered = false;
                        for (guid, reader) in reg.iter() {
                            let res = reader.type_support.deserialize(&final_payload);
                            println!("[spawn_receiver_loop] Trying reader GUID: {:?}, Topic: {}, deserialize result: {:?}", guid, reader.topic().name(), res.is_ok());
                            if res.is_ok() {
                                reader.push_sample_sn(
                                    dds_types::instance::InstanceHandle::NIL,
                                    final_payload.clone(),
                                    d.writer_sn,
                                );
                                println!("[spawn_receiver_loop] Successfully pushed sample to reader");
                                delivered = true;
                                break;
                            }
                        }
                        if !delivered {
                            println!("[spawn_receiver_loop] WARNING: Failed to deliver payload to any reader!");
                        }
                    } else if let Submessage::DataFrag(df) = sub {
                        println!("[spawn_receiver_loop] Found Submessage::DataFrag. SN: {:?}, frag: {}, size: {}, total: {}", df.writer_sn, df.fragment_starting_num, df.fragment_size, df.data_size);
                        let writer_guid = Guid::new(header.guid_prefix, df.writer_id);
                        let key = (writer_guid, df.writer_sn);
                        let frag_entry = reassembly_buffers.entry(key).or_insert_with(|| {
                            FragmentBuffer {
                                data_size: df.data_size as usize,
                                _fragment_size: df.fragment_size as usize,
                                received_bytes: 0,
                                buffer: vec![0; df.data_size as usize],
                                received_mask: vec![false; ((df.data_size + df.fragment_size as u32 - 1) / df.fragment_size as u32) as usize],
                            }
                        });

                        let num_fragments = df.fragments_in_submessage as usize;
                        let frag_size = df.fragment_size as usize;
                        let start_idx = (df.fragment_starting_num - 1) as usize;

                        for i in 0..num_fragments {
                            let frag_idx = start_idx + i;
                            if frag_idx < frag_entry.received_mask.len() && !frag_entry.received_mask[frag_idx] {
                                let offset = frag_idx * frag_size;
                                let src_start = i * frag_size;
                                let src_end = (src_start + frag_size).min(df.serialized_payload.len());
                                if src_start < df.serialized_payload.len() {
                                    let chunk = &df.serialized_payload[src_start..src_end];
                                    let dest_end = (offset + chunk.len()).min(frag_entry.buffer.len());
                                    frag_entry.buffer[offset..dest_end].copy_from_slice(&chunk[..dest_end - offset]);
                                    frag_entry.received_mask[frag_idx] = true;
                                    frag_entry.received_bytes += chunk.len();
                                }
                            }
                        }

                        if frag_entry.received_bytes >= frag_entry.data_size {
                            println!("[spawn_receiver_loop] Reassembled fragmented payload of size {}", frag_entry.data_size);
                            let mut final_payload = frag_entry.buffer.clone();
                            reassembly_buffers.remove(&key);

                            let sender_prefix = header.guid_prefix;
                            let opt_remote_crypto = remote_crypto_handles.lock().unwrap().get(&sender_prefix).copied();
                            
                            if let Some(remote_crypto_handle) = opt_remote_crypto {
                                if final_payload.len() >= 44 { // 28 + 16
                                    let mut iv = [0_u8; 12];
                                    iv.copy_from_slice(&final_payload[0..12]);
                                    let mut session_id = [0_u8; 16];
                                    session_id.copy_from_slice(&final_payload[12..28]);
                                    
                                    let tag_start = final_payload.len() - 16;
                                    let mut mac_tag = [0_u8; 16];
                                    mac_tag.copy_from_slice(&final_payload[tag_start..]);
                                    
                                    let ciphertext = &final_payload[28..tag_start];
                                    
                                    let crypto_header = dds_security::CryptoHeader {
                                        initialization_vector: iv,
                                        session_id,
                                    };
                                    let crypto_footer = dds_security::CryptoFooter { mac_tag };
                                    
                                    if let Ok(dec_bytes) = security_crypto.decrypt_payload(
                                        ciphertext,
                                        &crypto_header,
                                        &crypto_footer,
                                        &local_crypto_handle,
                                        &remote_crypto_handle,
                                    ) {
                                        final_payload = dec_bytes;
                                    }
                                }
                            }

                            let mut delivered = false;
                            for (_guid, reader) in reg.iter() {
                                let res = reader.type_support.deserialize(&final_payload);
                                if res.is_ok() {
                                    reader.push_sample_sn(
                                        dds_types::instance::InstanceHandle::NIL,
                                        final_payload.clone(),
                                        df.writer_sn,
                                    );
                                    delivered = true;
                                    break;
                                }
                            }
                            if !delivered {
                                println!("[spawn_receiver_loop] WARNING: Failed to deliver reassembled payload!");
                            }
                        }
                    } else if let Submessage::Heartbeat(hb) = sub {
                        println!("[spawn_receiver_loop] Found Submessage::Heartbeat. Reader: {:?}, Writer: {:?}", hb.reader_id, hb.writer_id);
                        for reader in reg.values() {
                            if hb.reader_id == reader.guid.entity_id || hb.reader_id == EntityId::UNKNOWN {
                                if reader.qos.reliability.kind == dds_types::qos::ReliabilityKind::Reliable {
                                    let mut missing = Vec::new();
                                    {
                                        let received = reader.received_sns.lock().unwrap();
                                        for sn_val in hb.first_sn.0..=hb.last_sn.0 {
                                            let sn = SequenceNumber(sn_val);
                                            if !received.contains(&sn) {
                                                missing.push(sn);
                                            }
                                        }
                                    }
                                    
                                    // Send AckNack back to the writer's address
                                    let ack_sub = Submessage::AckNack(dds_rtps::AckNack {
                                        reader_id: reader.guid.entity_id,
                                        writer_id: hb.writer_id,
                                        reader_sn_state: missing,
                                        count: reader.next_acknack_count(),
                                    });
                                    let header = dds_rtps::RtpsHeader::new(guid_prefix);
                                    let msg = dds_rtps::serialize_rtps_message(&header, &[ack_sub], dds_rtps::Endianness::LittleEndian);
                                    let dest = Locator::from_socket_addr(from);
                                    let _ = transport.send(&msg, &dest);
                                }
                            }
                        }
                    } else if let Submessage::AckNack(ack) = sub {
                        println!("[spawn_receiver_loop] Found Submessage::AckNack. Reader: {:?}, Writer: {:?}, SN State count: {}", ack.reader_id, ack.writer_id, ack.reader_sn_state.len());
                        let writer_guid = Guid::new(guid_prefix, ack.writer_id);
                        let w_reg = writer_registry.lock().unwrap();
                        if let Some(shared_writer) = w_reg.get(&writer_guid) {
                            let w = shared_writer.lock().unwrap();
                            for sn in &ack.reader_sn_state {
                                if let Some(change) = w.writer_cache.get_changes().iter().find(|c| c.sequence_number == *sn) {
                                    let subs = [
                                        Submessage::InfoTs(dds_rtps::InfoTs { timestamp: change.source_timestamp }),
                                        Submessage::Data(dds_rtps::Data {
                                            reader_id: ack.reader_id,
                                            writer_id: ack.writer_id,
                                            writer_sn: change.sequence_number,
                                            inline_qos: None,
                                            serialized_payload: change.data_value.clone(),
                                        }),
                                    ];
                                    let header = dds_rtps::RtpsHeader::new(guid_prefix);
                                    let msg = dds_rtps::serialize_rtps_message(&header, &subs, dds_rtps::Endianness::LittleEndian);
                                    
                                    if let Some(proxy) = w.reader_proxies.iter().find(|p| p.remote_reader_guid.entity_id == ack.reader_id) {
                                        let locators: Vec<Locator> = proxy.unicast_locator_list.iter()
                                            .chain(proxy.multicast_locator_list.iter())
                                            .cloned()
                                            .collect();
                                        for locator in &locators {
                                            let _ = transport.send(&msg, locator);
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        println!("[spawn_receiver_loop] Submessage kind: {:?}", sub);
                    }
                }
            }
        })
    }
}


// ──────────────────────────────────────────────────────────────────────────────
// DomainParticipantFactory (DCPS §2.2.2.2.2)
// ──────────────────────────────────────────────────────────────────────────────

/// Factory for creating `DomainParticipants`.
pub struct DomainParticipantFactory;

impl DomainParticipantFactory {
    /// Create a `DomainParticipant` with a unique GUID prefix.
    ///
    /// Binds a UDP socket for sending on a random ephemeral port, and
    /// computes the standard RTPS unicast receive port per RTPS §9.6.2:
    /// `PB + DG * domain_id + PO + 2 * participant_idx`
    /// where PB=PORT_BASE, DG=DOMAIN_ID_GAIN, PO=SPDP_UNICAST_OFFSET.
    pub fn create_participant(
        domain_id: u32,
        qos: DomainParticipantQos,
    ) -> DdsResult<DomainParticipant> {
        // Derive a unique GUID prefix using domain_id + process id
        let mut prefix = [0u8; 12];
        prefix[0..4].copy_from_slice(&domain_id.to_be_bytes());
        prefix[4..8].copy_from_slice(&std::process::id().to_be_bytes());

        // Probe for an available unicast port
        let mut participant_idx = 0;
        let mut unicast_port = 0;
        while participant_idx < 100 {
            let port = PORT_BASE + DOMAIN_ID_GAIN * (domain_id as u16) + SPDP_UNICAST_OFFSET + PARTICIPANT_ID_GAIN * participant_idx;
            if std::net::UdpSocket::bind(format!("127.0.0.1:{port}")).is_ok() {
                unicast_port = port;
                // Add participant index to prefix to keep it unique
                prefix[8..12].copy_from_slice(&participant_idx.to_be_bytes());
                break;
            }
            participant_idx += 1;
        }

        if unicast_port == 0 {
            return Err(DdsError::Error(
                "Failed to find an available unicast port (all ports in range 0..100 are busy)".into(),
            ));
        }

        // Bind send socket on ephemeral port
        let transport = UdpTransport::bind(0)
            .map_err(|e| DdsError::Error(format!("UDP bind failed: {e}")))?;
        let transport = Arc::new(transport);

        Ok(DomainParticipant::new(
            GuidPrefix::new(prefix),
            domain_id,
            participant_idx as u32,
            qos,
            transport,
            unicast_port as u32,
        ))
    }
}


// ──────────────────────────────────────────────────────────────────────────────
// Requested vs Offered QoS Compatibility (RxO checks, DCPS §2.2.3)
// ──────────────────────────────────────────────────────────────────────────────

/// Verify if offered `DataWriter` `QoS` is compatible with requested `DataReader` `QoS`.
///
/// Returns true if compatible, false if incompatible.
#[must_use]
pub fn check_qos_compatibility(offered: &DataWriterQos, requested: &DataReaderQos) -> bool {
    // 1. Durability compatibility (offered durability >= requested durability)
    if offered.durability.kind < requested.durability.kind {
        return false;
    }

    // 2. Reliability compatibility (offered reliability >= requested reliability)
    if offered.reliability.kind < requested.reliability.kind {
        return false;
    }

    // 3. Deadline compatibility (offered period <= requested period)
    if offered.deadline.period > requested.deadline.period {
        return false;
    }

    // 4. Liveliness compatibility (offered liveliness kind >= requested liveliness kind)
    if offered.liveliness.kind < requested.liveliness.kind {
        return false;
    }

    // 5. Liveliness lease duration (offered lease <= requested lease)
    if offered.liveliness.lease_duration > requested.liveliness.lease_duration {
        return false;
    }

    // 6. LatencyBudget compatibility (offered duration <= requested duration)
    if offered.latency_budget.duration > requested.latency_budget.duration {
        return false;
    }

    // 7. Ownership compatibility (offered kind == requested kind)
    if offered.ownership.kind != requested.ownership.kind {
        return false;
    }

    // 8. DestinationOrder compatibility (offered kind >= requested kind)
    if offered.destination_order.kind < requested.destination_order.kind {
        return false;
    }

    true
}

/// Verify if `TypeConsistency` `QoS` permits matching between offered type and requested type.
#[must_use]
pub fn check_type_compatibility(
    policy: &dds_types::qos::TypeConsistencyEnforcement,
    offered_type: &dds_xtypes::TypeObject,
    requested_type: &dds_xtypes::TypeObject,
) -> bool {
    match policy.kind {
        dds_types::qos::TypeConsistencyKind::DisallowTypeCoercion => offered_type == requested_type,
        dds_types::qos::TypeConsistencyKind::AllowTypeCoercion => {
            dds_xtypes::is_assignable_from(requested_type, offered_type)
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Conditions & WaitSets (DCPS §2.2.2.1.3)
// ──────────────────────────────────────────────────────────────────────────────

/// Root Condition trait.
pub trait Condition: Send + Sync {
    /// Return the trigger value status.
    fn get_trigger_value(&self) -> bool;
}

/// A `GuardCondition` is controlled manually by the application.
pub struct GuardCondition {
    trigger_value: Mutex<bool>,
}

impl GuardCondition {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            trigger_value: Mutex::new(false),
        }
    }

    pub fn set_trigger_value(&self, value: bool) {
        let mut trigger = self.trigger_value.lock().unwrap();
        *trigger = value;
    }
}

impl Default for GuardCondition {
    fn default() -> Self {
        Self::new()
    }
}

impl Condition for GuardCondition {
    fn get_trigger_value(&self) -> bool {
        return *self.trigger_value.lock().unwrap()
    }
}

/// A `StatusCondition` is associated with a specific DDS Entity.
pub struct StatusCondition {
    trigger_value: Mutex<bool>,
}

impl StatusCondition {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            trigger_value: Mutex::new(false),
        }
    }

    pub fn set_trigger_value(&self, value: bool) {
        let mut trigger = self.trigger_value.lock().unwrap();
        *trigger = value;
    }
}

impl Default for StatusCondition {
    fn default() -> Self {
        Self::new()
    }
}

impl Condition for StatusCondition {
    fn get_trigger_value(&self) -> bool {
        return *self.trigger_value.lock().unwrap()
    }
}

/// A `WaitSet` allows an application to wait for the triggers of attached Conditions.
pub struct WaitSet {
    conditions: Mutex<Vec<Arc<dyn Condition>>>,
}

impl WaitSet {
    #[must_use]
    pub fn new() -> Self {
        Self {
            conditions: Mutex::new(Vec::new()),
        }
    }

    /// Attach a Condition to the `WaitSet`.
    pub fn attach_condition(&self, cond: Arc<dyn Condition>) -> DdsResult<()> {
        let mut list = self.conditions.lock().unwrap();
        list.push(cond);
        Ok(())
    }

    /// Detach a Condition from the `WaitSet`.
    pub fn detach_condition(&self, cond: &Arc<dyn Condition>) -> DdsResult<()> {
        let mut list = self.conditions.lock().unwrap();
        list.retain(|c| !Arc::ptr_eq(c, cond));
        Ok(())
    }

    /// Wait until at least one attached Condition evaluates to true, or timeout.
    pub fn wait(
        &self,
        active_conditions: &mut Vec<Arc<dyn Condition>>,
        timeout: dds_types::time::Duration,
    ) -> DdsResult<()> {
        let start = std::time::Instant::now();
        let timeout_std = timeout
            .to_std()
            .unwrap_or(core::time::Duration::from_secs(0));

        loop {
            let list = self.conditions.lock().unwrap();
            for cond in list.iter() {
                if cond.get_trigger_value() {
                    active_conditions.push(cond.clone());
                }
            }

            if !active_conditions.is_empty() {
                return Ok(());
            }

            if start.elapsed() >= timeout_std {
                return Err(dds_types::return_code::DdsError::Timeout);
            }

            std::thread::sleep(core::time::Duration::from_millis(5));
        }
    }
}

impl Default for WaitSet {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Listener traits (DCPS §2.2.2.1.4)
// ──────────────────────────────────────────────────────────────────────────────

/// General Entity Listener callback definitions.
pub trait Listener: Send + Sync {}

/// `DataReader` listener callback interface.
pub trait DataReaderListener: Listener {
    /// Callback triggered when a new sample is received.
    fn on_data_available(&self, reader: &DataReader);
}

/// `DataWriter` listener callback interface.
pub trait DataWriterListener: Listener {
    /// Callback triggered when a publication gets matched or unmatched.
    fn on_publication_matched(
        &self,
        writer: &DataWriter,
        status: dds_types::status::PublicationMatchedStatus,
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::Any;

    // Simulate type-support for a HelloWorld struct
    struct HelloWorld {
        msg: String,
    }

    struct HelloWorldTypeSupport;

    impl TypeSupport for HelloWorldTypeSupport {
        fn get_type_name(&self) -> &str {
            "HelloWorld"
        }

        fn serialize(&self, value: &dyn Any) -> DdsResult<Vec<u8>> {
            if let Some(hw) = value.downcast_ref::<HelloWorld>() {
                Ok(hw.msg.as_bytes().to_vec())
            } else {
                Err(DdsError::BadParameter("cast failed".into()))
            }
        }

        fn deserialize(&self, bytes: &[u8]) -> DdsResult<Box<dyn Any>> {
            let msg =
                String::from_utf8(bytes.to_vec()).map_err(|e| DdsError::Error(e.to_string()))?;
            Ok(Box::new(HelloWorld { msg }))
        }

        fn get_key_hash(&self, _value: &dyn Any) -> DdsResult<dds_types::instance::InstanceHandle> {
            Ok(dds_types::instance::InstanceHandle::NIL)
        }
    }

    #[test]
    fn test_create_participant_and_entities() {
        let participant =
            DomainParticipantFactory::create_participant(0, DomainParticipantQos::default())
                .unwrap();
        let ts = Arc::new(HelloWorldTypeSupport);
        participant.register_type("HelloWorld", ts.clone()).unwrap();

        let topic = participant
            .create_topic("HelloTopic", "HelloWorld", TopicQos::default())
            .unwrap();
        assert_eq!(topic.name(), "HelloTopic");

        let publisher = participant
            .create_publisher(PublisherQos::default())
            .unwrap();
        let writer = publisher
            .create_datawriter(&topic, DataWriterQos::default(), ts.clone())
            .unwrap();

        let subscriber = participant
            .create_subscriber(SubscriberQos::default())
            .unwrap();
        let reader = subscriber
            .create_datareader(&topic, DataReaderQos::default(), ts)
            .unwrap();

        // Perform test pub/sub write/read
        let sample = HelloWorld {
            msg: "Hello World!".to_string(),
        };
        writer.write(&sample).unwrap();

        // Push data to reader manually to simulate RTPS dispatching
        let bytes = {
            let rtps = writer.rtps_writer.lock().unwrap();
            let change = rtps.writer_cache.get_changes().first().unwrap();
            change.data_value.to_vec()
        };
        reader.push_sample(dds_types::instance::InstanceHandle::NIL, bytes);

        let read_boxed = reader.read_next().unwrap();
        let read_hw = read_boxed.downcast_ref::<HelloWorld>().unwrap();
        assert_eq!(read_hw.msg, "Hello World!");
    }

    #[test]
    fn test_qos_compatibility() {
        use dds_types::qos::{Durability, DurabilityKind, Reliability, ReliabilityKind};

        let mut offered = DataWriterQos::default();
        let mut requested = DataReaderQos::default();

        // Default QoS should be compatible
        assert!(check_qos_compatibility(&offered, &requested));

        // Incompatible Durability: Offered Volatile, Requested TransientLocal
        offered.durability = Durability {
            kind: DurabilityKind::Volatile,
        };
        requested.durability = Durability {
            kind: DurabilityKind::TransientLocal,
        };
        assert!(!check_qos_compatibility(&offered, &requested));

        // Compatible Durability: Offered TransientLocal, Requested Volatile
        offered.durability = Durability {
            kind: DurabilityKind::TransientLocal,
        };
        requested.durability = Durability {
            kind: DurabilityKind::Volatile,
        };
        assert!(check_qos_compatibility(&offered, &requested));

        // Incompatible Reliability: Offered BestEffort, Requested Reliable
        offered.reliability = Reliability {
            kind: ReliabilityKind::BestEffort,
            max_blocking_time: dds_types::time::Duration::ZERO,
        };
        requested.reliability = Reliability {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: dds_types::time::Duration::ZERO,
        };
        assert!(!check_qos_compatibility(&offered, &requested));

        // Compatible Reliability again so we can test other things
        offered.reliability.kind = ReliabilityKind::Reliable;

        // Incompatible Ownership: Offered Shared, Requested Exclusive
        offered.ownership.kind = dds_types::qos::OwnershipKind::Shared;
        requested.ownership.kind = dds_types::qos::OwnershipKind::Exclusive;
        assert!(!check_qos_compatibility(&offered, &requested));
        offered.ownership.kind = dds_types::qos::OwnershipKind::Exclusive;
        assert!(check_qos_compatibility(&offered, &requested));
    }

    #[test]
    fn test_waitset_and_conditions() {
        let ws = WaitSet::new();
        let guard_cond = Arc::new(GuardCondition::new());

        ws.attach_condition(guard_cond.clone()).unwrap();

        // Check timeout if no conditions triggered
        let mut active = Vec::new();
        let res = ws.wait(&mut active, dds_types::time::Duration::from_millis(10));
        assert!(res.is_err()); // Timeout error

        // Set trigger value and wait again
        guard_cond.set_trigger_value(true);
        let mut active2 = Vec::new();
        ws.wait(&mut active2, dds_types::time::Duration::from_millis(100))
            .unwrap();
        assert_eq!(active2.len(), 1);
        assert!(Arc::ptr_eq(
            &active2[0],
            &(guard_cond as Arc<dyn Condition>)
        ));
    }

    struct TestReaderListener {
        call_count: Mutex<usize>,
    }
    impl Listener for TestReaderListener {}
    impl DataReaderListener for TestReaderListener {
        fn on_data_available(&self, _reader: &DataReader) {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
        }
    }

    #[test]
    fn test_reader_listener_callback() {
        let participant =
            DomainParticipantFactory::create_participant(0, DomainParticipantQos::default())
                .unwrap();
        let ts = Arc::new(HelloWorldTypeSupport);
        let topic = participant
            .create_topic("HelloTopic", "HelloWorld", TopicQos::default())
            .unwrap();
        let subscriber = participant
            .create_subscriber(SubscriberQos::default())
            .unwrap();
        let reader = subscriber
            .create_datareader(&topic, DataReaderQos::default(), ts)
            .unwrap();

        let listener = Arc::new(TestReaderListener {
            call_count: Mutex::new(0),
        });
        reader.set_listener(Some(listener.clone()));

        // Push a sample and check if call count incremented
        reader.push_sample(dds_types::instance::InstanceHandle::NIL, vec![1, 2, 3]);
        assert_eq!(*listener.call_count.lock().unwrap(), 1);
    }

    #[test]
    fn test_type_consistency_qos() {
        use dds_types::qos::{TypeConsistencyEnforcement, TypeConsistencyKind};
        use dds_xtypes::{ExtensibilityKind, StructureType, TypeObject};

        let offered = TypeObject::Complete(StructureType {
            name: "Point".to_string(),
            extensibility: ExtensibilityKind::Appendable,
            members: vec![],
        });
        let requested = TypeObject::Complete(StructureType {
            name: "Point".to_string(),
            extensibility: ExtensibilityKind::Final, // Incompatible
            members: vec![],
        });

        // 1. Kind: DisallowTypeCoercion -> Must match exactly
        let mut policy = TypeConsistencyEnforcement::default();
        policy.kind = TypeConsistencyKind::DisallowTypeCoercion;
        assert!(!check_type_compatibility(&policy, &offered, &requested));

        // 2. Kind: AllowTypeCoercion -> Assignable check
        policy.kind = TypeConsistencyKind::AllowTypeCoercion;
        // offered is Appendable, requested is Final -> not assignable requested from offered
        assert!(!check_type_compatibility(&policy, &offered, &requested));
    }
}
