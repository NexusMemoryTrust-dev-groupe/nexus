use crate::core::context::context_package::UserIntent;
use crate::core::entity_id::EntityId;
use crate::core::graph::entity::Entity;
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::result::Result;

/// Injects relevant memory records into the context based on entities and intent.
/// Now with recent and important memory injection.
pub struct MemoryInjector<M: MemoryRepository> {
    memory_repo: M,
}

impl<M: MemoryRepository> MemoryInjector<M> {
    pub fn new(memory_repo: M) -> Self {
        Self { memory_repo }
    }

    /// Find memory records relevant to the given entities and intent.
    /// Optimized: single combined query, limited recent/important, no N separate searches.
    pub async fn inject(
        &self,
        entities: &[Entity],
        intent: &UserIntent,
    ) -> Result<Vec<MemoryRecord>> {
        let mut records = Vec::new();

        // 1. Single combined search: entity titles + intent query (not N searches)
        let combined_query = self.build_combined_query(entities, &intent.query);
        if !combined_query.is_empty() {
            let found = self.memory_repo.search(&combined_query).await?;
            records.extend(found);
        }

        // 2. Search by top keywords only (limit to 3 most important keywords)
        let top_keywords: Vec<&str> = intent.keywords.iter().take(3).map(|s| s.as_str()).collect();
        for keyword in &top_keywords {
            let found = self.memory_repo.search(keyword).await?;
            records.extend(found);
        }

        // 3. Get recent memories (last 7 days) — limit to top 5 by importance
        let recent = self.get_recent_memories(7, 5).await?;
        records.extend(recent);

        // 4. Get important memories (importance > 0.7) — limit to top 5
        let important = self.get_important_memories(0.7, 5).await?;
        records.extend(important);

        // Deduplicate by ID
        records.dedup_by_key(|r| r.id.clone());

        // Sort by importance (descending)
        records.sort_by(|a, b| {
            b.importance_score
                .partial_cmp(&a.importance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Hard limit: max 15 memories total (prevents context explosion)
        records.truncate(15);

        Ok(records)
    }

    /// Build a single combined query string from entity titles.
    /// Instead of N separate searches, one query with all entity names.
    fn build_combined_query(&self, entities: &[Entity], intent_query: &str) -> String {
        let mut parts: Vec<String> = Vec::new();
        
        // Add intent query first (most relevant)
        if !intent_query.is_empty() {
            parts.push(intent_query.to_string());
        }
        
        // Add top 5 entity titles (most entities = most relevant)
        for entity in entities.iter().take(5) {
            parts.push(entity.title.clone());
        }
        
        parts.join(" ")
    }

    /// Find memory records related to a specific entity.
    pub async fn inject_for_entity(
        &self,
        entity_id: &EntityId,
    ) -> Result<Vec<MemoryRecord>> {
        let query = entity_id.as_str().to_string();
        self.memory_repo.search(&query).await
    }

    /// Get recent memories from the last N days, limited to K results.
    async fn get_recent_memories(&self, days: u32, limit: usize) -> Result<Vec<MemoryRecord>> {
        let all = self.memory_repo.list(50, 0).await?; // Load at most 50
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let mut recent: Vec<MemoryRecord> = all
            .into_iter()
            .filter(|r| r.created_at >= cutoff)
            .collect();
        // Sort by importance descending, take top N
        recent.sort_by(|a, b| {
            b.importance_score
                .partial_cmp(&a.importance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        recent.truncate(limit);
        Ok(recent)
    }

    /// Get memories with importance above threshold, limited to K results.
    async fn get_important_memories(&self, threshold: f64, limit: usize) -> Result<Vec<MemoryRecord>> {
        let all = self.memory_repo.list(50, 0).await?; // Load at most 50
        let mut important: Vec<MemoryRecord> = all
            .into_iter()
            .filter(|r| r.importance_score >= threshold)
            .collect();
        // Already filtered by importance, sort by recency descending
        important.sort_by(|a, b| {
            b.created_at
                .partial_cmp(&a.created_at)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        important.truncate(limit);
        Ok(important)
    }
}

#[cfg(test)]
mod tests {
    // MemoryInjector tests require a MemoryRepository mock — tested via integration
}
