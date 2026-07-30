use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::entity_id::EntityId;
use crate::core::graph::relationship_types::RelationshipType;
use crate::core::result::{AppError, Result};

/// A directed edge between two entities in the knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub id: EntityId,
    pub source_entity_id: EntityId,
    pub target_entity_id: EntityId,
    pub relationship_type: RelationshipType,
    pub weight: f64,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Relationship {
    /// Create a new relationship. Validates weight range and source != target.
    pub fn new(
        source: EntityId,
        target: EntityId,
        rel_type: RelationshipType,
        weight: f64,
    ) -> Result<Self> {
        if !(0.0..=1.0).contains(&weight) {
            return Err(AppError::Validation(
                "Weight must be between 0.0 and 1.0".into(),
            ));
        }
        if source == target {
            return Err(AppError::Validation(
                "Source and target must be different entities".into(),
            ));
        }
        Ok(Self {
            id: EntityId::new(),
            source_entity_id: source,
            target_entity_id: target,
            relationship_type: rel_type,
            weight,
            created_at: Utc::now(),
            created_by: String::new(),
            metadata: HashMap::new(),
        })
    }

    /// Validate relationship invariants.
    pub fn validate(&self) -> Result<()> {
        if self.weight < 0.0 || self.weight > 1.0 {
            return Err(AppError::Validation(
                "Weight must be between 0.0 and 1.0".into(),
            ));
        }
        if self.source_entity_id == self.target_entity_id {
            return Err(AppError::Validation(
                "Source and target must be different entities".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_relationship() -> Relationship {
        Relationship::new(
            EntityId::new(),
            EntityId::new(),
            RelationshipType::Created,
            1.0,
        )
        .unwrap()
    }

    #[test]
    fn new_relationship_defaults() {
        let r = sample_relationship();
        assert!(!r.id.as_str().is_empty());
        assert_eq!(r.relationship_type, RelationshipType::Created);
        assert_eq!(r.weight, 1.0);
        assert!(r.created_by.is_empty());
        assert!(r.metadata.is_empty());
    }

    #[test]
    fn new_relationship_has_valid_uuid() {
        let r = sample_relationship();
        assert!(uuid::Uuid::parse_str(r.id.as_str()).is_ok());
    }

    #[test]
    fn new_relationship_timestamp_is_recent() {
        let r = sample_relationship();
        let diff = (Utc::now() - r.created_at).num_milliseconds();
        assert!(diff >= 0 && diff < 1000);
    }

    #[test]
    fn weight_zero_valid() {
        let r = Relationship::new(
            EntityId::new(),
            EntityId::new(),
            RelationshipType::RelatedTo,
            0.0,
        );
        assert!(r.is_ok());
    }

    #[test]
    fn weight_one_valid() {
        let r = Relationship::new(
            EntityId::new(),
            EntityId::new(),
            RelationshipType::RelatedTo,
            1.0,
        );
        assert!(r.is_ok());
    }

    #[test]
    fn weight_negative_fails() {
        let r = Relationship::new(
            EntityId::new(),
            EntityId::new(),
            RelationshipType::RelatedTo,
            -0.1,
        );
        assert!(r.is_err());
    }

    #[test]
    fn weight_above_one_fails() {
        let r = Relationship::new(
            EntityId::new(),
            EntityId::new(),
            RelationshipType::RelatedTo,
            1.1,
        );
        assert!(r.is_err());
    }

    #[test]
    fn same_source_target_fails() {
        let id = EntityId::new();
        let r = Relationship::new(
            id.clone(),
            id,
            RelationshipType::RelatedTo,
            0.5,
        );
        assert!(r.is_err());
    }

    #[test]
    fn validate_invalid_weight() {
        let mut r = sample_relationship();
        r.weight = 2.0;
        assert!(r.validate().is_err());
    }

    #[test]
    fn validate_same_source_target() {
        let id = EntityId::new();
        let mut r = sample_relationship();
        r.source_entity_id = id.clone();
        r.target_entity_id = id;
        assert!(r.validate().is_err());
    }

    #[test]
    fn relationship_clone() {
        let r = sample_relationship();
        let cloned = r.clone();
        assert_eq!(r.id, cloned.id);
        assert_eq!(r.weight, cloned.weight);
    }

    #[test]
    fn relationship_serialization() {
        let r = sample_relationship();
        let json = serde_json::to_string(&r).unwrap();
        let decoded: Relationship = serde_json::from_str(&json).unwrap();
        assert_eq!(r.id, decoded.id);
        assert_eq!(r.weight, decoded.weight);
        assert_eq!(r.relationship_type, decoded.relationship_type);
    }
}
