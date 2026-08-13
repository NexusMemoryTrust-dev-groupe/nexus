use std::sync::Arc;

use chrono::Utc;

use crate::core::entity_id::EntityId;
use crate::core::memory::layer::LayerClassifier;
use crate::core::memory::memory_compression::MemoryCompressionService;
use crate::core::memory::memory_recall::{MemoryRecallService, RecallContext, RecallResult};
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::memory::types::{
    LayerAssignment, LayerHistoryEntry, MemoryLayer, MemoryStatus, MemoryVisibility,
};
use crate::core::result::{AppError, Result};
use crate::core::security::RequestContext;

/// Business-logic service for memory operations.
/// Orchestrates repository, recall, and compression services.
pub struct MemoryService {
    repository: Arc<dyn MemoryRepository>,
    recall_service: Arc<dyn MemoryRecallService>,
    compression_service: Arc<dyn MemoryCompressionService>,
}

impl MemoryService {
    pub fn new(
        repository: Arc<dyn MemoryRepository>,
        recall_service: Arc<dyn MemoryRecallService>,
        compression_service: Arc<dyn MemoryCompressionService>,
    ) -> Self {
        Self {
            repository,
            recall_service,
            compression_service,
        }
    }

    /// Create a new memory record with validation and audit context.
    ///
    /// Auto-classification: if the record carries no explicit layer history
    /// (fresh capture), the signature classifier assigns the layer and records
    /// provenance (confidence, reason, history entry tagged `classifier`).
    pub async fn create_memory(
        &self,
        mut record: MemoryRecord,
        _ctx: &RequestContext,
    ) -> Result<EntityId> {
        record.validate()?;
        if record.layer_history.is_empty() {
            apply_classification(&mut record, LayerAssignment::Classifier);
        }
        self.repository.save(&record).await
    }

    /// Get a memory record by ID.
    pub async fn get_memory(&self, id: &EntityId) -> Result<MemoryRecord> {
        self.repository
            .get_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Memory record {} not found", id)))
    }

    /// Update a memory record with validation.
    ///
    /// Re-classification: when the title or content changed and the current
    /// layer was NOT pinned by an explicit user choice (the last history entry
    /// is `classifier`), the classifier re-evaluates the record. User-chosen
    /// layers always win.
    pub async fn update_memory(
        &self,
        mut record: MemoryRecord,
        _ctx: &RequestContext,
    ) -> Result<()> {
        record.validate()?;
        record.touch();

        let pinned_by_user = record
            .layer_history
            .last()
            .map(|e| e.by == LayerAssignment::User)
            .unwrap_or(false);
        if !pinned_by_user {
            apply_classification(&mut record, LayerAssignment::Classifier);
        }
        self.repository.update(&record).await
    }

    /// Archive (soft-delete) a memory record.
    pub async fn archive_memory(&self, id: &EntityId, _ctx: &RequestContext) -> Result<()> {
        let mut record = self.get_memory(id).await?;
        record.status = MemoryStatus::Archived;
        record.touch();
        self.repository.update(&record).await
    }

    /// List memory records with pagination.
    pub async fn list_memories(&self, limit: u32, offset: u32) -> Result<Vec<MemoryRecord>> {
        self.repository.list(limit, offset).await
    }

    /// Count total memory records.
    pub async fn count_memories(&self) -> Result<u64> {
        self.repository.count().await
    }

    /// Search memory records by query string.
    pub async fn search_memories(&self, query: &str) -> Result<Vec<MemoryRecord>> {
        self.repository.search(query).await
    }

    /// Recall memories with context (delegates to recall service).
    pub async fn recall(&self, query: &str, context: &RecallContext) -> Result<RecallResult> {
        self.recall_service.recall(query, context).await
    }

    /// Compress a set of records into a summary.
    pub async fn compress(
        &self,
        records: &[MemoryRecord],
    ) -> Result<crate::core::memory::memory_compression::CompressedMemory> {
        self.compression_service.compress(records).await
    }

    /// Set visibility on a memory record.
    pub async fn set_visibility(
        &self,
        id: &EntityId,
        visibility: MemoryVisibility,
        _ctx: &RequestContext,
    ) -> Result<()> {
        let mut record = self.get_memory(id).await?;
        record.visibility = visibility;
        record.touch();
        self.repository.update(&record).await
    }

