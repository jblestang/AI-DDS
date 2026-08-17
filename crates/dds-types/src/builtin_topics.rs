//! # Builtin Topics — Discovery Data Types
//!
//! DDS defines a set of builtin topics whose data types carry information
//! about discovered participants, publications, and subscriptions.
//! Applications can subscribe to these topics to learn about the
//! current state of the DDS domain.
//!
//! Reference: DCPS §2.2.5 — Builtin Topics.

use crate::instance::Handle;
use crate::qos;

// ──────────────────────────────────────────────────────────────────────────────
// Builtin topic names (DCPS §2.2.5)
// ──────────────────────────────────────────────────────────────────────────────

/// Builtin topic name for participant discovery data.
pub const PARTICIPANT_TOPIC_NAME: &str = "DCPSParticipant";

/// Builtin topic name for publication (`DataWriter`) discovery data.
pub const PUBLICATION_TOPIC_NAME: &str = "DCPSPublication";

/// Builtin topic name for subscription (`DataReader`) discovery data.
pub const SUBSCRIPTION_TOPIC_NAME: &str = "DCPSSubscription";

/// Builtin topic name for topic discovery data.
pub const TOPIC_TOPIC_NAME: &str = "DCPSTopic";

// ──────────────────────────────────────────────────────────────────────────────
// ParticipantBuiltinTopicData (DCPS §2.2.5.4)
// ──────────────────────────────────────────────────────────────────────────────

/// Data type for the builtin participant discovery topic.
/// Contains the key (handle) and `QoS` of discovered participants.
///
/// Reference: DCPS §2.2.5.4.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParticipantBuiltinTopicData {
    /// Instance handle identifying this participant (derived from GUID).
    pub key: Handle,
    /// Application-specific data attached to the participant.
    pub user_data: qos::UserData,
}

// ──────────────────────────────────────────────────────────────────────────────
// TopicBuiltinTopicData (DCPS §2.2.5.2)
// ──────────────────────────────────────────────────────────────────────────────

/// Data type for the builtin topic discovery topic.
/// Contains the key, name, type name, and `QoS` of discovered topics.
///
/// Reference: DCPS §2.2.5.2.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TopicBuiltinTopicData {
    /// Deadline `QoS`.
    pub deadline: qos::Deadline,
    /// Destination order `QoS`.
    pub destination_order: qos::DestinationOrder,
    /// Topic `QoS` policies.
    pub durability: qos::Durability,
    /// History `QoS`.
    pub history: qos::History,
    /// Instance handle identifying this topic.
    pub key: Handle,
    /// Latency budget `QoS`.
    pub latency_budget: qos::LatencyBudget,
    /// Lifespan `QoS`.
    pub lifespan: qos::Lifespan,
    /// Liveliness `QoS`.
    pub liveliness: qos::Liveliness,
    /// Name of the topic.
    pub name: String,
    /// Ownership `QoS`.
    pub ownership: qos::Ownership,
    /// Reliability `QoS`.
    pub reliability: qos::Reliability,
    /// Resource limits `QoS`.
    pub resource_limits: qos::ResourceLimits,
    /// Topic metadata.
    pub topic_data: qos::TopicData,
    /// Transport priority `QoS`.
    pub transport_priority: qos::TransportPriority,
    /// Fully qualified type name.
    pub type_name: String,
}

// ──────────────────────────────────────────────────────────────────────────────
// PublicationBuiltinTopicData (DCPS §2.2.5.3)
// ──────────────────────────────────────────────────────────────────────────────

/// Data type for the builtin publication discovery topic.
/// Contains the key, topic info, and `QoS` of discovered `DataWriters`.
///
/// Reference: DCPS §2.2.5.3.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PublicationBuiltinTopicData {
    /// Deadline offered.
    pub deadline: qos::Deadline,
    /// Destination order.
    pub destination_order: qos::DestinationOrder,
    /// Durability offered by this writer.
    pub durability: qos::Durability,
    /// Group data of the parent publisher.
    pub group_data: qos::GroupData,
    /// Instance handle identifying this publication (writer GUID).
    pub key: Handle,
    /// Latency budget offered.
    pub latency_budget: qos::LatencyBudget,
    /// Lifespan offered.
    pub lifespan: qos::Lifespan,
    /// Liveliness offered.
    pub liveliness: qos::Liveliness,
    /// Ownership mode.
    pub ownership: qos::Ownership,
    /// Ownership strength.
    pub ownership_strength: qos::OwnershipStrength,
    /// Handle of the participant owning this writer.
    pub participant_key: Handle,
    /// Partition of the parent publisher.
    pub partition: qos::Partition,
    /// Presentation `QoS` of the parent publisher.
    pub presentation: qos::Presentation,
    /// Reliability offered.
    pub reliability: qos::Reliability,
    /// Topic metadata.
    pub topic_data: qos::TopicData,
    /// Name of the associated topic.
    pub topic_name: String,
    /// Fully qualified type name.
    pub type_name: String,
    /// Application-specific data.
    pub user_data: qos::UserData,
}

