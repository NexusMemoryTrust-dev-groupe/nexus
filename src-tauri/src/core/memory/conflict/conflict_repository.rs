use async_trait::async_trait;

use crate::core::entity_id::EntityId;
use crate::core::memory::conflict::{ConflictGroup, ConflictStatus};
use crate::core::result::Result;

/// Repository trait for conflict group persistence.
#[async_trait]
pub trait ConflictRepository: Send + Sync {
    /// Persist a new conflict group. Returns its ID.
    async fn save_group(&self, group: &ConflictGroup) -> Result<EntityId>;

    /// Retrieve a conflict group by its ID.
    async fn get_group(&self, id: &EntityId) -> Result<Option<ConflictGroup>>;

    /// All groups, optionally filtered by status (None = both), newest first.
    async fn list_groups(&self, status: Option<ConflictStatus>) -> Result<Vec<ConflictGroup>>;

    /// Update an existing group in-place (resolution, status, members).
    async fn update_group(&self, group: &ConflictGroup) -> Result<()>;

    /// Open (unresolved) groups that contain the given memory.
    async fn open_groups_containing(&self, memory_id: &EntityId) -> Result<Vec<ConflictGroup>>;
}
