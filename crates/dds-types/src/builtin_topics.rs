//! # Builtin Topics — Discovery Data Types
//!
//! DDS defines a set of builtin topics whose data types carry information
//! about discovered participants, publications, and subscriptions.
//! Applications can subscribe to these topics to learn about the
//! current state of the DDS domain.
//!
//! Reference: DCPS §2.2.5 — Builtin Topics

use crate::instance::InstanceHandle;
use crate::qos;

// ──────────────────────────────────────────────────────────────────────────────
// Builtin topic names (DCPS §2.2.5)
// ──────────────────────────────────────────────────────────────────────────────

/// Builtin topic name for participant discovery data.
pub const PARTICIPANT_TOPIC_NAME: &str = "DCPSParticipant";

/// Builtin topic name for publication (DataWriter) discovery data.
pub const PUBLICATION_TOPIC_NAME: &str = "DCPSPublication";

/// Builtin topic name for subscription (DataReader) discovery data.
pub const SUBSCRIPTION_TOPIC_NAME: &str = "DCPSSubscription";

/// Builtin topic name for topic discovery data.
pub const TOPIC_TOPIC_NAME: &str = "DCPSTopic";

// ──────────────────────────────────────────────────────────────────────────────
// ParticipantBuiltinTopicData (DCPS §2.2.5.4)
// ──────────────────────────────────────────────────────────────────────────────

/// Data type for the builtin participant discovery topic.
/// Contains the key (handle) and QoS of discovered participants.
///
/// Reference: DCPS §2.2.5.4
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantBuiltinTopicData {
    /// Instance handle identifying this participant (derived from GUID).
    pub key: InstanceHandle,
    /// Application-specific data attached to the participant.
    pub user_data: qos::UserData,
}

// ──────────────────────────────────────────────────────────────────────────────
// TopicBuiltinTopicData (DCPS §2.2.5.2)
// ──────────────────────────────────────────────────────────────────────────────

/// Data type for the builtin topic discovery topic.
/// Contains the key, name, type name, and QoS of discovered topics.
///
/// Reference: DCPS §2.2.5.2
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicBuiltinTopicData {
    /// Instance handle identifying this topic.
    pub key: InstanceHandle,
    /// Name of the topic.
    pub name: String,
    /// Fully qualified type name.
    pub type_name: String,
    /// Topic QoS policies.
    pub durability: qos::Durability,
    /// Deadline QoS.
    pub deadline: qos::Deadline,
    /// Latency budget QoS.
    pub latency_budget: qos::LatencyBudget,
    /// Liveliness QoS.
    pub liveliness: qos::Liveliness,
    /// Reliability QoS.
    pub reliability: qos::Reliability,
    /// Transport priority QoS.
    pub transport_priority: qos::TransportPriority,
    /// Lifespan QoS.
    pub lifespan: qos::Lifespan,
    /// Destination order QoS.
    pub destination_order: qos::DestinationOrder,
    /// History QoS.
    pub history: qos::History,
    /// Resource limits QoS.
    pub resource_limits: qos::ResourceLimits,
    /// Ownership QoS.
    pub ownership: qos::Ownership,
    /// Topic metadata.
    pub topic_data: qos::TopicData,
}

// ──────────────────────────────────────────────────────────────────────────────
// PublicationBuiltinTopicData (DCPS §2.2.5.3)
// ──────────────────────────────────────────────────────────────────────────────

/// Data type for the builtin publication discovery topic.
/// Contains the key, topic info, and QoS of discovered DataWriters.
///
/// Reference: DCPS §2.2.5.3
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationBuiltinTopicData {
    /// Instance handle identifying this publication (writer GUID).
    pub key: InstanceHandle,
    /// Handle of the participant owning this writer.
    pub participant_key: InstanceHandle,
    /// Name of the associated topic.
    pub topic_name: String,
    /// Fully qualified type name.
    pub type_name: String,
    // ── QoS policies ──
    /// Durability offered by this writer.
    pub durability: qos::Durability,
    /// Deadline offered.
    pub deadline: qos::Deadline,
    /// Latency budget offered.
    pub latency_budget: qos::LatencyBudget,
    /// Liveliness offered.
    pub liveliness: qos::Liveliness,
    /// Reliability offered.
    pub reliability: qos::Reliability,
    /// Lifespan offered.
    pub lifespan: qos::Lifespan,
    /// Application-specific data.
    pub user_data: qos::UserData,
    /// Ownership mode.
    pub ownership: qos::Ownership,
    /// Ownership strength.
    pub ownership_strength: qos::OwnershipStrength,
    /// Destination order.
    pub destination_order: qos::DestinationOrder,
    /// Presentation QoS of the parent publisher.
    pub presentation: qos::Presentation,
    /// Partition of the parent publisher.
    pub partition: qos::Partition,
    /// Topic metadata.
    pub topic_data: qos::TopicData,
    /// Group data of the parent publisher.
    pub group_data: qos::GroupData,
}

// ──────────────────────────────────────────────────────────────────────────────
// SubscriptionBuiltinTopicData (DCPS §2.2.5.4)
// ──────────────────────────────────────────────────────────────────────────────

/// Data type for the builtin subscription discovery topic.
/// Contains the key, topic info, and QoS of discovered DataReaders.
///
/// Reference: DCPS §2.2.5.4
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionBuiltinTopicData {
    /// Instance handle identifying this subscription (reader GUID).
    pub key: InstanceHandle,
    /// Handle of the participant owning this reader.
    pub participant_key: InstanceHandle,
    /// Name of the associated topic.
    pub topic_name: String,
    /// Fully qualified type name.
    pub type_name: String,
    // ── QoS policies ──
    /// Durability requested by this reader.
    pub durability: qos::Durability,
    /// Deadline requested.
    pub deadline: qos::Deadline,
    /// Latency budget requested.
    pub latency_budget: qos::LatencyBudget,
    /// Liveliness requested.
    pub liveliness: qos::Liveliness,
    /// Reliability requested.
    pub reliability: qos::Reliability,
    /// Ownership mode.
    pub ownership: qos::Ownership,
    /// Destination order.
    pub destination_order: qos::DestinationOrder,
    /// Application-specific data.
    pub user_data: qos::UserData,
    /// Time-based filter.
    pub time_based_filter: qos::TimeBasedFilter,
    /// Presentation QoS of the parent subscriber.
    pub presentation: qos::Presentation,
    /// Partition of the parent subscriber.
    pub partition: qos::Partition,
    /// Topic metadata.
    pub topic_data: qos::TopicData,
    /// Group data of the parent subscriber.
    pub group_data: qos::GroupData,
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
            key: InstanceHandle::NIL,
            user_data: qos::UserData::default(),
        };
        assert!(data.key.is_nil());
        assert!(data.user_data.value.is_empty());
    }

    #[test]
    fn publication_builtin_topic_data_fields() {
        let data = PublicationBuiltinTopicData {
            key: InstanceHandle::NIL,
            participant_key: InstanceHandle::NIL,
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
            key: InstanceHandle::NIL,
            participant_key: InstanceHandle::NIL,
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
