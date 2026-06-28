//! # Status — Communication Status Types for DDS Entities
//!
//! Each DDS entity maintains a set of status flags that the application
//! can monitor via listeners or wait sets. These types represent the
//! data carried in each status change notification.
//!
//! Reference: DCPS §2.2.4 — Communication Status

use crate::instance::InstanceHandle;

// ──────────────────────────────────────────────────────────────────────────────
// QoS Policy identification (for status reporting)
// ──────────────────────────────────────────────────────────────────────────────

// Forward declaration: QosPolicyId is used in status types but defined here
// for self-containment. The qos module will reference this.

/// Identifies individual QoS policies in incompatibility reports.
///
/// Each numeric value corresponds to a specific QoS policy. These IDs
/// are used in `OfferedIncompatibleQos` and `RequestedIncompatibleQos`
/// status to indicate which policy failed matching.
///
/// Reference: DCPS §2.2.3, Table 2.10
pub use self::policy_id::QosPolicyId;

mod policy_id {
    /// Numeric identifier for a QoS policy, used in incompatibility reports.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(i32)]
    pub enum QosPolicyId {
        /// Invalid/unknown policy.
        Invalid = 0,
        /// UserData policy.
        UserData = 1,
        /// Durability policy.
        Durability = 2,
        /// Presentation policy.
        Presentation = 3,
        /// Deadline policy.
        Deadline = 4,
        /// LatencyBudget policy.
        LatencyBudget = 5,
        /// Ownership policy.
        Ownership = 6,
        /// OwnershipStrength policy.
        OwnershipStrength = 7,
        /// Liveliness policy.
        Liveliness = 8,
        /// TimeBasedFilter policy.
        TimeBasedFilter = 9,
        /// Partition policy.
        Partition = 10,
        /// Reliability policy.
        Reliability = 11,
        /// DestinationOrder policy.
        DestinationOrder = 12,
        /// History policy.
        History = 13,
        /// ResourceLimits policy.
        ResourceLimits = 14,
        /// EntityFactory policy.
        EntityFactory = 15,
        /// WriterDataLifecycle policy.
        WriterDataLifecycle = 16,
        /// ReaderDataLifecycle policy.
        ReaderDataLifecycle = 17,
        /// TopicData policy.
        TopicData = 18,
        /// GroupData policy.
        GroupData = 19,
        /// TransportPriority policy.
        TransportPriority = 20,
        /// Lifespan policy.
        Lifespan = 21,
        /// DurabilityService policy.
        DurabilityService = 22,
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Status types (DCPS §2.2.4)
// ──────────────────────────────────────────────────────────────────────────────

/// A count with a delta since last read, used in most status types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StatusCount {
    /// Cumulative count since entity creation.
    pub total_count: i32,
    /// Change since the last time the status was read.
    pub total_count_change: i32,
}

/// Inconsistent topic status — triggered when a topic with the same name
/// but incompatible type is discovered.
///
/// Applies to: Topic.
/// Reference: DCPS §2.2.4.1
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InconsistentTopicStatus {
    /// Count of inconsistent discoveries.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
}

/// Offered deadline missed — the DataWriter failed to write within
/// the deadline period for an instance.
///
/// Applies to: DataWriter.
/// Reference: DCPS §2.2.4.2
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OfferedDeadlineMissedStatus {
    /// Cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
    /// Handle of the last instance that missed its deadline.
    pub last_instance_handle: InstanceHandle,
}

/// Requested deadline missed — the DataReader did not receive data
/// within the deadline period for an instance.
///
/// Applies to: DataReader.
/// Reference: DCPS §2.2.4.3
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RequestedDeadlineMissedStatus {
    /// Cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
    /// Handle of the last instance that missed its deadline.
    pub last_instance_handle: InstanceHandle,
}

/// Offered incompatible QoS — a DataWriter's offered QoS is incompatible
/// with a DataReader's requested QoS.
///
/// Applies to: DataWriter.
/// Reference: DCPS §2.2.4.4
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OfferedIncompatibleQosStatus {
    /// Cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
    /// The ID of the last QoS policy that caused incompatibility.
    pub last_policy_id: Option<QosPolicyId>,
    /// Per-policy counts of incompatibilities.
    pub policies: Vec<QosPolicyCount>,
}

/// Requested incompatible QoS — a DataReader's requested QoS is
/// incompatible with a DataWriter's offered QoS.
///
/// Applies to: DataReader.
/// Reference: DCPS §2.2.4.5
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RequestedIncompatibleQosStatus {
    /// Cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
    /// The ID of the last QoS policy that caused incompatibility.
    pub last_policy_id: Option<QosPolicyId>,
    /// Per-policy counts of incompatibilities.
    pub policies: Vec<QosPolicyCount>,
}

/// Per-policy incompatibility counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QosPolicyCount {
    /// Which policy was incompatible.
    pub policy_id: QosPolicyId,
    /// How many times this policy caused an incompatibility.
    pub count: i32,
}

