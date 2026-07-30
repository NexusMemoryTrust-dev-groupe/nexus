use crate::core::context::context_package::UserIntent;
use crate::core::entity_id::EntityId;
use crate::core::graph::entity::Entity;
use crate::core::graph::graph_store::GraphStore;
use crate::core::result::Result;

/// Seeds the context builder with initial entities from the graph.
pub struct GraphSeeder<G: GraphStore> {
    pub graph_store: G,
}

impl<G: GraphStore> GraphSeeder<G> {
    pub fn new(graph_store: G) -> Self {
        Self { graph_store }
    }

    /// Find entities matching the intent query.
    pub async fn seed(&self, intent: &UserIntent) -> Result<Vec<Entity>> {
        let entities = self.graph_store.search_entities(&intent.query).await?;
        Ok(entities)
    }

    /// Get an entity and its 1-hop neighbors.
    pub async fn seed_entity(&self, entity_id: &EntityId) -> Result<Vec<Entity>> {
        if let Some(entity) = self.graph_store.get_entity(entity_id).await? {
            let mut result = vec![entity];
            let relationships = self.graph_store.get_entity_relationships(entity_id).await?;
            for rel in relationships {
                let neighbor_id = if rel.source_entity_id == *entity_id {
                    &rel.target_entity_id
                } else {
                    &rel.source_entity_id
                };
                if let Ok(Some(neighbor)) = self.graph_store.get_entity(neighbor_id).await {
                    result.push(neighbor);
                }
            }
            Ok(result)
        } else {
            Ok(vec![])
        }
    }
}

#[cfg(test)]
mod tests {
    // GraphSeeder tests require a GraphStore mock — tested via integration with SQLite
}
