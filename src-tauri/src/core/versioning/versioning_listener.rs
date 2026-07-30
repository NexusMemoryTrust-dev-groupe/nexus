use std::sync::Arc;

use crate::core::domain_event::{DomainEvent, DomainEventType};
use crate::core::entity_id::EntityId;
use crate::core::versioning::commit_service::{CommitService, CreateCommitParams};
use crate::core::versioning::automatic_commit::ChangeType;

/// Creates a boxed event handler that listens for MemoryRecordCreated events
/// and creates automatic commits via M28 CommitService.
///
/// This is the M2 → M28 integration bridge:
/// When a memory is saved (M2), this listener fires an automatic commit (M28).
pub fn create_versioning_handler(
    commit_service: Arc<dyn CommitService>,
) -> Box<dyn Fn(DomainEvent) + Send + Sync> {
    Box::new(move |event: DomainEvent| {
        if event.event_type == DomainEventType::MemoryRecordCreated {
            let cs = commit_service.clone();
            let event_id = event.id.clone();

            // Extract entity_id from payload
            let entity_id_str = event.payload.get("record_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if entity_id_str.is_empty() {
                tracing::warn!("MemoryRecordCreated event has no record_id in payload");
                return;
            }

            let entity_id = match EntityId::parse(entity_id_str) {
                Ok(id) => id,
                Err(e) => {
                    tracing::warn!("Failed to parse entity_id {}: {}", entity_id_str, e);
                    return;
                }
            };

            // Spawn async commit creation
            tokio::spawn(async move {
                let params = CreateCommitParams {
                    entity_type: "MemoryRecord".to_string(),
                    entity_id,
                    change_type: ChangeType::Created,
                    data: event.payload.clone(),
                    triggering_event_type: "MemoryRecordCreated".to_string(),
                    triggering_event_id: event_id,
                    diff: None,
                    linked_entities: None,
                    change_reason: Some("Auto-commit: memory record created".to_string()),
                };

                match cs.create_automatic_commit(params).await {
                    Ok(commit) => {
                        tracing::info!(
                            "Auto-commit created: v{} for {}",
                            commit.version_number,
                            commit.entity_id
                        );
                    }
                    Err(e) => {
                        tracing::error!("Failed to create auto-commit: {}", e);
                    }
                }
            });
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::domain_event::DomainEvent;
    use crate::core::versioning::commit_service::{CommitService, CreateCommitParams};
    use crate::core::versioning::automatic_commit::{AutomaticCommit, ChangeType};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockCommitService {
        call_count: std::sync::atomic::AtomicUsize,
    }

    impl MockCommitService {
        fn new() -> Self {
            Self {
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl CommitService for MockCommitService {
        async fn create_automatic_commit(
            &self,
            _params: CreateCommitParams,
        ) -> crate::core::Result<AutomaticCommit> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(AutomaticCommit {
                id: "commit-1".to_string(),
                hash: "abc123".to_string(),
                version_number: 1,
                entity_type: "MemoryRecord".to_string(),
                entity_id: EntityId::new(),
                change_type: ChangeType::Created,
                diff: None,
                baseline_snapshot_id: None,
                is_baseline: false,
                created_at: chrono::Utc::now(),
                created_by: "system".to_string(),
                triggering_event_type: "MemoryRecordCreated".to_string(),
                triggering_event_id: "evt-1".to_string(),
                change_reason: None,
                linked_entity_ids: vec![],
                linked_decision_ids: vec![],
                is_indexed: false,
                is_archived: false,
                size_bytes: 0,
            })
        }

        async fn get_commit(&self, _commit_id: &str) -> crate::core::Result<Option<AutomaticCommit>> {
            Ok(None)
        }

        async fn get_entity_history(
            &self,
            _entity_type: &str,
            _entity_id: &EntityId,
        ) -> crate::core::Result<Vec<AutomaticCommit>> {
            Ok(vec![])
        }

        async fn get_baseline(
            &self,
            _entity_type: &str,
            _entity_id: &EntityId,
        ) -> crate::core::Result<Option<AutomaticCommit>> {
            Ok(None)
        }
    }

    #[tokio::test]
    async fn handler_triggers_commit_on_memory_created() {
        let cs = Arc::new(MockCommitService::new());
        let handler = create_versioning_handler(cs.clone());

        let event = DomainEvent::new(
            DomainEventType::MemoryRecordCreated,
            serde_json::json!({"record_id": EntityId::new().to_string()}),
        );

        handler(event);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(cs.call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn handler_ignores_other_event_types() {
        let cs = Arc::new(MockCommitService::new());
        let handler = create_versioning_handler(cs.clone());

        let event = DomainEvent::new(
            DomainEventType::EntityCreated,
            serde_json::json!({"record_id": EntityId::new().to_string()}),
        );

        handler(event);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(cs.call_count.load(Ordering::SeqCst), 0);
    }
}