    /// Explicitly assign a layer to a memory — a user choice.
    ///
    /// Records full provenance: confidence 1.0 (user is authoritative), a
    /// reason (either provided or the previous one), and a history entry
    /// tagged `user`, which pins the layer against auto-reclassification.
    pub async fn set_layer(
        &self,
        id: &EntityId,
        layer: MemoryLayer,
        reason: Option<String>,
        _ctx: &RequestContext,
    ) -> Result<()> {
        let mut record = self.get_memory(id).await?;
        record.layer = layer.clone();
        record.layer_confidence = 1.0;
        record.layer_reason = reason.unwrap_or_else(|| "user-assigned layer".to_string());
        record.layer_updated_at = Some(Utc::now());
        record.layer_history.push(LayerHistoryEntry {
            layer,
            confidence: 1.0,
            reason: record.layer_reason.clone(),
            at: Utc::now().to_rfc3339(),
            by: LayerAssignment::User,
        });
        record.touch();
        self.repository.update(&record).await
    }

    /// Promote a memory to a higher layer — kept for API compatibility;
    /// delegates to [`Self::set_layer`] with a default reason.
    pub async fn promote_layer(
        &self,
        id: &EntityId,
        layer: MemoryLayer,
        _ctx: &RequestContext,
    ) -> Result<()> {
        self.set_layer(id, layer, None, _ctx).await
    }

    /// Re-run the classifier on a memory and persist the result (with a new
    /// history entry tagged `classifier`) unless the layer is user-pinned.
    /// Returns the effective classification: the pin wins over the classifier,
    /// so a pinned memory reports its current state unchanged.
    pub async fn reclassify(
        &self,
        id: &EntityId,
        _ctx: &RequestContext,
    ) -> Result<crate::core::memory::layer::LayerClassification> {
        let mut record = self.get_memory(id).await?;
        let pinned_by_user = record
            .layer_history
            .last()
            .map(|e| e.by == LayerAssignment::User)
            .unwrap_or(false);
        // A manual choice pins the layer: re-running the classifier must not
        // overwrite it. Report the current (pinned) state as the result.
        if pinned_by_user {
            return Ok(crate::core::memory::layer::LayerClassification {
                layer: record.layer,
                confidence: record.layer_confidence,
                reason: record.layer_reason,
            });
        }
        let classification = apply_classification(&mut record, LayerAssignment::Classifier);
        record.touch();
        self.repository.update(&record).await?;
        Ok(classification)
    }

    /// Full layer history, newest first.
    pub async fn get_layer_history(&self, id: &EntityId) -> Result<Vec<LayerHistoryEntry>> {
        let record = self.get_memory(id).await?;
        let mut history = record.layer_history;
        LayerHistoryEntry::sort_newest_first(&mut history);
        Ok(history)
    }

    /// Distribution of layers across the memory pool, with mean confidence.
    pub async fn get_layer_stats(&self) -> Result<Vec<LayerStat>> {
        let records = self.repository.list(10_000, 0).await?;
        let mut by_layer: std::collections::HashMap<MemoryLayer, (u64, f64)> =
            std::collections::HashMap::new();
        for r in &records {
            let entry = by_layer.entry(r.layer.clone()).or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += r.layer_confidence;
        }
        let mut stats: Vec<LayerStat> = by_layer
            .into_iter()
            .map(|(layer, (count, conf_sum))| LayerStat {
                layer: layer.as_str().to_string(),
                count,
                mean_confidence: if count == 0 {
                    0.0
                } else {
                    conf_sum / count as f64
                },
            })
            .collect();
        stats.sort_by_key(|s| std::cmp::Reverse(s.count));
        Ok(stats)
    }

    /// Get memories for a specific project.
    pub async fn get_by_project(&self, project_id: &EntityId) -> Result<Vec<MemoryRecord>> {
        self.repository.get_by_project(project_id).await
    }
}

/// Distribution of memories across cognitive layers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LayerStat {
    pub layer: String,
    pub count: u64,
    pub mean_confidence: f64,
}

