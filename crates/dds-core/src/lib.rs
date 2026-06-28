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

use dds_types::guid::{EntityId, EntityKind, Guid, GuidPrefix};
use dds_types::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, PublisherQos, SubscriberQos, TopicQos,
};
use dds_types::return_code::{DdsError, DdsResult};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
pub struct DataWriter {
    guid: Guid,
    topic: Topic,
    qos: DataWriterQos,
    type_support: Arc<dyn TypeSupport>,
    // Simulates an internal writer cache or RTPS Stateless/Stateful writer integration
    history: Mutex<Vec<(dds_types::instance::InstanceHandle, Vec<u8>)>>,
    listener: Mutex<Option<Arc<dyn DataWriterListener>>>,
}

impl DataWriter {
    /// Write a sample to the DDS network.
    pub fn write(&self, value: &dyn core::any::Any) -> DdsResult<()> {
        let serialized = self.type_support.serialize(value)?;
        let key_hash = self.type_support.get_key_hash(value)?;

        let mut history = self.history.lock().unwrap();
        history.push((key_hash, serialized));
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
    samples: Mutex<Vec<(dds_types::instance::InstanceHandle, Vec<u8>)>>,
    listener: Mutex<Option<Arc<dyn DataReaderListener>>>,
}

impl DataReader {
    /// Read the next available sample.
    pub fn read_next(&self) -> DdsResult<Box<dyn core::any::Any>> {
        let mut samples = self.samples.lock().unwrap();
        if samples.is_empty() {
            return Err(DdsError::NoData);
        }
        let (_, bytes) = samples.remove(0);
        self.type_support.deserialize(&bytes)
    }

    /// Set a listener to receive callbacks.
    pub fn set_listener(&self, listener: Option<Arc<dyn DataReaderListener>>) {
        let mut l = self.listener.lock().unwrap();
        *l = listener;
    }

    /// Push a received packet into the reader's cache (simulating RTPS matching).
    pub fn push_sample(&self, key: dds_types::instance::InstanceHandle, bytes: Vec<u8>) {
        {
            let mut samples = self.samples.lock().unwrap();
            samples.push((key, bytes));
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
}

// ──────────────────────────────────────────────────────────────────────────────
// Publisher (DCPS §2.2.2.4.1)
// ──────────────────────────────────────────────────────────────────────────────

/// Publisher coordinates writing data samples for various topics.
pub struct Publisher {
    guid: Guid,
    qos: PublisherQos,
    writers: Mutex<HashMap<Guid, Arc<DataWriter>>>,
}

impl Publisher {
    #[must_use]
    pub const fn qos(&self) -> &PublisherQos {
        &self.qos
    }

    /// Create a `DataWriter` for the given Topic.
    pub fn create_datawriter(
        &self,
        topic: &Topic,
        qos: DataWriterQos,
        type_support: Arc<dyn TypeSupport>,
    ) -> DdsResult<Arc<DataWriter>> {
        let mut writers = self.writers.lock().unwrap();
        let writer_entity_id = EntityId::new([
            0x00,
            0x00,
            (writers.len() + 1) as u8,
            EntityKind::WriterWithKey as u8,
        ]);
        let writer_guid = Guid::new(self.guid.prefix, writer_entity_id);

        let writer = Arc::new(DataWriter {
            guid: writer_guid,
            topic: topic.clone(),
            qos,
            type_support,
            history: Mutex::new(Vec::new()),
            listener: Mutex::new(None),
        });

        writers.insert(writer_guid, writer.clone());
        Ok(writer)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Subscriber (DCPS §2.2.2.5.1)
// ──────────────────────────────────────────────────────────────────────────────

/// Subscriber coordinates reading data samples for various topics.
pub struct Subscriber {
    guid: Guid,
    qos: SubscriberQos,
    readers: Mutex<HashMap<Guid, Arc<DataReader>>>,
}

impl Subscriber {
    #[must_use]
    pub const fn qos(&self) -> &SubscriberQos {
        &self.qos
    }

    /// Create a `DataReader` for the given Topic.
    pub fn create_datareader(
        &self,
        topic: &Topic,
        qos: DataReaderQos,
        type_support: Arc<dyn TypeSupport>,
    ) -> DdsResult<Arc<DataReader>> {
        let mut readers = self.readers.lock().unwrap();
        let reader_entity_id = EntityId::new([
            0x00,
            0x00,
            (readers.len() + 1) as u8,
            EntityKind::ReaderWithKey as u8,
        ]);
        let reader_guid = Guid::new(self.guid.prefix, reader_entity_id);

        let reader = Arc::new(DataReader {
            guid: reader_guid,
            topic: topic.clone(),
            qos,
            type_support,
            samples: Mutex::new(Vec::new()),
            listener: Mutex::new(None),
        });

        readers.insert(reader_guid, reader.clone());
        Ok(reader)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// DomainParticipant (DCPS §2.2.2.2.1)
// ──────────────────────────────────────────────────────────────────────────────

/// Represents a local Participant containing topics, publishers, and subscribers.
pub struct DomainParticipant {
    guid_prefix: GuidPrefix,
    qos: DomainParticipantQos,
    topics: Mutex<HashMap<String, Topic>>,
    types: Mutex<HashMap<String, Arc<dyn TypeSupport>>>,
}

impl DomainParticipant {
    #[must_use]
    pub fn new(guid_prefix: GuidPrefix, qos: DomainParticipantQos) -> Self {
        Self {
            guid_prefix,
            qos,
            topics: Mutex::new(HashMap::new()),
            types: Mutex::new(HashMap::new()),
        }
    }

    #[must_use]
    pub const fn qos(&self) -> &DomainParticipantQos {
        &self.qos
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

    /// Create a Publisher.
    pub fn create_publisher(&self, qos: PublisherQos) -> DdsResult<Publisher> {
        let pub_guid = Guid::new(
            self.guid_prefix,
            EntityId::new([0, 0, 1, EntityKind::BuiltinParticipant as u8]),
        );
        Ok(Publisher {
            guid: pub_guid,
            qos,
            writers: Mutex::new(HashMap::new()),
        })
    }

    /// Create a Subscriber.
    pub fn create_subscriber(&self, qos: SubscriberQos) -> DdsResult<Subscriber> {
        let sub_guid = Guid::new(
            self.guid_prefix,
            EntityId::new([0, 0, 2, EntityKind::BuiltinParticipant as u8]),
        );
        Ok(Subscriber {
            guid: sub_guid,
            qos,
            readers: Mutex::new(HashMap::new()),
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
    pub fn create_participant(
        domain_id: u32,
        qos: DomainParticipantQos,
    ) -> DdsResult<DomainParticipant> {
        // Derive a unique GUID prefix using domain_id
        let mut prefix = [0u8; 12];
        prefix[0..4].copy_from_slice(&domain_id.to_be_bytes());
        // Add random or process-specific identifier bytes
        prefix[4..8].copy_from_slice(&std::process::id().to_be_bytes());
        Ok(DomainParticipant::new(GuidPrefix::new(prefix), qos))
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
        let bytes = writer.history.lock().unwrap().first().cloned().unwrap().1;
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

        let offered = TypeObject::Structure(StructureType {
            name: "Point".to_string(),
            extensibility: ExtensibilityKind::Appendable,
            members: vec![],
        });
        let requested = TypeObject::Structure(StructureType {
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
