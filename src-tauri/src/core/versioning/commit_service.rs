use async_trait::async_trait;

use crate::core::entity_id::EntityId;
use crate::core::result::Result;
use crate::core::versioning::automatic_commit::{AutomaticCommit, ChangeType};

/// Parameters for creating a new automatic commit.
#[derive(Debug, Clone)]
pub struct CreateCommitParams {
    pub entity_type: String,
    pub entity_id: EntityId,
    pub change_type: ChangeType,
    pub data: serde_json::Value,
    pub triggering_event_type: String,
    pub triggering_event_id: String,
    pub diff: Option<String>,
    pub linked_entities: Option<Vec<String>>,
    pub change_reason: Option<String>,
}

/// Service for creating and querying automatic commits.
#[async_trait]
pub trait CommitService: Send + Sync {
    /// Create a new automatic commit from the given params.
    async fn create_automatic_commit(
        &self,
        params: CreateCommitParams,
    ) -> Result<AutomaticCommit>;

    /// Get a single commit by its ID.
    async fn get_commit(&self, commit_id: &str) -> Result<Option<AutomaticCommit>>;

    /// Get the full version history for an entity, ordered by version number.
    async fn get_entity_history(
        &self,
        entity_type: &str,
        entity_id: &EntityId,
    ) -> Result<Vec<AutomaticCommit>>;

    /// Get the latest baseline commit for an entity.
    async fn get_baseline(
        &self,
        entity_type: &str,
        entity_id: &EntityId,
    ) -> Result<Option<AutomaticCommit>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_commit_params_clone() {
        let params = CreateCommitParams {
            entity_type: "MemoryRecord".to_string(),
            entity_id: EntityId::new(),
            change_type: ChangeType::Created,
            data: serde_json::json!({"title": "test"}),
            triggering_event_type: "EntityCreated".to_string(),
            triggering_event_id: "evt-1".to_string(),
            diff: None,
            linked_entities: None,
            change_reason: Some("user created memory".to_string()),
        };
        let cloned = params.clone();
        assert_eq!(params.entity_type, cloned.entity_type);
        assert_eq!(params.change_type, cloned.change_type);
    }
}