/// Run the signature classifier on a record and write provenance onto it.
/// Appends a history entry. Returns the classification for callers that need
/// the raw result.
fn apply_classification(
    record: &mut MemoryRecord,
    by: LayerAssignment,
) -> crate::core::memory::layer::LayerClassification {
    let classification = LayerClassifier::classify(
        &record.title,
        &record.content,
        record.source.clone(),
        record.memory_state.clone(),
        record.importance_score,
    );
    record.layer = classification.layer.clone();
    record.layer_confidence = classification.confidence;
    record.layer_reason = classification.reason.clone();
    record.layer_updated_at = Some(Utc::now());
    record.layer_history.push(LayerHistoryEntry {
        layer: classification.layer.clone(),
        confidence: classification.confidence,
        reason: classification.reason.clone(),
        at: Utc::now().to_rfc3339(),
        by,
    });
    classification
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::memory_compression::SimpleCompressionService;
    use crate::core::memory::memory_recall::RecallContext;
    use crate::core::memory::types::{MemoryLayer, MemorySource, MemoryStatus};
    use crate::core::security::RequestContext;
    use async_trait::async_trait;
    use std::sync::Arc;

    // ── In-memory mock repository ──

    struct MockMemoryRepository {
        records: tokio::sync::Mutex<Vec<MemoryRecord>>,
    }

    impl MockMemoryRepository {
        fn new() -> Self {
            Self {
                records: tokio::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl MemoryRepository for MockMemoryRepository {
        async fn save(&self, record: &MemoryRecord) -> Result<EntityId> {
            let mut records = self.records.lock().await;
            let id = record.id.clone();
            records.push(record.clone());
            Ok(id)
        }

        async fn get_by_id(&self, id: &EntityId) -> Result<Option<MemoryRecord>> {
            let records = self.records.lock().await;
            Ok(records.iter().find(|r| r.id == *id).cloned())
        }

        async fn get_by_project(&self, project_id: &EntityId) -> Result<Vec<MemoryRecord>> {
            let records = self.records.lock().await;
            Ok(records
                .iter()
                .filter(|r| r.project_space_id.as_ref() == Some(project_id))
                .cloned()
                .collect())
        }

        async fn search(&self, query: &str) -> Result<Vec<MemoryRecord>> {
            let records = self.records.lock().await;
            let q = query.to_lowercase();
            Ok(records
                .iter()
                .filter(|r| {
                    r.title.to_lowercase().contains(&q) || r.content.to_lowercase().contains(&q)
                })
                .cloned()
                .collect())
        }

        async fn update(&self, record: &MemoryRecord) -> Result<()> {
            let mut records = self.records.lock().await;
            if let Some(existing) = records.iter_mut().find(|r| r.id == record.id) {
                *existing = record.clone();
            }
            Ok(())
        }

        async fn delete(&self, id: &EntityId) -> Result<()> {
            let mut records = self.records.lock().await;
            records.retain(|r| r.id != *id);
            Ok(())
        }

        async fn list(&self, limit: u32, offset: u32) -> Result<Vec<MemoryRecord>> {
            let records = self.records.lock().await;
            Ok(records
                .iter()
                .skip(offset as usize)
                .take(limit as usize)
                .cloned()
                .collect())
        }

        async fn count(&self) -> Result<u64> {
            let records = self.records.lock().await;
            Ok(records.len() as u64)
        }
    }

    // ── Mock recall service ──

    struct MockRecallService;

    #[async_trait]
    impl MemoryRecallService for MockRecallService {
        async fn recall(&self, _query: &str, _context: &RecallContext) -> Result<RecallResult> {
            Ok(RecallResult {
                records: vec![],
                score: 0.0,
            })
        }

        async fn recall_by_entity(&self, _entity_id: &EntityId) -> Result<RecallResult> {
            Ok(RecallResult {
                records: vec![],
                score: 0.0,
            })
        }

        async fn recall_recent(&self, _limit: u32) -> Result<Vec<MemoryRecord>> {
            Ok(vec![])
        }
    }

    fn sample_record() -> MemoryRecord {
        MemoryRecord::new(
            "Test".to_string(),
            "Content".to_string(),
            "author".to_string(),
            MemorySource::Manual,
        )
        .unwrap()
    }

    fn test_ctx() -> RequestContext {
        RequestContext::new(
            "user-1".to_string(),
            "session-1".to_string(),
            "device-1".to_string(),
        )
    }

    fn test_service() -> MemoryService {
        MemoryService::new(
            Arc::new(MockMemoryRepository::new()),
            Arc::new(MockRecallService),
            Arc::new(SimpleCompressionService),
        )
    }

    #[tokio::test]
    async fn create_and_get_memory() {
        let svc = test_service();
        let record = sample_record();
        let id = svc.create_memory(record, &test_ctx()).await.unwrap();
        let fetched = svc.get_memory(&id).await.unwrap();
        assert_eq!(fetched.title, "Test");
    }

    #[tokio::test]
    async fn create_empty_title_fails() {
        let svc = test_service();
        let mut record = sample_record();
        record.title = "  ".to_string();
        let result = svc.create_memory(record, &test_ctx()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn get_nonexistent_fails() {
        let svc = test_service();
        let result = svc.get_memory(&EntityId::new()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn update_memory() {
        let svc = test_service();
        let record = sample_record();
        let id = svc.create_memory(record, &test_ctx()).await.unwrap();
        let mut fetched = svc.get_memory(&id).await.unwrap();
        fetched.title = "Updated".to_string();
        svc.update_memory(fetched, &test_ctx()).await.unwrap();
        let updated = svc.get_memory(&id).await.unwrap();
        assert_eq!(updated.title, "Updated");
    }

    #[tokio::test]
    async fn archive_memory() {
        let svc = test_service();
        let record = sample_record();
        let id = svc.create_memory(record, &test_ctx()).await.unwrap();
        svc.archive_memory(&id, &test_ctx()).await.unwrap();
        let archived = svc.get_memory(&id).await.unwrap();
        assert_eq!(archived.status, MemoryStatus::Archived);
    }

    #[tokio::test]
    async fn list_and_count() {
        let svc = test_service();
        for i in 0..5 {
            let mut r = sample_record();
            r.title = format!("Record {}", i);
            svc.create_memory(r, &test_ctx()).await.unwrap();
        }
        let count = svc.count_memories().await.unwrap();
        assert_eq!(count, 5);
        let list = svc.list_memories(3, 0).await.unwrap();
        assert_eq!(list.len(), 3);
    }

    #[tokio::test]
    async fn search() {
        let svc = test_service();
        let mut r = sample_record();
        r.title = "Rust language".to_string();
        svc.create_memory(r, &test_ctx()).await.unwrap();
        let results = svc.search_memories("Rust").await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn set_visibility() {
        let svc = test_service();
        let record = sample_record();
        let id = svc.create_memory(record, &test_ctx()).await.unwrap();
        svc.set_visibility(&id, MemoryVisibility::Public, &test_ctx())
            .await
            .unwrap();
        let updated = svc.get_memory(&id).await.unwrap();
        assert_eq!(updated.visibility, MemoryVisibility::Public);
    }

    #[tokio::test]
    async fn promote_layer() {
        let svc = test_service();
        let record = sample_record();
        let id = svc.create_memory(record, &test_ctx()).await.unwrap();
        svc.promote_layer(&id, MemoryLayer::Semantic, &test_ctx())
            .await
            .unwrap();
        let updated = svc.get_memory(&id).await.unwrap();
        assert_eq!(updated.layer, MemoryLayer::Semantic);
    }

    #[tokio::test]
    async fn create_memory_auto_classifies() {
        let svc = test_service();
        // A decision-flavoured record with no explicit layer history must be
        // auto-classified to Decision with provenance.
        let record = MemoryRecord::new(
            "Redis".to_string(),
            "3 августа отказались от Redis".to_string(),
            "author".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        let id = svc.create_memory(record, &test_ctx()).await.unwrap();
        let saved = svc.get_memory(&id).await.unwrap();
        assert_eq!(saved.layer, MemoryLayer::Decision);
        assert!(saved.layer_confidence >= 0.5);
        assert!(!saved.layer_reason.is_empty());
        assert!(saved.layer_updated_at.is_some());
        assert_eq!(saved.layer_history.len(), 1);
        assert_eq!(saved.layer_history[0].by, LayerAssignment::Classifier);
    }

    #[tokio::test]
    async fn create_memory_keeps_explicit_user_layer() {
        let svc = test_service();
        let mut record = sample_record();
        // Simulate a user-pinned layer: non-empty history with a user entry.
        record.layer = MemoryLayer::Strategic;
        record.layer_confidence = 1.0;
        record.layer_history.push(LayerHistoryEntry {
            layer: MemoryLayer::Strategic,
            confidence: 1.0,
            reason: "user picked".to_string(),
            at: Utc::now().to_rfc3339(),
            by: LayerAssignment::User,
        });
        let id = svc.create_memory(record, &test_ctx()).await.unwrap();
        let saved = svc.get_memory(&id).await.unwrap();
        assert_eq!(saved.layer, MemoryLayer::Strategic);
        assert_eq!(saved.layer_history.len(), 1); // no extra classifier entry
    }

    #[tokio::test]
    async fn update_memory_reclassifies_unpinned() {
        let svc = test_service();
        let record = sample_record();
        let id = svc.create_memory(record, &test_ctx()).await.unwrap();
        // Rewrite content into a decision; layer was classifier-pinned, so
        // update must re-classify.
        let mut fetched = svc.get_memory(&id).await.unwrap();
        fetched.content = "мы решили отказаться от Redis".to_string();
        svc.update_memory(fetched, &test_ctx()).await.unwrap();
        let updated = svc.get_memory(&id).await.unwrap();
        assert_eq!(updated.layer, MemoryLayer::Decision);
        assert!(updated.layer_history.len() >= 2);
    }

    #[tokio::test]
    async fn update_memory_keeps_user_pinned_layer() {
        let svc = test_service();
        let mut record = sample_record();
        record.layer = MemoryLayer::Strategic;
        record.layer_confidence = 1.0;
        record.layer_history.push(LayerHistoryEntry {
            layer: MemoryLayer::Strategic,
            confidence: 1.0,
            reason: "user picked".to_string(),
            at: Utc::now().to_rfc3339(),
            by: LayerAssignment::User,
        });
        let id = svc.create_memory(record, &test_ctx()).await.unwrap();
        let mut fetched = svc.get_memory(&id).await.unwrap();
        fetched.content = "вчера пробовали фиксить баг".to_string(); // episodic content
        svc.update_memory(fetched, &test_ctx()).await.unwrap();
        let updated = svc.get_memory(&id).await.unwrap();
        // User choice wins over the classifier.
        assert_eq!(updated.layer, MemoryLayer::Strategic);
    }

    #[tokio::test]
    async fn set_layer_records_user_history() {
        let svc = test_service();
        let record = sample_record();
        let id = svc.create_memory(record, &test_ctx()).await.unwrap();
        svc.set_layer(
            &id,
            MemoryLayer::Procedural,
            Some("because steps".to_string()),
            &test_ctx(),
        )
        .await
        .unwrap();
        let updated = svc.get_memory(&id).await.unwrap();
        assert_eq!(updated.layer, MemoryLayer::Procedural);
        assert_eq!(updated.layer_confidence, 1.0);
        assert_eq!(updated.layer_reason, "because steps");
        let last = updated.layer_history.last().unwrap();
        assert_eq!(last.by, LayerAssignment::User);
        assert_eq!(last.confidence, 1.0);
    }

    #[tokio::test]
    async fn get_layer_history_newest_first() {
        let svc = test_service();
        let record = sample_record();
        let id = svc.create_memory(record, &test_ctx()).await.unwrap();
        svc.set_layer(&id, MemoryLayer::Semantic, None, &test_ctx())
            .await
            .unwrap();
        let history = svc.get_layer_history(&id).await.unwrap();
        assert!(history.len() >= 2);
        assert_eq!(history[0].by, LayerAssignment::User); // newest first
    }

    #[tokio::test]
    async fn reclassify_returns_new_layer() {
        let svc = test_service();
        let record = sample_record();
        let id = svc.create_memory(record, &test_ctx()).await.unwrap();
        let mut fetched = svc.get_memory(&id).await.unwrap();
        fetched.content = "это принцип: архитектура должна оставаться локальной".to_string();
        svc.update_memory(fetched, &test_ctx()).await.unwrap();
        let classification = svc.reclassify(&id, &test_ctx()).await.unwrap();
        assert_eq!(classification.layer, MemoryLayer::Strategic);
        let saved = svc.get_memory(&id).await.unwrap();
        assert_eq!(saved.layer, MemoryLayer::Strategic);
        assert!(!saved.layer_history.is_empty());
    }

    #[tokio::test]
    async fn layer_stats_aggregate() {
        let svc = test_service();
        for i in 0..3 {
            let mut r = sample_record();
            r.title = format!("Decision {}", i);
            r.content = "решили выбрать вариант".to_string();
            svc.create_memory(r, &test_ctx()).await.unwrap();
        }
        let mut r = sample_record();
        r.title = "Факт".to_string();
        r.content = "API реализован через JWT".to_string();
        svc.create_memory(r, &test_ctx()).await.unwrap();

        let stats = svc.get_layer_stats().await.unwrap();
        let decision = stats
            .iter()
            .find(|s| s.layer == "Decision")
            .expect("Decision layer present");
        assert_eq!(decision.count, 3);
        assert!((0.5..=1.0).contains(&decision.mean_confidence));
        let semantic = stats
            .iter()
            .find(|s| s.layer == "Semantic")
            .expect("Semantic layer present");
        assert_eq!(semantic.count, 1);
    }
}
