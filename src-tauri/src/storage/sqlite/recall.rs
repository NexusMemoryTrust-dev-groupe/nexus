use std::sync::Arc;

use async_trait::async_trait;

use crate::core::entity_id::EntityId;
use crate::core::memory::memory_recall::{MemoryRecallService, RecallContext, RecallResult};
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::result::Result;

/// In-memory recall service that delegates search to the repository
/// and ranks results by confidence score.
pub struct InMemoryRecallService {
    repository: Arc<dyn MemoryRepository>,
}

impl InMemoryRecallService {
    /// Create a new recall service. Used by command layer and tests.
    #[allow(dead_code)] // Used in tests and available for future command layer
    pub fn new(repository: Arc<dyn MemoryRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl MemoryRecallService for InMemoryRecallService {
    async fn recall(&self, query: &str, context: &RecallContext) -> Result<RecallResult> {
        let mut records = self.repository.search(query).await?;

        // Filter by confidence threshold
        records.retain(|r| r.confidence_score >= context.min_confidence);

        // Filter by project if specified
        if let Some(ref project_id) = context.project_id {
            records.retain(|r| r.project_space_id.as_ref() == Some(project_id));
        }

        // Sort by confidence_score descending
        records.sort_by(|a, b| {
            b.confidence_score
                .partial_cmp(&a.confidence_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Limit results
        records.truncate(context.max_results as usize);

        let score = if records.is_empty() {
            0.0
        } else {
            records.iter().map(|r| r.confidence_score).sum::<f64>() / records.len() as f64
        };

        Ok(RecallResult { records, score })
    }

    async fn recall_by_entity(&self, entity_id: &EntityId) -> Result<RecallResult> {
        let all = self.repository.search("").await?;
        let records: Vec<MemoryRecord> = all
            .into_iter()
            .filter(|r| r.linked_entity_ids.contains(entity_id))
            .collect();

        let score = if records.is_empty() {
            0.0
        } else {
            records.iter().map(|r| r.confidence_score).sum::<f64>() / records.len() as f64
        };

        Ok(RecallResult { records, score })
    }

    async fn recall_recent(&self, limit: u32) -> Result<Vec<MemoryRecord>> {
        self.repository.list(limit, 0).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::types::MemorySource;

    fn sample_record(title: &str, confidence: f64) -> MemoryRecord {
        let mut r = MemoryRecord::new(
            title.to_string(),
            format!("Content for {}", title),
            "author".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        r.confidence_score = confidence;
        r
    }

    // ── Minimal in-memory mock for recall tests ──

    struct MockRepo {
        records: tokio::sync::Mutex<Vec<MemoryRecord>>,
    }

    impl MockRepo {
        fn new() -> Self {
            Self {
                records: tokio::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl MemoryRepository for MockRepo {
        async fn save(&self, record: &MemoryRecord) -> Result<EntityId> {
            let mut recs = self.records.lock().await;
            let id = record.id.clone();
            recs.push(record.clone());
            Ok(id)
        }
        async fn get_by_id(&self, id: &EntityId) -> Result<Option<MemoryRecord>> {
            let recs = self.records.lock().await;
            Ok(recs.iter().find(|r| r.id == *id).cloned())
        }
        async fn get_by_project(&self, _pid: &EntityId) -> Result<Vec<MemoryRecord>> {
            Ok(vec![])
        }
        async fn search(&self, query: &str) -> Result<Vec<MemoryRecord>> {
            let recs = self.records.lock().await;
            if query.is_empty() {
                return Ok(recs.clone());
            }
            let q = query.to_lowercase();
            Ok(recs
                .iter()
                .filter(|r| r.title.to_lowercase().contains(&q))
                .cloned()
                .collect())
        }
        async fn update(&self, _record: &MemoryRecord) -> Result<()> {
            Ok(())
        }
        async fn delete(&self, _id: &EntityId) -> Result<()> {
            Ok(())
        }
        async fn list(&self, limit: u32, offset: u32) -> Result<Vec<MemoryRecord>> {
            let recs = self.records.lock().await;
            Ok(recs
                .iter()
                .skip(offset as usize)
                .take(limit as usize)
                .cloned()
                .collect())
        }
        async fn count(&self) -> Result<u64> {
            let recs = self.records.lock().await;
            Ok(recs.len() as u64)
        }
    }

    async fn setup_service() -> InMemoryRecallService {
        let repo = Arc::new(MockRepo::new());
        let svc = InMemoryRecallService::new(repo.clone());
        repo.save(&sample_record("Rust basics", 0.9)).await.unwrap();
        repo.save(&sample_record("Rust advanced", 0.7))
            .await
            .unwrap();
        repo.save(&sample_record("Python basics", 0.5))
            .await
            .unwrap();
        svc
    }

    #[tokio::test]
    async fn recall_finds_matching() {
        let svc = setup_service().await;
        let result = svc.recall("Rust", &RecallContext::default()).await.unwrap();
        assert_eq!(result.records.len(), 2);
        // Sorted by confidence desc
        assert!(result.records[0].confidence_score >= result.records[1].confidence_score);
    }

    #[tokio::test]
    async fn recall_filters_by_confidence() {
        let svc = setup_service().await;
        let ctx = RecallContext {
            min_confidence: 0.8,
            ..Default::default()
        };
        let result = svc.recall("Rust", &ctx).await.unwrap();
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0].title, "Rust basics");
    }

    #[tokio::test]
    async fn recall_limits_results() {
        let svc = setup_service().await;
        let ctx = RecallContext {
            max_results: 1,
            ..Default::default()
        };
        let result = svc.recall("Rust", &ctx).await.unwrap();
        assert_eq!(result.records.len(), 1);
    }

    #[tokio::test]
    async fn recall_no_match() {
        let svc = setup_service().await;
        let result = svc
            .recall("Go language", &RecallContext::default())
            .await
            .unwrap();
        assert!(result.records.is_empty());
        assert_eq!(result.score, 0.0);
    }

    #[tokio::test]
    async fn recall_recent() {
        let svc = setup_service().await;
        let records = svc.recall_recent(2).await.unwrap();
        assert_eq!(records.len(), 2);
    }

    #[tokio::test]
    async fn recall_score_averages_confidence() {
        let svc = setup_service().await;
        let result = svc.recall("Rust", &RecallContext::default()).await.unwrap();
        let expected_score = (0.9 + 0.7) / 2.0;
        assert!((result.score - expected_score).abs() < f64::EPSILON);
    }
}