/// Sample lost status — samples were lost (never delivered to the reader).
///
/// Applies to: DataReader.
/// Reference: DCPS §2.2.4.6
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SampleLostStatus {
    /// Cumulative count of lost samples.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
}

/// Sample rejected status — samples were rejected (e.g., due to
/// resource limits).
///
/// Applies to: DataReader.
/// Reference: DCPS §2.2.4.7
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SampleRejectedStatus {
    /// Cumulative count of rejected samples.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
    /// Reason for the last rejection.
    pub last_reason: SampleRejectedKind,
    /// Handle of the instance whose sample was rejected.
    pub last_instance_handle: InstanceHandle,
}

/// Reason why a sample was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleRejectedKind {
    /// Not rejected.
    NotRejected,
    /// Rejected because max_instances limit was reached.
    RejectedByInstancesLimit,
    /// Rejected because max_samples limit was reached.
    RejectedBySamplesLimit,
    /// Rejected because max_samples_per_instance limit was reached.
    RejectedBySamplesPerInstanceLimit,
}

impl Default for SampleRejectedKind {
    fn default() -> Self {
        Self::NotRejected
    }
}

/// Liveliness changed — the liveliness of one or more DataWriters
/// matching this DataReader has changed.
///
/// Applies to: DataReader.
/// Reference: DCPS §2.2.4.8
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LivelinessChangedStatus {
    /// Number of currently alive writers.
    pub alive_count: i32,
    /// Number of currently not-alive writers.
    pub not_alive_count: i32,
    /// Change in alive_count since last read.
    pub alive_count_change: i32,
    /// Change in not_alive_count since last read.
    pub not_alive_count_change: i32,
    /// Handle of the last writer whose liveliness changed.
    pub last_publication_handle: InstanceHandle,
}

/// Liveliness lost — the DataWriter failed to assert its liveliness
/// within the lease duration.
///
/// Applies to: DataWriter.
/// Reference: DCPS §2.2.4.9
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LivelinessLostStatus {
    /// Cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
}

/// Publication matched — a new DataReader matched (or unmatched) this
/// DataWriter.
///
/// Applies to: DataWriter.
/// Reference: DCPS §2.2.4.10
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PublicationMatchedStatus {
    /// Cumulative count of matches.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
    /// Current number of matched readers.
    pub current_count: i32,
    /// Change in current_count since last read.
    pub current_count_change: i32,
    /// Handle of the last reader that matched or unmatched.
    pub last_subscription_handle: InstanceHandle,
}

/// Subscription matched — a new DataWriter matched (or unmatched) this
/// DataReader.
///
/// Applies to: DataReader.
/// Reference: DCPS §2.2.4.11
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SubscriptionMatchedStatus {
    /// Cumulative count of matches.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
    /// Current number of matched writers.
    pub current_count: i32,
    /// Change in current_count since last read.
    pub current_count_change: i32,
    /// Handle of the last writer that matched or unmatched.
    pub last_publication_handle: InstanceHandle,
}

// ──────────────────────────────────────────────────────────────────────────────
// Status mask — bitflags for selecting which statuses to monitor
// ──────────────────────────────────────────────────────────────────────────────

/// Bitmask identifying which communication statuses to monitor.
/// Used with WaitSets and StatusConditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StatusMask(pub u32);

impl StatusMask {
    /// No statuses selected.
    pub const NONE: Self = Self(0);
    /// All statuses selected.
    pub const ALL: Self = Self(u32::MAX);

    // Individual status bits
    /// Inconsistent topic.
    pub const INCONSISTENT_TOPIC: Self = Self(1 << 0);
    /// Offered deadline missed.
    pub const OFFERED_DEADLINE_MISSED: Self = Self(1 << 1);
    /// Requested deadline missed.
    pub const REQUESTED_DEADLINE_MISSED: Self = Self(1 << 2);
    /// Offered incompatible QoS.
    pub const OFFERED_INCOMPATIBLE_QOS: Self = Self(1 << 5);
    /// Requested incompatible QoS.
    pub const REQUESTED_INCOMPATIBLE_QOS: Self = Self(1 << 6);
    /// Sample lost.
    pub const SAMPLE_LOST: Self = Self(1 << 7);
    /// Sample rejected.
    pub const SAMPLE_REJECTED: Self = Self(1 << 8);
    /// Data on readers (data available).
    pub const DATA_ON_READERS: Self = Self(1 << 9);
    /// Data available.
    pub const DATA_AVAILABLE: Self = Self(1 << 10);
    /// Liveliness lost (writer).
    pub const LIVELINESS_LOST: Self = Self(1 << 11);
    /// Liveliness changed (reader).
    pub const LIVELINESS_CHANGED: Self = Self(1 << 12);
    /// Publication matched.
    pub const PUBLICATION_MATCHED: Self = Self(1 << 13);
    /// Subscription matched.
    pub const SUBSCRIPTION_MATCHED: Self = Self(1 << 14);

