//! # Status — Communication Status Types for DDS Entities
//!
//! Each DDS entity maintains a set of status flags that the application
//! can monitor via listeners or wait sets. These types represent the
//! data carried in each status change notification.
//!
//! Reference: DCPS §2.2.4 — Communication Status.

use crate::instance::Handle;

// ──────────────────────────────────────────────────────────────────────────────
// QoS Policy identification (for status reporting)
// ──────────────────────────────────────────────────────────────────────────────

use crate::policy_id::Id;

// ──────────────────────────────────────────────────────────────────────────────
// Status types (DCPS §2.2.4)
// ──────────────────────────────────────────────────────────────────────────────

/// A count with a delta since last read, used in most status types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub struct Count {
    /// Cumulative count since entity creation.
    pub total_count: i32,
    /// Change since the last time the status was read.
    pub total_count_change: i32,
}

/// Inconsistent topic status — triggered when a topic with the same name
/// but incompatible type is discovered.
///
/// Applies to: Topic.
/// Reference: DCPS §2.2.4.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct InconsistentTopic {
    /// Count of inconsistent discoveries.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
}

/// Offered deadline missed — the `DataWriter` failed to write within
/// the deadline period for an instance.
///
/// Applies to: `DataWriter`.
/// Reference: DCPS §2.2.4.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct OfferedDeadlineMissed {
    /// Handle of the last instance that missed its deadline.
    pub last_instance_handle: Handle,
    /// Cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
}

/// Requested deadline missed — the `DataReader` did not receive data
/// within the deadline period for an instance.
///
/// Applies to: `DataReader`.
/// Reference: DCPS §2.2.4.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct RequestedDeadlineMissed {
    /// Handle of the last instance that missed its deadline.
    pub last_instance_handle: Handle,
    /// Cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
}

/// Offered incompatible `QoS` — a `DataWriter`'s offered `QoS` is incompatible
/// with a `DataReader`'s requested `QoS`.
///
/// Applies to: `DataWriter`.
/// Reference: DCPS §2.2.4.4.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct OfferedIncompatibleQos {
    /// The ID of the last `QoS` policy that caused incompatibility.
    pub last_policy_id: Option<Id>,
    /// Per-policy counts of incompatibilities.
    pub policies: Vec<QosPolicyCount>,
    /// Cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
}

/// Requested incompatible `QoS` — a `DataReader`'s requested `QoS` is
/// incompatible with a `DataWriter`'s offered `QoS`.
///
/// Applies to: `DataReader`.
/// Reference: DCPS §2.2.4.5.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct RequestedIncompatibleQos {
    /// The ID of the last `QoS` policy that caused incompatibility.
    pub last_policy_id: Option<Id>,
    /// Per-policy counts of incompatibilities.
    pub policies: Vec<QosPolicyCount>,
    /// Cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
}

/// Per-policy incompatibility counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct QosPolicyCount {
    /// How many times this policy caused an incompatibility.
    pub count: i32,
    /// Which policy was incompatible.
    pub policy_id: Id,
}

/// Sample lost status — samples were lost (never delivered to the reader).
///
/// Applies to: `DataReader`.
/// Reference: DCPS §2.2.4.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct SampleLost {
    /// Cumulative count of lost samples.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
}

/// Sample rejected status — samples were rejected (e.g., due to
/// resource limits).
///
/// Applies to: `DataReader`.
/// Reference: DCPS §2.2.4.7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct SampleRejected {
    /// Handle of the instance whose sample was rejected.
    pub last_instance_handle: Handle,
    /// Reason for the last rejection.
    pub last_reason: SampleRejectedKind,
    /// Cumulative count of rejected samples.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
}

/// Reason why a sample was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Default)]
#[non_exhaustive]
pub enum SampleRejectedKind {
    /// Not rejected.
    #[default]
    NotRejected,
    /// Rejected because `max_instances` limit was reached.
    RejectedByInstancesLimit,
    /// Rejected because `max_samples` limit was reached.
    RejectedBySamplesLimit,
    /// Rejected because `max_samples_per_instance` limit was reached.
    RejectedBySamplesPerInstanceLimit,
}


/// Liveliness changed — the liveliness of one or more `DataWriters`
/// matching this `DataReader` has changed.
///
/// Applies to: `DataReader`.
/// Reference: DCPS §2.2.4.8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct LivelinessChanged {
    /// Number of currently alive writers.
    pub alive_count: i32,
    /// Change in `alive_count` since last read.
    pub alive_count_change: i32,
    /// Handle of the last writer whose liveliness changed.
    pub last_publication_handle: Handle,
    /// Number of currently not-alive writers.
    pub not_alive_count: i32,
    /// Change in `not_alive_count` since last read.
    pub not_alive_count_change: i32,
}

/// Liveliness lost — the `DataWriter` failed to assert its liveliness
/// within the lease duration.
///
/// Applies to: `DataWriter`.
/// Reference: DCPS §2.2.4.9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct LivelinessLost {
    /// Cumulative count.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
}

/// Publication matched — a new `DataReader` matched (or unmatched) this
/// `DataWriter`.
///
/// Applies to: `DataWriter`.
/// Reference: DCPS §2.2.4.10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct PublicationMatched {
    /// Current number of matched readers.
    pub current_count: i32,
    /// Change in `current_count` since last read.
    pub current_count_change: i32,
    /// Handle of the last reader that matched or unmatched.
    pub last_subscription_handle: Handle,
    /// Cumulative count of matches.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
}

