use crate::core::entity_id::EntityId;
use crate::core::graph::entity::{Entity, EntityStatus};
use crate::core::graph::entity_identity::EntityIdentityService;
use crate::core::graph::entity_types::EntityType;
use crate::core::graph::graph_query::{GraphQuery, GraphQueryRequest, GraphQueryResult, TimelineEvent};
use crate::core::graph::graph_store::GraphStore;
use crate::core::graph::graph_traversal::{GraphNeighborhood, GraphTraversal, SubGraph};
use crate::core::graph::relationship::Relationship;
use crate::core::graph::relationship_types::RelationshipType;
use crate::core::result::Result;

/// Orchestrator for graph operations.
/// Delegates to GraphStore, GraphTraversal, GraphQuery, EntityIdentityService.
pub struct GraphService<
    S: GraphStore,
    T: GraphTraversal,
    Q: GraphQuery,
    I: EntityIdentityService,
> {
    store: S,
    traversal: T,
    query: Q,
    identity: I,
}

impl<S: GraphStore, T: GraphTraversal, Q: GraphQuery, I: EntityIdentityService>
    GraphService<S, T, Q, I>
{
    pub fn new(store: S, traversal: T, query: Q, identity: I) -> Self {
        Self {
            store,
            traversal,
            query,
            identity,
        }
    }

    /// Create a new entity in the graph.
    pub async fn create_entity(
        &self,
        entity_type: EntityType,
        title: String,
        description: String,
    ) -> Result<EntityId> {
        let entity = Entity::new(entity_type, title, description);
        entity.validate()?;
        self.store.add_entity(&entity).await
    }

    /// Get an entity by ID.
    pub async fn get_entity(&self, id: &EntityId) -> Result<Option<Entity>> {
        self.store.get_entity(id).await
    }

    /// Update an existing entity.
    pub async fn update_entity(&self, entity: &Entity) -> Result<()> {
        entity.validate()?;
        self.store.update_entity(entity).await
    }

    /// Delete an entity by ID.
    pub async fn delete_entity(&self, id: &EntityId) -> Result<()> {
        self.store.delete_entity(id).await
    }

    /// Create a relationship between two entities.
    pub async fn link_entities(
        &self,
        source: EntityId,
        target: EntityId,
        rel_type: RelationshipType,
        weight: f64,
    ) -> Result<EntityId> {
        let relationship = Relationship::new(source, target, rel_type, weight)?;
        self.store.add_relationship(&relationship).await
    }

    /// Get the context neighborhood of an entity (depth 2).
    pub async fn get_context(
        &self,
        entity_id: &EntityId,
    ) -> Result<GraphNeighborhood> {
        self.traversal.get_neighbors(entity_id, 2).await
    }

    /// Get the shortest path between two entities.
    pub async fn find_path(
        &self,
        from: &EntityId,
        to: &EntityId,
        max_depth: u32,
    ) -> Result<Option<Vec<EntityId>>> {
        self.traversal.find_path(from, to, max_depth).await
    }

    /// Search entities by query string.
    pub async fn search(&self, query_str: &str) -> Result<Vec<Entity>> {
        self.store.search_entities(query_str).await
    }

    /// Execute a filtered graph query.
    pub async fn query(&self, request: &GraphQueryRequest) -> Result<GraphQueryResult> {
        self.query.query(request).await
    }

    /// Get knowledge density for an entity.
    pub async fn get_knowledge_density(
        &self,
        entity_id: &EntityId,
    ) -> Result<f64> {
        self.query.get_knowledge_density(entity_id).await
    }

    /// Get the timeline of changes for an entity.
    pub async fn get_timeline(
        &self,
        entity_id: &EntityId,
    ) -> Result<Vec<TimelineEvent>> {
        self.query.get_timeline(entity_id).await
    }

    /// Get all relationships for an entity.
    pub async fn get_entity_relationships(
        &self,
        entity_id: &EntityId,
    ) -> Result<Vec<Relationship>> {
        self.store.get_entity_relationships(entity_id).await
    }

    /// Count entities in the graph.
    pub async fn count_entities(&self) -> Result<u64> {
        self.store.count_entities().await
    }

    /// Count relationships in the graph.
    pub async fn count_relationships(&self) -> Result<u64> {
        self.store.count_relationships().await
    }
}

#[cfg(test)]
mod tests {
    // GraphService tests require mock implementations of all 4 traits.
    // Tested via integration tests with SQLite in storage/sqlite/graph_repository.rs
}
