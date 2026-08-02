use async_trait::async_trait;

use crate::core::entity_id::EntityId;
use crate::core::graph::entity::Entity;
use crate::core::graph::entity_types::EntityType;
use crate::core::graph::relationship::Relationship;
use crate::core::result::Result;

/// Storage trait for graph entities and relationships.
/// All operations are async for non-blocking I/O.
#[async_trait]
pub trait GraphStore: Send + Sync {
    /// Add an entity to the graph. Returns the entity ID.
    async fn add_entity(&self, entity: &Entity) -> Result<EntityId>;

    /// Get an entity by its ID.
    async fn get_entity(&self, id: &EntityId) -> Result<Option<Entity>>;

    /// Update an existing entity.
    async fn update_entity(&self, entity: &Entity) -> Result<()>;

    /// Delete an entity by its ID.
    async fn delete_entity(&self, id: &EntityId) -> Result<()>;

    /// Add a relationship to the graph. Returns the relationship ID.
    async fn add_relationship(&self, relationship: &Relationship) -> Result<EntityId>;

    /// Get a relationship by its ID.
    async fn get_relationship(&self, id: &EntityId) -> Result<Option<Relationship>>;

    /// Delete a relationship by its ID.
    async fn delete_relationship(&self, id: &EntityId) -> Result<()>;

    /// Get all relationships where the given entity is source or target.
    async fn get_entity_relationships(&self, entity_id: &EntityId) -> Result<Vec<Relationship>>;

    /// Get all entities of a given type.
    async fn get_entities_by_type(&self, entity_type: &EntityType) -> Result<Vec<Entity>>;

    /// Search entities by title or description (full-text).
    async fn search_entities(&self, query: &str) -> Result<Vec<Entity>>;

    /// Get the total number of entities in the graph.
    async fn count_entities(&self) -> Result<u64>;

    /// Get the total number of relationships in the graph.
    async fn count_relationships(&self) -> Result<u64>;
}

#[cfg(test)]
mod tests {
    // GraphStore is a trait — concrete implementations tested in storage/sqlite/
}