    /// Check if a specific status bit is set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Combine two masks (bitwise OR).
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_mask_none_contains_nothing() {
        assert!(!StatusMask::NONE.contains(StatusMask::DATA_AVAILABLE));
    }

    #[test]
    fn status_mask_all_contains_everything() {
        assert!(StatusMask::ALL.contains(StatusMask::DATA_AVAILABLE));
        assert!(StatusMask::ALL.contains(StatusMask::LIVELINESS_CHANGED));
        assert!(StatusMask::ALL.contains(StatusMask::PUBLICATION_MATCHED));
    }

    #[test]
    fn status_mask_union() {
        let mask = StatusMask::DATA_AVAILABLE.union(StatusMask::LIVELINESS_CHANGED);
        assert!(mask.contains(StatusMask::DATA_AVAILABLE));
        assert!(mask.contains(StatusMask::LIVELINESS_CHANGED));
        assert!(!mask.contains(StatusMask::SAMPLE_LOST));
    }

    #[test]
    fn status_count_default() {
        let sc = StatusCount::default();
        assert_eq!(sc.total_count, 0);
        assert_eq!(sc.total_count_change, 0);
    }

    #[test]
    fn inconsistent_topic_status_default() {
        let s = InconsistentTopicStatus::default();
        assert_eq!(s.total_count, 0);
        assert_eq!(s.total_count_change, 0);
    }

    #[test]
    fn sample_rejected_kind_default() {
        assert_eq!(
            SampleRejectedKind::default(),
            SampleRejectedKind::NotRejected
        );
    }

    #[test]
    fn publication_matched_default() {
        let s = PublicationMatchedStatus::default();
        assert_eq!(s.total_count, 0);
        assert_eq!(s.current_count, 0);
        assert!(s.last_subscription_handle.is_nil());
    }

    #[test]
    fn subscription_matched_default() {
        let s = SubscriptionMatchedStatus::default();
        assert_eq!(s.total_count, 0);
        assert_eq!(s.current_count, 0);
        assert!(s.last_publication_handle.is_nil());
    }

    #[test]
    fn liveliness_changed_default() {
        let s = LivelinessChangedStatus::default();
        assert_eq!(s.alive_count, 0);
        assert_eq!(s.not_alive_count, 0);
    }

    #[test]
    fn offered_deadline_missed_default() {
        let s = OfferedDeadlineMissedStatus::default();
        assert_eq!(s.total_count, 0);
        assert!(s.last_instance_handle.is_nil());
    }

    #[test]
    fn qos_policy_id_values() {
        assert_eq!(QosPolicyId::Invalid as i32, 0);
        assert_eq!(QosPolicyId::Reliability as i32, 11);
        assert_eq!(QosPolicyId::DurabilityService as i32, 22);
    }

    #[test]
    fn status_mask_individual_bits_distinct() {
        // Verify no two status masks overlap
        let masks = [
            StatusMask::INCONSISTENT_TOPIC,
            StatusMask::OFFERED_DEADLINE_MISSED,
            StatusMask::REQUESTED_DEADLINE_MISSED,
            StatusMask::OFFERED_INCOMPATIBLE_QOS,
            StatusMask::REQUESTED_INCOMPATIBLE_QOS,
            StatusMask::SAMPLE_LOST,
            StatusMask::SAMPLE_REJECTED,
            StatusMask::DATA_ON_READERS,
            StatusMask::DATA_AVAILABLE,
            StatusMask::LIVELINESS_LOST,
            StatusMask::LIVELINESS_CHANGED,
            StatusMask::PUBLICATION_MATCHED,
            StatusMask::SUBSCRIPTION_MATCHED,
        ];
        for (i, a) in masks.iter().enumerate() {
            for (j, b) in masks.iter().enumerate() {
                if i != j {
                    assert!(!a.contains(*b), "mask {i} should not contain mask {j}");
                }
            }
        }
    }
}
