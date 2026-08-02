use async_trait::async_trait;

use crate::core::entity_id::EntityId;
use crate::core::graph::entity::Entity;
use crate::core::result::Result;

/// Service for entity identity resolution — deduplication, merging, canonical forms.
#[async_trait]
pub trait EntityIdentityService: Send + Sync {
    /// Find potential duplicate entities based on title/type similarity.
    async fn find_duplicates(&self, entity: &Entity) -> Result<Vec<Entity>>;

    /// Merge duplicate entities into a primary one.
    /// The primary entity retains its ID; duplicates are marked as Merged.
    async fn merge_entities(&self, primary: &EntityId, duplicates: &[EntityId]) -> Result<Entity>;

    /// Get the canonical entity for a given entity.
    /// If the entity has been merged, returns the primary entity.
    async fn get_canonical(&self, entity_id: &EntityId) -> Result<Entity>;

    /// Resolve an alias (name/alias) to an entity.
    async fn resolve_alias(&self, name: &str) -> Result<Option<Entity>>;
}

#[cfg(test)]
mod tests {
    // EntityIdentityService is a trait — concrete implementations tested in storage/sqlite/
}
