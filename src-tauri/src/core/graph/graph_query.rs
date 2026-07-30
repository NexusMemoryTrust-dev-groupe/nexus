use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::core::entity_id::EntityId;
use crate::core::graph::entity::Entity;
use crate::core::graph::entity_types::EntityType;
use crate::core::graph::relationship::Relationship;
use crate::core::graph::relationship_types::RelationshipType;
use crate::core::result::Result;

/// Parameters for a graph query.
#[derive(Debug, Clone)]
pub struct GraphQueryRequest {
    pub entity_type: Option<EntityType>,
    pub relationship_type: Option<RelationshipType>,
    pub min_weight: Option<f64>,
    pub limit: u32,
}

impl Default for GraphQueryRequest {
    fn default() -> Self {
        Self {
            entity_type: None,
            relationship_type: None,
            min_weight: None,
            limit: 100,
        }
    }
}

/// Result of a graph query.
#[derive(Debug, Clone)]
pub struct GraphQueryResult {
    pub entities: Vec<Entity>,
    pub relationships: Vec<Relationship>,
    pub score: f64,
}

/// A point on the entity timeline — an entity and when it was last changed.
#[derive(Debug, Clone)]
pub struct TimelineEvent {
    pub entity: Entity,
    pub relationship: Option<Relationship>,
    pub timestamp: DateTime<Utc>,
}

/// Graph query operations — filtered search, density, timeline.
#[async_trait]
pub trait GraphQuery: Send + Sync {
    /// Execute a filtered query on the graph.
    async fn query(&self, query: &GraphQueryRequest) -> Result<GraphQueryResult>;

    /// Calculate knowledge density for an entity — ratio of actual connections
    /// to possible connections within its neighborhood.
    async fn get_knowledge_density(
        &self,
        entity_id: &EntityId,
    ) -> Result<f64>;

    /// Get the timeline of changes for an entity.
    async fn get_timeline(
        &self,
        entity_id: &EntityId,
    ) -> Result<Vec<TimelineEvent>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_request_default() {
        let req = GraphQueryRequest::default();
        assert!(req.entity_type.is_none());
        assert!(req.relationship_type.is_none());
        assert!(req.min_weight.is_none());
        assert_eq!(req.limit, 100);
    }
}
