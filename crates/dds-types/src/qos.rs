//! # QoS — Quality of Service Policies for DDS
//!
//! Defines all 22 QoS policies from the DDS DCPS 1.4 specification.
//! Each policy is a Rust struct with spec-defined default values.
//!
//! QoS policies control the behavior of DDS entities (participants, topics,
//! publishers, subscribers, writers, readers). Some policies are "RxO"
//! (Requested/Offered) — the middleware checks compatibility between a
//! DataWriter's offered QoS and a DataReader's requested QoS.
//!
//! Reference: DCPS §2.2.3 — Supported QoS

use crate::time::Duration;

// ──────────────────────────────────────────────────────────────────────────────
// Policy 1: UserData (DCPS §2.2.3.1)
// ──────────────────────────────────────────────────────────────────────────────

/// Arbitrary application data attached to a DomainParticipant, DataWriter,
/// or DataReader. Propagated via discovery.
///
/// Default: empty (no data).
/// Not RxO.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct UserData {
    /// Opaque byte payload — interpretation is application-defined.
    pub value: Vec<u8>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 2: TopicData (DCPS §2.2.3.2)
// ──────────────────────────────────────────────────────────────────────────────

/// Arbitrary application data attached to a Topic. Propagated via discovery.
///
/// Default: empty.
/// Not RxO.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TopicData {
    /// Opaque byte payload.
    pub value: Vec<u8>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 3: GroupData (DCPS §2.2.3.3)
// ──────────────────────────────────────────────────────────────────────────────

/// Arbitrary application data attached to a Publisher or Subscriber.
///
/// Default: empty.
/// Not RxO.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct GroupData {
    /// Opaque byte payload.
    pub value: Vec<u8>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 4: Durability (DCPS §2.2.3.4)
// ──────────────────────────────────────────────────────────────────────────────

/// Controls whether data is kept for late-joining readers.
///
/// Default: `Volatile`.
/// RxO: Yes — offered must be ≥ requested.
///
/// Ordering: Volatile < TransientLocal < Transient < Persistent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DurabilityKind {
    /// No durability — samples are not stored.
    Volatile = 0,
    /// Samples are stored in the DataWriter and sent to late-joiners.
    TransientLocal = 1,
    /// Samples are stored by a separate durability service.
    Transient = 2,
    /// Samples are stored persistently (survive process restart).
    Persistent = 3,
}

impl Default for DurabilityKind {
    fn default() -> Self {
        Self::Volatile
    }
}

/// Durability QoS policy wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Durability {
    /// The durability kind.
    pub kind: DurabilityKind,
}

