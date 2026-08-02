use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::entity_id::EntityId;

/// Types of domain events that can occur in the system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DomainEventType {
    EntityCreated,
    EntityUpdated,
    EntityDeleted,
    RelationshipCreated,
    RelationshipDeleted,
    MemoryRecordCreated,
    MemoryRecordUpdated,
    ExecutionCompleted,
    DecisionMade,
}

/// A domain event representing something that happened in the system.
/// Immutable after creation, carries payload and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEvent {
    pub id: String,
    pub event_type: DomainEventType,
    pub timestamp: DateTime<Utc>,
    pub payload: serde_json::Value,
    pub metadata: HashMap<String, String>,
}

impl DomainEvent {
    /// Create a new domain event with auto-generated ID and timestamp.
    pub fn new(event_type: DomainEventType, payload: serde_json::Value) -> Self {
        Self {
            id: EntityId::new().to_string(),
            event_type,
            timestamp: Utc::now(),
            payload,
            metadata: HashMap::new(),
        }
    }

    /// Create a domain event with custom metadata.
    pub fn with_metadata(
        event_type: DomainEventType,
        payload: serde_json::Value,
        metadata: HashMap<String, String>,
    ) -> Self {
        Self {
            id: EntityId::new().to_string(),
            event_type,
            timestamp: Utc::now(),
            payload,
            metadata,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_event_new() {
        let event = DomainEvent::new(
            DomainEventType::EntityCreated,
            serde_json::json!({"name": "test"}),
        );

        assert!(!event.id.is_empty());
        assert_eq!(event.event_type, DomainEventType::EntityCreated);
        assert_eq!(event.payload["name"], "test");
        assert!(event.metadata.is_empty());
    }

    #[test]
    fn domain_event_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), "test".to_string());

        let event = DomainEvent::with_metadata(
            DomainEventType::EntityUpdated,
            serde_json::json!({}),
            metadata,
        );

        assert_eq!(event.metadata.get("source").unwrap(), "test");
    }

    #[test]
    fn domain_event_serialization() {
        let event = DomainEvent::new(
            DomainEventType::EntityCreated,
            serde_json::json!({"key": "value"}),
        );

        let json = serde_json::to_string(&event).unwrap();
        let decoded: DomainEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(event.id, decoded.id);
        assert_eq!(event.event_type, decoded.event_type);
        assert_eq!(event.payload, decoded.payload);
    }

    #[test]
    fn domain_event_clone() {
        let event = DomainEvent::new(DomainEventType::EntityCreated, serde_json::json!({}));
        let cloned = event.clone();
        assert_eq!(event.id, cloned.id);
        assert_eq!(event.event_type, cloned.event_type);
    }

    #[test]
    fn domain_event_timestamp_is_recent() {
        let event = DomainEvent::new(DomainEventType::EntityCreated, serde_json::json!({}));
        let now = Utc::now();
        let diff = (now - event.timestamp).num_milliseconds();
        assert!(diff < 1000, "Timestamp should be recent");
    }
}
