use async_trait::async_trait;

use crate::core::entity_id::EntityId;
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::result::Result;

/// Context for a memory recall operation.
#[derive(Debug, Clone)]
pub struct RecallContext {
    /// Filter by project space.
    pub project_id: Option<EntityId>,
    /// Maximum number of results to return.
    pub max_results: u32,
    /// Minimum confidence threshold (0.0–1.0).
    pub min_confidence: f64,
}

impl Default for RecallContext {
    fn default() -> Self {
        Self {
            project_id: None,
            max_results: 20,
            min_confidence: 0.0,
        }
    }
}

/// Result of a memory recall operation.
#[derive(Debug, Clone)]
pub struct RecallResult {
    /// The matching memory records, ordered by relevance.
    pub records: Vec<MemoryRecord>,
    /// Aggregate relevance score for the entire result set.
    pub score: f64,
}

/// Service trait for memory recall — not just search, but context reconstruction.
/// Recovers connected memories and builds coherent context from scattered facts.
#[async_trait]
pub trait MemoryRecallService: Send + Sync {
    /// Recall memories matching a natural language query within the given context.
    async fn recall(&self, query: &str, context: &RecallContext) -> Result<RecallResult>;

    /// Recall all memories linked to a specific entity.
    async fn recall_by_entity(&self, entity_id: &EntityId) -> Result<RecallResult>;

    /// Recall the N most recent active memories.
    async fn recall_recent(&self, limit: u32) -> Result<Vec<MemoryRecord>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recall_context_default() {
        let ctx = RecallContext::default();
        assert!(ctx.project_id.is_none());
        assert_eq!(ctx.max_results, 20);
        assert_eq!(ctx.min_confidence, 0.0);
    }

    #[test]
    fn recall_result_clone() {
        let result = RecallResult {
            records: vec![],
            score: 0.85,
        };
        let cloned = result.clone();
        assert_eq!(cloned.score, 0.85);
        assert!(cloned.records.is_empty());
    }
}