impl Default for Durability {
    fn default() -> Self {
        Self {
            kind: DurabilityKind::default(),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 5: DurabilityService (DCPS §2.2.3.5) — Full Profile
// ──────────────────────────────────────────────────────────────────────────────

/// Configures the behavior of the durability service for Transient/Persistent
/// durability. Controls the history and resource limits of the service's
/// internal cache.
///
/// Default: service_cleanup_delay = 0, history = KEEP_LAST(1).
/// Not RxO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DurabilityService {
    /// Delay before the service cleans up instances.
    pub service_cleanup_delay: Duration,
    /// History kind for the service's internal cache.
    pub history_kind: HistoryKind,
    /// Depth when using KEEP_LAST.
    pub history_depth: i32,
    /// Maximum samples the service will store.
    pub max_samples: i32,
    /// Maximum instances the service will track.
    pub max_instances: i32,
    /// Maximum samples per instance.
    pub max_samples_per_instance: i32,
}

impl Default for DurabilityService {
    fn default() -> Self {
        Self {
            service_cleanup_delay: Duration::ZERO,
            history_kind: HistoryKind::KeepLast,
            history_depth: 1,
            max_samples: i32::MAX,
            max_instances: i32::MAX,
            max_samples_per_instance: i32::MAX,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 6: Deadline (DCPS §2.2.3.6)
// ──────────────────────────────────────────────────────────────────────────────

/// The maximum time interval between data updates for an instance.
/// If exceeded, a deadline-missed status is triggered.
///
/// Default: `Duration::INFINITE` (no deadline).
/// RxO: Yes — offered period must be ≤ requested period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Deadline {
    /// Maximum interval between successive writes for a given instance.
    pub period: Duration,
}

impl Default for Deadline {
    fn default() -> Self {
        Self {
            period: Duration::INFINITE,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 7: LatencyBudget (DCPS §2.2.3.7)
// ──────────────────────────────────────────────────────────────────────────────

/// A hint to the middleware about how urgently data should be delivered.
/// Does not guarantee latency — purely advisory.
///
/// Default: `Duration::ZERO` (deliver ASAP).
/// RxO: Yes — offered must be ≤ requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LatencyBudget {
    /// Maximum acceptable delay for data delivery.
    pub duration: Duration,
}

impl Default for LatencyBudget {
    fn default() -> Self {
        Self {
            duration: Duration::ZERO,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 8: Liveliness (DCPS §2.2.3.8)
// ──────────────────────────────────────────────────────────────────────────────

/// How the system detects that a DataWriter is still alive.
///
/// Default: `Automatic`, lease_duration = INFINITE.
/// RxO: Yes — offered kind must be ≥ requested kind, and offered
///      lease_duration must be ≤ requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LivelinessKind {
    /// The middleware automatically asserts liveliness.
    Automatic = 0,
    /// The participant must assert liveliness (any writer in the
    /// participant asserting counts for all).
    ManualByParticipant = 1,
    /// Each DataWriter must individually assert liveliness.
    ManualByTopic = 2,
}

impl Default for LivelinessKind {
    fn default() -> Self {
        Self::Automatic
    }
}

/// Liveliness QoS policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Liveliness {
    /// The liveliness assertion mechanism.
    pub kind: LivelinessKind,
    /// Maximum time between liveliness assertions.
    pub lease_duration: Duration,
}

impl Default for Liveliness {
    fn default() -> Self {
        Self {
            kind: LivelinessKind::default(),
            lease_duration: Duration::INFINITE,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 9: Reliability (DCPS §2.2.3.9)
// ──────────────────────────────────────────────────────────────────────────────

/// Whether data delivery is guaranteed (reliable) or best-effort.
///
/// Default for DataWriter: RELIABLE, max_blocking_time = 100ms.
/// Default for DataReader: BEST_EFFORT.
/// RxO: Yes — offered must be ≥ requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ReliabilityKind {
    /// No delivery guarantees — samples may be lost.
    BestEffort = 0,
    /// Guaranteed delivery — uses ACK/NACK protocol.
    Reliable = 1,
}

impl Default for ReliabilityKind {
    /// Note: Spec default differs for writers (Reliable) vs readers (BestEffort).
    /// This default is BestEffort; entity-specific defaults are applied in dds-core.
    fn default() -> Self {
        Self::BestEffort
    }
}

/// Reliability QoS policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Reliability {
    /// Reliable or best-effort delivery.
    pub kind: ReliabilityKind,
    /// Maximum blocking time for a reliable write when buffers are full.
    pub max_blocking_time: Duration,
}

impl Default for Reliability {
    fn default() -> Self {
        Self {
            kind: ReliabilityKind::default(),
            max_blocking_time: Duration::from_millis(100),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 10: DestinationOrder (DCPS §2.2.3.10)
// ──────────────────────────────────────────────────────────────────────────────

/// Order in which data is presented to the application.
///
/// Default: `ByReceptionTimestamp`.
/// RxO: Yes — offered must be ≥ requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DestinationOrderKind {
    /// Data is ordered by the time it was received.
    ByReceptionTimestamp = 0,
    /// Data is ordered by the source timestamp (requires synchronized clocks).
    BySourceTimestamp = 1,
}

impl Default for DestinationOrderKind {
    fn default() -> Self {
        Self::ByReceptionTimestamp
    }
}

/// Destination order QoS policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DestinationOrder {
    /// Ordering mechanism for data delivery.
    pub kind: DestinationOrderKind,
}

impl Default for DestinationOrder {
    fn default() -> Self {
        Self {
            kind: DestinationOrderKind::default(),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 11: History (DCPS §2.2.3.11)
// ──────────────────────────────────────────────────────────────────────────────

/// How many samples are kept in the middleware cache per instance.
///
/// Default: `KeepLast(1)`.
/// Not RxO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HistoryKind {
    /// Keep the last N samples per instance.
    KeepLast = 0,
    /// Keep all samples (bounded by ResourceLimits).
    KeepAll = 1,
}

impl Default for HistoryKind {
    fn default() -> Self {
        Self::KeepLast
    }
}

/// History QoS policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct History {
    /// Keep-last or keep-all strategy.
    pub kind: HistoryKind,
    /// Depth when using KeepLast (ignored for KeepAll).
    pub depth: i32,
}

impl Default for History {
    fn default() -> Self {
        Self {
            kind: HistoryKind::default(),
            depth: 1,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 12: ResourceLimits (DCPS §2.2.3.12)
// ──────────────────────────────────────────────────────────────────────────────

/// Limits on the resources consumed by an entity's cache.
///
/// Default: all limits are `LENGTH_UNLIMITED` (i32::MAX).
/// Not RxO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceLimits {
    /// Maximum total samples in the cache.
    pub max_samples: i32,
    /// Maximum instances tracked simultaneously.
    pub max_instances: i32,
    /// Maximum samples per individual instance.
    pub max_samples_per_instance: i32,
}

/// Sentinel value meaning "no limit" for resource limits.
pub const LENGTH_UNLIMITED: i32 = i32::MAX;

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_samples: LENGTH_UNLIMITED,
            max_instances: LENGTH_UNLIMITED,
            max_samples_per_instance: LENGTH_UNLIMITED,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 13: TransportPriority (DCPS §2.2.3.13) — Full Profile
// ──────────────────────────────────────────────────────────────────────────────

/// Hint to the transport layer for prioritizing data delivery.
/// Higher values indicate higher priority (maps to UDP TOS/DSCP).
///
/// Default: 0 (normal priority).
/// Not RxO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransportPriority {
    /// Priority value. Higher = more urgent.
    pub value: i32,
}

impl Default for TransportPriority {
    fn default() -> Self {
        Self { value: 0 }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 14: Lifespan (DCPS §2.2.3.14)
// ──────────────────────────────────────────────────────────────────────────────

/// Duration for which a data sample is considered valid.
/// Expired samples are automatically removed from caches.
///
/// Default: `Duration::INFINITE` (samples never expire).
/// Not RxO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Lifespan {
    /// Maximum validity duration for a sample.
    pub duration: Duration,
}

impl Default for Lifespan {
    fn default() -> Self {
        Self {
            duration: Duration::INFINITE,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 15: Ownership (DCPS §2.2.3.15)
// ──────────────────────────────────────────────────────────────────────────────

/// Controls whether multiple DataWriters can update the same instance.
///
/// Default: `Shared`.
/// RxO: Yes — must match exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OwnershipKind {
    /// Multiple writers can update the same instance concurrently.
    Shared = 0,
    /// Only the highest-strength writer owns a given instance.
    Exclusive = 1,
}

impl Default for OwnershipKind {
    fn default() -> Self {
        Self::Shared
    }
}

/// Ownership QoS policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ownership {
    /// Shared or exclusive ownership mode.
    pub kind: OwnershipKind,
}

impl Default for Ownership {
    fn default() -> Self {
        Self {
            kind: OwnershipKind::default(),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 16: OwnershipStrength (DCPS §2.2.3.16)
// ──────────────────────────────────────────────────────────────────────────────

/// Priority of a DataWriter when using Exclusive ownership.
/// Only meaningful when `Ownership::kind` is `Exclusive`.
///
/// Default: 0.
/// Not RxO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OwnershipStrength {
    /// Strength value. Higher wins when competing for ownership.
    pub value: i32,
}

impl Default for OwnershipStrength {
    fn default() -> Self {
        Self { value: 0 }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 17: Presentation (DCPS §2.2.3.17) — Full Profile
// ──────────────────────────────────────────────────────────────────────────────

/// Controls the scope of coherent changes and ordered access.
///
/// Default: `Instance` scope, coherent_access = false, ordered_access = false.
/// RxO: Yes — offered scope must be ≥ requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PresentationAccessScopeKind {
    /// Changes are scoped to individual instances.
    Instance = 0,
    /// Changes are scoped to a single Topic.
    Topic = 1,
    /// Changes are scoped to a Publisher/Subscriber group.
    Group = 2,
}

impl Default for PresentationAccessScopeKind {
    fn default() -> Self {
        Self::Instance
    }
}

/// Presentation QoS policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Presentation {
    /// Scope of coherent/ordered access.
    pub access_scope: PresentationAccessScopeKind,
    /// Whether begin/end_coherent_changes is supported.
    pub coherent_access: bool,
    /// Whether begin/end_access provides ordered delivery.
    pub ordered_access: bool,
}

impl Default for Presentation {
    fn default() -> Self {
        Self {
            access_scope: PresentationAccessScopeKind::default(),
            coherent_access: false,
            ordered_access: false,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 18: Partition (DCPS §2.2.3.18)
// ──────────────────────────────────────────────────────────────────────────────

/// Logical partitioning of the data space. Only writers and readers with
/// at least one matching partition name can communicate.
///
/// Default: empty (matches all partitions).
/// Not RxO (but matching is required for communication).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Partition {
    /// List of partition name strings. Empty = default partition ("").
    pub name: Vec<String>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 19: TimeBasedFilter (DCPS §2.2.3.19)
// ──────────────────────────────────────────────────────────────────────────────

/// Limits the rate at which a DataReader receives data.
/// The reader will not see samples arriving faster than `minimum_separation`.
///
/// Default: `Duration::ZERO` (no filtering).
/// Not RxO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeBasedFilter {
    /// Minimum interval between successive samples delivered to the reader.
    pub minimum_separation: Duration,
}

impl Default for TimeBasedFilter {
    fn default() -> Self {
        Self {
            minimum_separation: Duration::ZERO,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 20: EntityFactory (DCPS §2.2.3.20)
// ──────────────────────────────────────────────────────────────────────────────

/// Controls whether entities are automatically enabled upon creation.
///
/// Default: `autoenable_created_entities = true`.
/// Not RxO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityFactory {
    /// If true, newly created entities are automatically enabled.
    pub autoenable_created_entities: bool,
}

impl Default for EntityFactory {
    fn default() -> Self {
        Self {
            autoenable_created_entities: true,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy: Property (RTPS §8.2.4, DDS Security §9.1)
// ──────────────────────────────────────────────────────────────────────────────

/// A name/value property pair used to pass configuration to the middleware
/// and security plugins through `DomainParticipantQos`.
///
/// Standard property key prefixes:
/// - `dds.sec.auth.*`   — Authentication plugin configuration
/// - `dds.sec.access.*` — Access control plugin configuration
/// - `dds.sec.crypto.*` — Cryptography plugin configuration
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Property {
    /// List of (name, value) string pairs.
    pub value: Vec<(String, String)>,
}

impl Property {
    /// Look up a property value by name. Returns `None` if not set.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.value
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// Insert or overwrite a property.
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        if let Some(entry) = self.value.iter_mut().find(|(k, _)| k == &name) {
            entry.1 = value.into();
        } else {
            self.value.push((name, value.into()));
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 21: WriterDataLifecycle (DCPS §2.2.3.21)
// ──────────────────────────────────────────────────────────────────────────────

/// Controls DataWriter behavior when instances are unregistered.
///
/// Default: `autodispose_unregistered_instances = true`.
/// Not RxO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WriterDataLifecycle {
    /// If true, unregistering an instance automatically disposes it.
    pub autodispose_unregistered_instances: bool,
}

impl Default for WriterDataLifecycle {
    fn default() -> Self {
        Self {
            autodispose_unregistered_instances: true,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy 22: ReaderDataLifecycle (DCPS §2.2.3.22)
// ──────────────────────────────────────────────────────────────────────────────

/// Controls how a DataReader manages the lifecycle of received instances.
/// Automatic purging reclaims resources for instances that are no longer active.
///
/// Default: both delays are INFINITE (no automatic purging).
/// Not RxO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReaderDataLifecycle {
    /// Delay before purging samples for an instance with no active writers.
    pub autopurge_nowriter_samples_delay: Duration,
    /// Delay before purging samples for a disposed instance.
    pub autopurge_disposed_samples_delay: Duration,
}

impl Default for ReaderDataLifecycle {
    fn default() -> Self {
        Self {
            autopurge_nowriter_samples_delay: Duration::INFINITE,
            autopurge_disposed_samples_delay: Duration::INFINITE,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Aggregate QoS structs per entity type
// ──────────────────────────────────────────────────────────────────────────────

/// QoS policies applicable to a `DomainParticipant`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct DomainParticipantQos {
    /// Application-specific data for discovery.
    pub user_data: UserData,
    /// Whether child entities are auto-enabled.
    pub entity_factory: EntityFactory,
    /// Key/value property pairs for middleware and security plugin configuration.
    /// Standard prefixes: `dds.sec.auth.*`, `dds.sec.access.*`, `dds.sec.crypto.*`.
    pub property: Property,
}

/// QoS policies applicable to a `Topic`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TopicQos {
    /// Application-specific topic metadata.
    pub topic_data: TopicData,
    /// Durability level for topic data.
    pub durability: Durability,
    /// Durability service configuration (Full Profile).
    pub durability_service: DurabilityService,
    /// Maximum interval between data updates.
    pub deadline: Deadline,
    /// Delivery urgency hint.
    pub latency_budget: LatencyBudget,
    /// Liveliness detection mechanism.
    pub liveliness: Liveliness,
    /// Guaranteed or best-effort delivery.
    pub reliability: Reliability,
    /// Data ordering for the reader.
    pub destination_order: DestinationOrder,
    /// Cache depth strategy.
    pub history: History,
    /// Cache resource limits.
    pub resource_limits: ResourceLimits,
    /// Transport-level priority hint (Full Profile).
    pub transport_priority: TransportPriority,
    /// Sample validity duration.
    pub lifespan: Lifespan,
    /// Concurrent writer policy.
    pub ownership: Ownership,
}

/// QoS policies applicable to a `Publisher`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct PublisherQos {
    /// Presentation scope for coherent/ordered access.
    pub presentation: Presentation,
    /// Logical partitioning.
    pub partition: Partition,
    /// Application-specific group data.
    pub group_data: GroupData,
    /// Whether child entities are auto-enabled.
    pub entity_factory: EntityFactory,
}

/// QoS policies applicable to a `Subscriber`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SubscriberQos {
    /// Presentation scope for coherent/ordered access.
    pub presentation: Presentation,
    /// Logical partitioning.
    pub partition: Partition,
    /// Application-specific group data.
    pub group_data: GroupData,
    /// Whether child entities are auto-enabled.
    pub entity_factory: EntityFactory,
}

/// QoS policies applicable to a `DataWriter`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct DataWriterQos {
    /// Durability level.
    pub durability: Durability,
    /// Durability service config (Full Profile).
    pub durability_service: DurabilityService,
    /// Maximum interval between updates.
    pub deadline: Deadline,
    /// Delivery urgency hint.
    pub latency_budget: LatencyBudget,
    /// Liveliness mechanism.
    pub liveliness: Liveliness,
    /// Delivery guarantee.
    pub reliability: Reliability,
    /// Ordering policy.
    pub destination_order: DestinationOrder,
    /// Cache depth.
    pub history: History,
    /// Cache limits.
    pub resource_limits: ResourceLimits,
    /// Transport priority (Full Profile).
    pub transport_priority: TransportPriority,
    /// Sample validity.
    pub lifespan: Lifespan,
    /// Application-specific data for discovery.
    pub user_data: UserData,
    /// Ownership mode.
    pub ownership: Ownership,
    /// Writer priority for exclusive ownership.
    pub ownership_strength: OwnershipStrength,
    /// Lifecycle behavior on unregister.
    pub writer_data_lifecycle: WriterDataLifecycle,
}

/// QoS policies applicable to a `DataReader`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct DataReaderQos {
    /// Durability level.
    pub durability: Durability,
    /// Maximum interval between updates.
    pub deadline: Deadline,
    /// Delivery urgency hint.
    pub latency_budget: LatencyBudget,
    /// Liveliness mechanism.
    pub liveliness: Liveliness,
    /// Delivery guarantee.
    pub reliability: Reliability,
    /// Ordering policy.
    pub destination_order: DestinationOrder,
    /// Cache depth.
    pub history: History,
    /// Cache limits.
    pub resource_limits: ResourceLimits,
    /// Application-specific data.
    pub user_data: UserData,
    /// Ownership mode.
    pub ownership: Ownership,
    /// Minimum interval between delivered samples.
    pub time_based_filter: TimeBasedFilter,
    /// Instance purging behavior.
    pub reader_data_lifecycle: ReaderDataLifecycle,
    /// Type consistency enforcement configuration.
    pub type_consistency: TypeConsistencyEnforcement,
}

// ──────────────────────────────────────────────────────────────────────────────
// Policy: TypeConsistencyEnforcement (XTypes §7.6.1)
// ──────────────────────────────────────────────────────────────────────────────

/// Policy kind controlling whether type coercion is allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeConsistencyKind {
    /// Types must be identical.
    DisallowTypeCoercion = 0,
    /// Types are assignable based on evolution rules.
    AllowTypeCoercion = 1,
}

/// TypeConsistencyEnforcement QoS policy wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeConsistencyEnforcement {
    /// The enforcement kind.
    pub kind: TypeConsistencyKind,
    /// Force validation of TypeObject structure layout.
    pub force_type_validation: bool,
    /// Prevent assignment of a wider type to a narrower type.
    pub prevent_type_widening: bool,
    /// Ignore array or sequence bound differences during match.
    pub ignore_sequence_bounds: bool,
    /// Ignore string bounds during match.
    pub ignore_string_bounds: bool,
    /// Ignore member name mismatches.
    pub ignore_member_names: bool,
}

impl Default for TypeConsistencyEnforcement {
    fn default() -> Self {
        Self {
            kind: TypeConsistencyKind::AllowTypeCoercion,
            force_type_validation: true,
            prevent_type_widening: false,
            ignore_sequence_bounds: true,
            ignore_string_bounds: true,
            ignore_member_names: false,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Default value tests (all per-spec) ──

    #[test]
    fn durability_default_is_volatile() {
        assert_eq!(Durability::default().kind, DurabilityKind::Volatile);
    }

    #[test]
    fn deadline_default_is_infinite() {
        assert!(Deadline::default().period.is_infinite());
    }

    #[test]
    fn latency_budget_default_is_zero() {
        assert!(LatencyBudget::default().duration.is_zero());
    }

    #[test]
    fn liveliness_default_automatic_infinite() {
        let l = Liveliness::default();
        assert_eq!(l.kind, LivelinessKind::Automatic);
        assert!(l.lease_duration.is_infinite());
    }

    #[test]
    fn reliability_default_is_best_effort() {
        assert_eq!(Reliability::default().kind, ReliabilityKind::BestEffort);
    }

    #[test]
    fn reliability_default_blocking_time() {
        let r = Reliability::default();
        assert_eq!(r.max_blocking_time, Duration::from_millis(100));
    }

    #[test]
    fn destination_order_default_by_reception() {
        assert_eq!(
            DestinationOrder::default().kind,
            DestinationOrderKind::ByReceptionTimestamp
        );
    }

    #[test]
    fn history_default_keep_last_1() {
        let h = History::default();
        assert_eq!(h.kind, HistoryKind::KeepLast);
        assert_eq!(h.depth, 1);
    }

    #[test]
    fn resource_limits_default_unlimited() {
        let r = ResourceLimits::default();
        assert_eq!(r.max_samples, LENGTH_UNLIMITED);
        assert_eq!(r.max_instances, LENGTH_UNLIMITED);
        assert_eq!(r.max_samples_per_instance, LENGTH_UNLIMITED);
    }

    #[test]
    fn transport_priority_default_zero() {
        assert_eq!(TransportPriority::default().value, 0);
    }

    #[test]
    fn lifespan_default_infinite() {
        assert!(Lifespan::default().duration.is_infinite());
    }

    #[test]
    fn ownership_default_shared() {
        assert_eq!(Ownership::default().kind, OwnershipKind::Shared);
    }

    #[test]
    fn ownership_strength_default_zero() {
        assert_eq!(OwnershipStrength::default().value, 0);
    }

    #[test]
    fn presentation_default_instance_scope() {
        let p = Presentation::default();
        assert_eq!(p.access_scope, PresentationAccessScopeKind::Instance);
        assert!(!p.coherent_access);
        assert!(!p.ordered_access);
    }

    #[test]
    fn partition_default_empty() {
        assert!(Partition::default().name.is_empty());
    }

    #[test]
    fn time_based_filter_default_zero() {
        assert!(TimeBasedFilter::default().minimum_separation.is_zero());
    }

    #[test]
    fn entity_factory_default_autoenable() {
        assert!(EntityFactory::default().autoenable_created_entities);
    }

    #[test]
    fn writer_lifecycle_default_autodispose() {
        assert!(WriterDataLifecycle::default().autodispose_unregistered_instances);
    }

    #[test]
    fn reader_lifecycle_default_infinite_delays() {
        let r = ReaderDataLifecycle::default();
        assert!(r.autopurge_nowriter_samples_delay.is_infinite());
        assert!(r.autopurge_disposed_samples_delay.is_infinite());
    }

    #[test]
    fn user_data_default_empty() {
        assert!(UserData::default().value.is_empty());
    }

    #[test]
    fn topic_data_default_empty() {
        assert!(TopicData::default().value.is_empty());
    }

    #[test]
    fn group_data_default_empty() {
        assert!(GroupData::default().value.is_empty());
    }

    // ── Aggregate QoS tests ──

    #[test]
    fn participant_qos_default() {
        let qos = DomainParticipantQos::default();
        assert!(qos.user_data.value.is_empty());
        assert!(qos.entity_factory.autoenable_created_entities);
    }

    #[test]
    fn topic_qos_default() {
        let qos = TopicQos::default();
        assert_eq!(qos.durability.kind, DurabilityKind::Volatile);
        assert_eq!(qos.reliability.kind, ReliabilityKind::BestEffort);
        assert_eq!(qos.history.depth, 1);
    }

    #[test]
    fn writer_qos_default() {
        let qos = DataWriterQos::default();
        assert_eq!(qos.durability.kind, DurabilityKind::Volatile);
        assert!(qos.writer_data_lifecycle.autodispose_unregistered_instances);
    }

    #[test]
    fn reader_qos_default() {
        let qos = DataReaderQos::default();
        assert_eq!(qos.durability.kind, DurabilityKind::Volatile);
        assert!(qos
            .reader_data_lifecycle
            .autopurge_nowriter_samples_delay
            .is_infinite());
    }

    // ── RxO ordering tests ──

    #[test]
    fn durability_kind_ordering() {
        // Volatile < TransientLocal < Transient < Persistent
        assert!(DurabilityKind::Volatile < DurabilityKind::TransientLocal);
        assert!(DurabilityKind::TransientLocal < DurabilityKind::Transient);
        assert!(DurabilityKind::Transient < DurabilityKind::Persistent);
    }

    #[test]
    fn reliability_kind_ordering() {
        // BestEffort < Reliable
        assert!(ReliabilityKind::BestEffort < ReliabilityKind::Reliable);
    }

    #[test]
    fn liveliness_kind_ordering() {
        // Automatic < ManualByParticipant < ManualByTopic
        assert!(LivelinessKind::Automatic < LivelinessKind::ManualByParticipant);
        assert!(LivelinessKind::ManualByParticipant < LivelinessKind::ManualByTopic);
    }

    #[test]
    fn destination_order_kind_ordering() {
        assert!(
            DestinationOrderKind::ByReceptionTimestamp < DestinationOrderKind::BySourceTimestamp
        );
    }

    #[test]
    fn presentation_scope_ordering() {
        assert!(PresentationAccessScopeKind::Instance < PresentationAccessScopeKind::Topic);
        assert!(PresentationAccessScopeKind::Topic < PresentationAccessScopeKind::Group);
    }

    // ── DurabilityService tests ──

    #[test]
    fn durability_service_default() {
        let ds = DurabilityService::default();
        assert!(ds.service_cleanup_delay.is_zero());
        assert_eq!(ds.history_kind, HistoryKind::KeepLast);
        assert_eq!(ds.history_depth, 1);
        assert_eq!(ds.max_samples, i32::MAX);
    }
}
