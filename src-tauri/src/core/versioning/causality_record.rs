use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity_id::EntityId;

/// Links a version to a cause (decision, event, or entity).
/// Enables answering "why did this change?" and "what did this affect?".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalityRecord {
    pub id: String,
    pub entity_id: EntityId,
    pub version_id: String,
    pub reason: String,
    pub affected_entities: Vec<String>,
    pub created_at: DateTime<Utc>,
}

impl CausalityRecord {
    pub fn new(
        entity_id: EntityId,
        version_id: String,
        reason: String,
        affected_entities: Vec<String>,
    ) -> Self {
        Self {
            id: crate::core::EntityId::new().as_str().to_string(),
            entity_id,
            version_id,
            reason,
            affected_entities,
            created_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_causality_record() {
        let eid = EntityId::new();
        let r = CausalityRecord::new(
            eid.clone(),
            "v1".to_string(),
            "user decided to update".to_string(),
            vec!["entity-2".to_string()],
        );
        assert!(!r.id.is_empty());
        assert_eq!(r.entity_id, eid);
        assert_eq!(r.version_id, "v1");
    }

    #[test]
    fn serialization_roundtrip() {
        let r = CausalityRecord::new(
            EntityId::new(),
            "v2".to_string(),
            "reason".to_string(),
            vec![],
        );
        let json = serde_json::to_string(&r).unwrap();
        let decoded: CausalityRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(r.id, decoded.id);
        assert_eq!(r.reason, decoded.reason);
    }
}
