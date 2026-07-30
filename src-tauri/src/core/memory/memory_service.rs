use std::sync::Arc;

use crate::core::entity_id::EntityId;
use crate::core::memory::memory_compression::MemoryCompressionService;
use crate::core::memory::memory_recall::{RecallContext, RecallResult, MemoryRecallService};
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::memory::types::{MemoryLayer, MemoryStatus, MemoryVisibility};
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
    pub async fn create_memory(
        &self,
        record: MemoryRecord,
        _ctx: &RequestContext,
    ) -> Result<EntityId> {
        record.validate()?;
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
    pub async fn update_memory(
        &self,
        mut record: MemoryRecord,
        _ctx: &RequestContext,
    ) -> Result<()> {
        record.validate()?;
        record.touch();
        self.repository.update(&record).await
    }

    /// Archive (soft-delete) a memory record.
    pub async fn archive_memory(
        &self,
        id: &EntityId,
        _ctx: &RequestContext,
    ) -> Result<()> {
        let mut record = self.get_memory(id).await?;
        record.status = MemoryStatus::Archived;
        record.touch();
        self.repository.update(&record).await
    }

    /// List memory records with pagination.
    pub async fn list_memories(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MemoryRecord>> {
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
    pub async fn recall(
        &self,
        query: &str,
        context: &RecallContext,
    ) -> Result<RecallResult> {
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

    /// Promote a memory to a higher layer.
    pub async fn promote_layer(
        &self,
        id: &EntityId,
        layer: MemoryLayer,
        _ctx: &RequestContext,
    ) -> Result<()> {
        let mut record = self.get_memory(id).await?;
        record.layer = layer;
        record.touch();
        self.repository.update(&record).await
    }

    /// Get memories for a specific project.
    pub async fn get_by_project(
        &self,
        project_id: &EntityId,
    ) -> Result<Vec<MemoryRecord>> {
        self.repository.get_by_project(project_id).await
    }
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
                    r.title.to_lowercase().contains(&q)
                        || r.content.to_lowercase().contains(&q)
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
        svc.promote_layer(&id, MemoryLayer::Knowledge, &test_ctx())
            .await
            .unwrap();
        let updated = svc.get_memory(&id).await.unwrap();
        assert_eq!(updated.layer, MemoryLayer::Knowledge);
    }
}
