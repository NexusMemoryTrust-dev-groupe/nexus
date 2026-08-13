use async_trait::async_trait;

use crate::core::entity_id::EntityId;
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::result::Result;

/// Repository trait for MemoryRecord persistence.
/// Implementations must be Send + Sync for async access.
#[async_trait]
pub trait MemoryRepository: Send + Sync {
    /// Persist a new memory record. Returns its ID.
    async fn save(&self, record: &MemoryRecord) -> Result<EntityId>;

    /// Persist many memory records. The default implementation saves them one
    /// by one; storage backends may override this with a single transaction to
    /// avoid one WAL commit per record (bulk import / benchmarks).
    async fn save_many(&self, records: &[MemoryRecord]) -> Result<()> {
        for record in records {
            self.save(record).await?;
        }
        Ok(())
    }

    /// Retrieve a memory record by its ID.
    async fn get_by_id(&self, id: &EntityId) -> Result<Option<MemoryRecord>>;

    /// Retrieve all memory records belonging to a project space.
    async fn get_by_project(&self, project_id: &EntityId) -> Result<Vec<MemoryRecord>>;

    /// Full-text search across title, summary, and content.
    async fn search(&self, query: &str) -> Result<Vec<MemoryRecord>>;

    /// Update an existing memory record in-place.
    async fn update(&self, record: &MemoryRecord) -> Result<()>;

    /// Soft-delete a memory record (set status to Archived).
    async fn delete(&self, id: &EntityId) -> Result<()>;

    /// List records with pagination.
    async fn list(&self, limit: u32, offset: u32) -> Result<Vec<MemoryRecord>>;

    /// Count total records (for pagination metadata).
    async fn count(&self) -> Result<u64>;
}

#[cfg(test)]
mod tests {
    // MemoryRepository is a trait — concrete implementations tested in storage/sqlite/
}