// ──────────────────────────────────────────────────────────────────────────────
// SubscriptionBuiltinTopicData (DCPS §2.2.5.4)
// ──────────────────────────────────────────────────────────────────────────────

/// Data type for the builtin subscription discovery topic.
/// Contains the key, topic info, and `QoS` of discovered `DataReaders`.
///
/// Reference: DCPS §2.2.5.4.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SubscriptionBuiltinTopicData {
    /// Deadline requested.
    pub deadline: qos::Deadline,
    /// Destination order.
    pub destination_order: qos::DestinationOrder,
    /// Durability requested by this reader.
    pub durability: qos::Durability,
    /// Group data of the parent subscriber.
    pub group_data: qos::GroupData,
    /// Instance handle identifying this subscription (reader GUID).
    pub key: Handle,
    /// Latency budget requested.
    pub latency_budget: qos::LatencyBudget,
    /// Liveliness requested.
    pub liveliness: qos::Liveliness,
    /// Ownership mode.
    pub ownership: qos::Ownership,
    /// Handle of the participant owning this reader.
    pub participant_key: Handle,
    /// Partition of the parent subscriber.
    pub partition: qos::Partition,
    /// Presentation `QoS` of the parent subscriber.
    pub presentation: qos::Presentation,
    /// Reliability requested.
    pub reliability: qos::Reliability,
    /// Time-based filter.
    pub time_based_filter: qos::TimeBasedFilter,
    /// Topic metadata.
    pub topic_data: qos::TopicData,
    /// Name of the associated topic.
    pub topic_name: String,
    /// Fully qualified type name.
    pub type_name: String,
    /// Application-specific data.
    pub user_data: qos::UserData,
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_topic_names() {
        assert_eq!(PARTICIPANT_TOPIC_NAME, "DCPSParticipant");
        assert_eq!(PUBLICATION_TOPIC_NAME, "DCPSPublication");
        assert_eq!(SUBSCRIPTION_TOPIC_NAME, "DCPSSubscription");
        assert_eq!(TOPIC_TOPIC_NAME, "DCPSTopic");
    }

    #[test]
    fn participant_builtin_topic_data_construction() {
        let data = ParticipantBuiltinTopicData {
            key: Handle::NIL,
            user_data: qos::UserData::default(),
        };
        assert!(data.key.is_nil());
        assert!(data.user_data.value.is_empty());
    }

    #[test]
    fn publication_builtin_topic_data_fields() {
        let data = PublicationBuiltinTopicData {
            key: Handle::NIL,
            participant_key: Handle::NIL,
            topic_name: "HelloWorldTopic".into(),
            type_name: "HelloWorld".into(),
            durability: qos::Durability::default(),
            deadline: qos::Deadline::default(),
            latency_budget: qos::LatencyBudget::default(),
            liveliness: qos::Liveliness::default(),
            reliability: qos::Reliability::default(),
            lifespan: qos::Lifespan::default(),
            user_data: qos::UserData::default(),
            ownership: qos::Ownership::default(),
            ownership_strength: qos::OwnershipStrength::default(),
            destination_order: qos::DestinationOrder::default(),
            presentation: qos::Presentation::default(),
            partition: qos::Partition::default(),
            topic_data: qos::TopicData::default(),
            group_data: qos::GroupData::default(),
        };
        assert_eq!(data.topic_name, "HelloWorldTopic");
        assert_eq!(data.type_name, "HelloWorld");
    }

    #[test]
    fn subscription_builtin_topic_data_fields() {
        let data = SubscriptionBuiltinTopicData {
            key: Handle::NIL,
            participant_key: Handle::NIL,
            topic_name: "SensorTopic".into(),
            type_name: "SensorData".into(),
            durability: qos::Durability::default(),
            deadline: qos::Deadline::default(),
            latency_budget: qos::LatencyBudget::default(),
            liveliness: qos::Liveliness::default(),
            reliability: qos::Reliability::default(),
            ownership: qos::Ownership::default(),
            destination_order: qos::DestinationOrder::default(),
            user_data: qos::UserData::default(),
            time_based_filter: qos::TimeBasedFilter::default(),
            presentation: qos::Presentation::default(),
            partition: qos::Partition::default(),
            topic_data: qos::TopicData::default(),
            group_data: qos::GroupData::default(),
        };
        assert_eq!(data.topic_name, "SensorTopic");
    }
}
