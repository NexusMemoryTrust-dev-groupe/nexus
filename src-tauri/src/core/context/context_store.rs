use async_trait::async_trait;

use crate::core::context::context_package::ContextPackage;
use crate::core::context::context_snapshot::ContextSnapshot;
use crate::core::entity_id::EntityId;
use crate::core::result::Result;

/// Persistent storage for context snapshots.
#[async_trait]
pub trait ContextStore: Send + Sync {
    /// Save a snapshot and return its ID.
    async fn save_snapshot(&self, snapshot: &ContextSnapshot) -> Result<String>;

    /// Get a snapshot by its ID.
    async fn get_snapshot(&self, snapshot_id: &str) -> Result<Option<ContextSnapshot>>;

    /// List all snapshots for an entity.
    async fn list_snapshots(&self, entity_id: &EntityId) -> Result<Vec<ContextSnapshot>>;

    /// Restore a context package from a snapshot.
    async fn restore_snapshot(&self, snapshot_id: &str) -> Result<ContextPackage>;
}

#[cfg(test)]
mod tests {
    // ContextStore is a trait — concrete implementations tested in storage/sqlite/
}
