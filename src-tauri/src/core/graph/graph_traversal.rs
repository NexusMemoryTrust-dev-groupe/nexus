use async_trait::async_trait;

use crate::core::entity_id::EntityId;
use crate::core::graph::entity::Entity;
use crate::core::graph::relationship::Relationship;
use crate::core::result::Result;

/// Result of a neighborhood query around an entity.
#[derive(Debug, Clone)]
pub struct GraphNeighborhood {
    pub center: Entity,
    pub entities: Vec<Entity>,
    pub relationships: Vec<Relationship>,
}

/// A subgraph — a subset of entities and their relationships.
#[derive(Debug, Clone)]
pub struct SubGraph {
    pub entities: Vec<Entity>,
    pub relationships: Vec<Relationship>,
}

/// Graph traversal operations — neighbors, distance, paths, subgraphs.
#[async_trait]
pub trait GraphTraversal: Send + Sync {
    /// Get the neighborhood of an entity up to a given depth (BFS).
    async fn get_neighbors(&self, entity_id: &EntityId, depth: u32) -> Result<GraphNeighborhood>;

    /// Get the shortest path distance between two entities.
    /// Returns None if no path exists.
    async fn get_distance(&self, from: &EntityId, to: &EntityId) -> Result<Option<u32>>;

    /// Find a path between two entities, limited by max_depth.
    /// Returns None if no path exists within the depth limit.
    async fn find_path(
        &self,
        from: &EntityId,
        to: &EntityId,
        max_depth: u32,
    ) -> Result<Option<Vec<EntityId>>>;

    /// Get a subgraph centered on an entity within a given radius.
    async fn get_subgraph(&self, entity_id: &EntityId, radius: u32) -> Result<SubGraph>;
}

#[cfg(test)]
mod tests {
    // GraphTraversal is a trait — concrete implementations tested in storage/sqlite/
}
