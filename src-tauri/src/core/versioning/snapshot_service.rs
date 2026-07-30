use async_trait::async_trait;

use crate::core::entity_id::EntityId;
use crate::core::result::Result;

/// Service for capturing, storing, and retrieving entity snapshots.
/// Snapshots are periodic full-state captures used for baseline recovery.
#[async_trait]
pub trait SnapshotService: Send + Sync {
    /// Capture the current state of an entity as bytes.
    async fn capture(&self, entity_type: &str, entity_id: &EntityId) -> Result<Vec<u8>>;

    /// Store a snapshot and return its ID.
    async fn store(
        &self,
        snapshot: &[u8],
        entity_type: &str,
        entity_id: &EntityId,
    ) -> Result<String>;

    /// Retrieve a snapshot by its ID.
    async fn get(&self, snapshot_id: &str) -> Result<Option<Vec<u8>>>;

    /// Get the latest baseline snapshot ID for an entity.
    async fn get_baseline(
        &self,
        entity_type: &str,
        entity_id: &EntityId,
    ) -> Result<Option<String>>;
}

#[cfg(test)]
mod tests {
    // SnapshotService is a trait — concrete implementations tested in storage/sqlite/
}