/// Subscription matched — a new `DataWriter` matched (or unmatched) this
/// `DataReader`.
///
/// Applies to: `DataReader`.
/// Reference: DCPS §2.2.4.11.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct SubscriptionMatched {
    /// Current number of matched writers.
    pub current_count: i32,
    /// Change in `current_count` since last read.
    pub current_count_change: i32,
    /// Handle of the last writer that matched or unmatched.
    pub last_publication_handle: Handle,
    /// Cumulative count of matches.
    pub total_count: i32,
    /// Change since last read.
    pub total_count_change: i32,
}

// ──────────────────────────────────────────────────────────────────────────────
// Status mask — bitflags for selecting which statuses to monitor
// ──────────────────────────────────────────────────────────────────────────────

/// Bitmask identifying which communication statuses to monitor.
/// Used with `WaitSets` and `StatusConditions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Mask(pub u32);

impl Mask {
    /// All statuses selected.
    pub const ALL: Self = Self(u32::MAX);

    // Individual status bits
    /// Data available.
    pub const DATA_AVAILABLE: Self = Self(1 << 10);
    /// Data on readers (data available).
    pub const DATA_ON_READERS: Self = Self(1 << 9);
    /// Inconsistent topic.
    pub const INCONSISTENT_TOPIC: Self = Self(1 << 0);
    /// Liveliness changed (reader).
    pub const LIVELINESS_CHANGED: Self = Self(1 << 12);
    /// Liveliness lost (writer).
    pub const LIVELINESS_LOST: Self = Self(1 << 11);
    /// No statuses selected.
    pub const NONE: Self = Self(0);
    /// Offered deadline missed.
    pub const OFFERED_DEADLINE_MISSED: Self = Self(1 << 1);
    /// Offered incompatible `QoS`.
    pub const OFFERED_INCOMPATIBLE_QOS: Self = Self(1 << 5);
    /// Publication matched.
    pub const PUBLICATION_MATCHED: Self = Self(1 << 13);
    /// Requested deadline missed.
    pub const REQUESTED_DEADLINE_MISSED: Self = Self(1 << 2);
    /// Requested incompatible `QoS`.
    pub const REQUESTED_INCOMPATIBLE_QOS: Self = Self(1 << 6);
    /// Sample lost.
    pub const SAMPLE_LOST: Self = Self(1 << 7);
    /// Sample rejected.
    pub const SAMPLE_REJECTED: Self = Self(1 << 8);
    /// Subscription matched.
    pub const SUBSCRIPTION_MATCHED: Self = Self(1 << 14);

    /// Check if a specific status bit is set.
    #[must_use]
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        return (self.0 & other.0) == other.0
    }

    /// Combine two masks (bitwise OR).
    #[must_use]
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        return Self(self.0 | other.0)
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
        assert!(!Mask::NONE.contains(Mask::DATA_AVAILABLE));
    }

    #[test]
    fn status_mask_all_contains_everything() {
        assert!(Mask::ALL.contains(Mask::DATA_AVAILABLE));
        assert!(Mask::ALL.contains(Mask::LIVELINESS_CHANGED));
        assert!(Mask::ALL.contains(Mask::PUBLICATION_MATCHED));
    }

    #[test]
    fn status_mask_union() {
        let mask = Mask::DATA_AVAILABLE.union(Mask::LIVELINESS_CHANGED);
        assert!(mask.contains(Mask::DATA_AVAILABLE));
        assert!(mask.contains(Mask::LIVELINESS_CHANGED));
        assert!(!mask.contains(Mask::SAMPLE_LOST));
    }

    #[test]
    fn status_count_default() {
        let sc = Count::default();
        assert_eq!(sc.total_count, 0);
        assert_eq!(sc.total_count_change, 0);
    }

    #[test]
    fn inconsistent_topic_status_default() {
        let s = InconsistentTopic::default();
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
        let s = PublicationMatched::default();
        assert_eq!(s.total_count, 0);
        assert_eq!(s.current_count, 0);
        assert!(s.last_subscription_handle.is_nil());
    }

    #[test]
    fn subscription_matched_default() {
        let s = SubscriptionMatched::default();
        assert_eq!(s.total_count, 0);
        assert_eq!(s.current_count, 0);
        assert!(s.last_publication_handle.is_nil());
    }

    #[test]
    fn liveliness_changed_default() {
        let s = LivelinessChanged::default();
        assert_eq!(s.alive_count, 0);
        assert_eq!(s.not_alive_count, 0);
    }

    #[test]
    fn offered_deadline_missed_default() {
        let s = OfferedDeadlineMissed::default();
        assert_eq!(s.total_count, 0);
        assert!(s.last_instance_handle.is_nil());
    }

    #[test]
    fn qos_policy_id_values() {
        assert_eq!(Id::Invalid as i32, 0);
        assert_eq!(Id::Reliability as i32, 11);
        assert_eq!(Id::DurabilityService as i32, 22);
    }

    #[test]
    fn status_mask_individual_bits_distinct() {
        // Verify no two status masks overlap
        let masks = [
            Mask::INCONSISTENT_TOPIC,
            Mask::OFFERED_DEADLINE_MISSED,
            Mask::REQUESTED_DEADLINE_MISSED,
            Mask::OFFERED_INCOMPATIBLE_QOS,
            Mask::REQUESTED_INCOMPATIBLE_QOS,
            Mask::SAMPLE_LOST,
            Mask::SAMPLE_REJECTED,
            Mask::DATA_ON_READERS,
            Mask::DATA_AVAILABLE,
            Mask::LIVELINESS_LOST,
            Mask::LIVELINESS_CHANGED,
            Mask::PUBLICATION_MATCHED,
            Mask::SUBSCRIPTION_MATCHED,
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
