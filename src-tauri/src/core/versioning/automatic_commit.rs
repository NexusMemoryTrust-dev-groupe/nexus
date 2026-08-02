use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity_id::EntityId;
use crate::core::result::{AppError, Result};

/// Type of change captured by a version.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChangeType {
    Created,
    Modified,
    Deleted,
}

/// An automatic commit — an immutable record of a significant change.
/// Captures what changed, why, and links to causality chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomaticCommit {
    pub id: String,
    pub hash: String,
    pub version_number: u32,
    pub entity_type: String,
    pub entity_id: EntityId,
    pub change_type: ChangeType,
    pub diff: Option<String>,
    pub baseline_snapshot_id: Option<String>,
    pub is_baseline: bool,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub triggering_event_type: String,
    pub triggering_event_id: String,
    pub change_reason: Option<String>,
    pub linked_entity_ids: Vec<String>,
    pub linked_decision_ids: Vec<String>,
    pub is_indexed: bool,
    pub is_archived: bool,
    pub size_bytes: u64,
}

impl AutomaticCommit {
    /// Validate commit invariants.
    pub fn validate(&self) -> Result<()> {
        if self.entity_type.is_empty() {
            return Err(AppError::Validation(
                "entity_type cannot be empty".to_string(),
            ));
        }
        if self.version_number == 0 {
            return Err(AppError::Validation(
                "version_number must be >= 1".to_string(),
            ));
        }
        if self.hash.is_empty() {
            return Err(AppError::Validation("hash cannot be empty".to_string()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_commit() -> AutomaticCommit {
        AutomaticCommit {
            id: "commit-1".to_string(),
            hash: "abc123def456".to_string(),
            version_number: 1,
            entity_type: "MemoryRecord".to_string(),
            entity_id: EntityId::new(),
            change_type: ChangeType::Created,
            diff: None,
            baseline_snapshot_id: None,
            is_baseline: false,
            created_at: Utc::now(),
            created_by: "system".to_string(),
            triggering_event_type: "EntityCreated".to_string(),
            triggering_event_id: "evt-1".to_string(),
            change_reason: None,
            linked_entity_ids: vec![],
            linked_decision_ids: vec![],
            is_indexed: false,
            is_archived: false,
            size_bytes: 0,
        }
    }

    #[test]
    fn valid_commit() {
        assert!(sample_commit().validate().is_ok());
    }

    #[test]
    fn validate_empty_entity_type() {
        let mut c = sample_commit();
        c.entity_type = "".to_string();
        assert!(c.validate().is_err());
    }

    #[test]
    fn validate_zero_version() {
        let mut c = sample_commit();
        c.version_number = 0;
        assert!(c.validate().is_err());
    }

    #[test]
    fn validate_empty_hash() {
        let mut c = sample_commit();
        c.hash = "".to_string();
        assert!(c.validate().is_err());
    }

    #[test]
    fn serialization_roundtrip() {
        let c = sample_commit();
        let json = serde_json::to_string(&c).unwrap();
        let decoded: AutomaticCommit = serde_json::from_str(&json).unwrap();
        assert_eq!(c.id, decoded.id);
        assert_eq!(c.hash, decoded.hash);
        assert_eq!(c.change_type, decoded.change_type);
    }

    #[test]
    fn change_type_serialization() {
        for ct in [
            ChangeType::Created,
            ChangeType::Modified,
            ChangeType::Deleted,
        ] {
            let json = serde_json::to_string(&ct).unwrap();
            let decoded: ChangeType = serde_json::from_str(&json).unwrap();
            assert_eq!(ct, decoded);
        }
    }
}
