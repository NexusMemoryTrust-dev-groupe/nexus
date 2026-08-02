use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::entity_id::EntityId;
use crate::core::graph::entity_types::EntityType;
use crate::core::result::{AppError, Result};

/// Status of an entity in the graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EntityStatus {
    Active,
    Archived,
    Merged,
}

/// A node in the knowledge graph — the universal world object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub entity_type: EntityType,
    pub title: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: EntityStatus,
    pub metadata: HashMap<String, serde_json::Value>,
    pub canonical_id: Option<String>,
}

impl Entity {
    /// Create a new entity with auto-generated ID and timestamps.
    pub fn new(entity_type: EntityType, title: String, description: String) -> Self {
        let now = Utc::now();
        Self {
            id: EntityId::new(),
            entity_type,
            title,
            description,
            created_at: now,
            updated_at: now,
            status: EntityStatus::Active,
            metadata: HashMap::new(),
            canonical_id: None,
        }
    }

    /// Validate entity invariants.
    pub fn validate(&self) -> Result<()> {
        if self.title.trim().is_empty() {
            return Err(AppError::Validation("Entity title cannot be empty".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entity() -> Entity {
        Entity::new(
            EntityType::Person,
            "Alice".to_string(),
            "Software engineer".to_string(),
        )
    }

    #[test]
    fn new_entity_defaults() {
        let e = sample_entity();
        assert!(!e.id.as_str().is_empty());
        assert_eq!(e.entity_type, EntityType::Person);
        assert_eq!(e.title, "Alice");
        assert_eq!(e.description, "Software engineer");
        assert_eq!(e.status, EntityStatus::Active);
        assert!(e.metadata.is_empty());
        assert!(e.canonical_id.is_none());
    }

    #[test]
    fn new_entity_has_valid_uuid() {
        let e = sample_entity();
        assert!(uuid::Uuid::parse_str(e.id.as_str()).is_ok());
    }

    #[test]
    fn new_entity_timestamps_are_close() {
        let e = sample_entity();
        let diff = (e.updated_at - e.created_at).num_milliseconds();
        assert!((0..1000).contains(&diff));
    }

    #[test]
    fn validate_empty_title_fails() {
        let mut e = sample_entity();
        e.title = "".to_string();
        assert!(e.validate().is_err());
    }

    #[test]
    fn validate_whitespace_title_fails() {
        let mut e = sample_entity();
        e.title = "   ".to_string();
        assert!(e.validate().is_err());
    }

    #[test]
    fn validate_valid_entity() {
        assert!(sample_entity().validate().is_ok());
    }

    #[test]
    fn entity_clone() {
        let e = sample_entity();
        let cloned = e.clone();
        assert_eq!(e.id, cloned.id);
        assert_eq!(e.title, cloned.title);
    }

    #[test]
    fn entity_serialization() {
        let e = sample_entity();
        let json = serde_json::to_string(&e).unwrap();
        let decoded: Entity = serde_json::from_str(&json).unwrap();
        assert_eq!(e.id, decoded.id);
        assert_eq!(e.title, decoded.title);
        assert_eq!(e.entity_type, decoded.entity_type);
    }
}
