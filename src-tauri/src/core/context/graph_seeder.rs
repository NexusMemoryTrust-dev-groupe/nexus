use std::collections::HashSet;

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
        // Prefer already-normalized keywords (stop words stripped, lowercased).
        // Fall back to the raw query — `search_entities` normalizes internally
        // too, so a stop-word-only query degrades to an empty result instead
        // of a false AND across meaningless words.
        let query = if intent.keywords.is_empty() {
            intent.query.clone()
        } else {
            intent.keywords.join(" ")
        };
        let entities = self.graph_store.search_entities(&query).await?;
        Ok(entities)
    }

    /// Get an entity and its 1-hop neighbors (legacy, kept for backward compat).
    pub async fn seed_entity(&self, entity_id: &EntityId) -> Result<Vec<Entity>> {
        self.seed_entity_deep(entity_id, 1).await
    }

    /// Get an entity and its N-hop neighbors using iterative BFS.
    /// `depth=1` means just the entity + direct neighbors.
    /// `depth=2` means entity + neighbors + their neighbors, etc.
    /// Uses iterative BFS to avoid stack overflow on large graphs with cycles.
    pub async fn seed_entity_deep(&self, entity_id: &EntityId, depth: u32) -> Result<Vec<Entity>> {
        if depth == 0 {
            return Ok(vec![]);
        }

        let mut visited = HashSet::new();
        let mut result = Vec::new();
        let mut current_level = vec![entity_id.clone()];

        // `depth + 1` levels: level 0 is the seed entity itself, so `depth` hops
        // of neighbours need one extra pass. Looping only `depth` times returned
        // just the seed for `depth = 1` and dropped the outermost ring entirely.
        for _ in 0..=depth {
            if current_level.is_empty() {
                break;
            }
            let mut next_level = Vec::new();

            for eid in &current_level {
                let id_str = eid.as_str().to_string();
                if visited.contains(&id_str) {
                    continue;
                }
                visited.insert(id_str);

                if let Some(entity) = self.graph_store.get_entity(eid).await? {
                    result.push(entity);
                }

                let relationships = self.graph_store.get_entity_relationships(eid).await?;
                for rel in relationships {
                    let neighbor_id = if rel.source_entity_id == *eid {
                        rel.target_entity_id.clone()
                    } else {
                        rel.source_entity_id.clone()
                    };
                    if !visited.contains(neighbor_id.as_str()) {
                        next_level.push(neighbor_id);
                    }
                }
            }

            current_level = next_level;
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    // GraphSeeder tests require a GraphStore mock — tested via integration with SQLite
}
