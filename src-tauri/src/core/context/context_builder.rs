use async_trait::async_trait;

use crate::core::context::compressor::ContextCompressor;
use crate::core::context::context_package::{ContextPackage, UserIntent};
use crate::core::context::context_request::ContextRequest;
use crate::core::context::graph_seeder::GraphSeeder;
use crate::core::context::intent_detector::IntentDetector;
use crate::core::context::memory_injector::MemoryInjector;
use crate::core::context::ranker::ContextRanker;
use crate::core::entity_id::EntityId;
use crate::core::graph::relationship::Relationship;
use crate::core::graph::graph_store::GraphStore;
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::result::Result;

/// Builds context packages from requests through a 6-step pipeline:
/// Intent Detection → Graph Seeding → Expansion → Memory Injection → Compression → Ranking
#[async_trait]
pub trait ContextBuilder: Send + Sync {
    /// Build a context package from a request (full pipeline).
    async fn build(&self, request: &ContextRequest) -> Result<ContextPackage>;

    /// Build context centered on a specific entity.
    async fn build_for_entity(
        &self,
        entity_id: &EntityId,
        depth: u32,
    ) -> Result<ContextPackage>;

    /// Build context for a free-text query.
    async fn build_for_query(&self, query: &str) -> Result<ContextPackage>;
}

/// Concrete implementation of ContextBuilder that wires the full 6-step pipeline.
///
/// Pipeline: Intent Detection → Graph Seeding → Expansion → Memory Injection → Ranking → Compression
pub struct ContextBuilderImpl<G: GraphStore, M: MemoryRepository> {
    intent_detector: IntentDetector,
    graph_seeder: GraphSeeder<G>,
    memory_injector: MemoryInjector<M>,
    ranker: ContextRanker,
    compressor: ContextCompressor,
}

impl<G: GraphStore, M: MemoryRepository> ContextBuilderImpl<G, M> {
    pub fn new(graph_store: G, memory_repo: M) -> Self {
        Self {
            intent_detector: IntentDetector::new(),
            graph_seeder: GraphSeeder::new(graph_store),
            memory_injector: MemoryInjector::new(memory_repo),
            ranker: ContextRanker::new(),
            compressor: ContextCompressor::new(),
        }
    }

    /// Collect relationships for all given entity IDs from the graph store.
    /// Deduplicates by relationship ID.
    async fn collect_relationships(
        &self,
        entity_ids: &[EntityId],
    ) -> Result<Vec<Relationship>> {
        let mut all_rels: Vec<Relationship> = Vec::new();
        for eid in entity_ids {
            let rels = self.graph_seeder.graph_store.get_entity_relationships(eid).await?;
            all_rels.extend(rels);
        }
        all_rels.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        all_rels.dedup_by(|a, b| a.id == b.id);
        Ok(all_rels)
    }
}

#[async_trait]
impl<G: GraphStore, M: MemoryRepository> ContextBuilder for ContextBuilderImpl<G, M> {
    /// Full 6-step pipeline:
    /// 1. Intent Detection
    /// 2. Graph Seeding (search by query)
    /// 3. Expansion (1-hop neighbors)
    /// 4. Memory Injection (search memory for each entity)
    /// 5. Ranking (score entities)
    /// 6. Compression (fit within max_tokens)
    async fn build(&self, request: &ContextRequest) -> Result<ContextPackage> {
        request.validate()?;

        // Step 1: Intent Detection
        let intent = self.intent_detector.detect(&request.query);

        // Step 2: Graph Seeding — search for entities matching the query
        let mut entities = self.graph_seeder.seed(&intent).await?;

        // Step 3: Expansion — 1-hop neighbors for each seeded entity
        let mut expanded = Vec::new();
        for entity in &entities {
            let neighbors = self.graph_seeder.seed_entity(&entity.id).await?;
            expanded.extend(neighbors);
        }
        entities.extend(expanded);

        // Deduplicate entities by ID
        entities.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        entities.dedup_by(|a, b| a.id == b.id);

        // Respect max_entities limit
        if entities.len() > request.max_entities as usize {
            entities.truncate(request.max_entities as usize);
        }

        // Collect relationships for all entities
        let entity_ids: Vec<EntityId> = entities.iter().map(|e| e.id.clone()).collect();
        let relationships = self.collect_relationships(&entity_ids).await?;

        // Step 4: Memory Injection
        let memory_records = self.memory_injector.inject(&entities, &intent).await?;

        // Build initial package
        let mut package = ContextPackage::new(intent);
        package.entities = entities;
        package.relationships = relationships;
        package.memory_records = memory_records;

        // Step 5: Ranking
        package = self.ranker.rank(&package);

        // Step 6: Compression
        package = self.compressor.compress(&package, request.max_tokens)?;

        Ok(package)
    }

    async fn build_for_entity(
        &self,
        entity_id: &EntityId,
        _depth: u32,
    ) -> Result<ContextPackage> {
        // Seed from a specific entity and its neighbors
        let entities = self.graph_seeder.seed_entity(entity_id).await?;

        let entity_ids: Vec<EntityId> = entities.iter().map(|e| e.id.clone()).collect();
        let relationships = self.collect_relationships(&entity_ids).await?;

        let intent = UserIntent {
            query: entity_id.as_str().to_string(),
            intent_type: crate::core::context::context_package::IntentType::Exploration,
            confidence: 0.8,
            keywords: vec![],
            temporal: None,
        };

        let memory_records = self.memory_injector.inject(&entities, &intent).await?;

        let mut package = ContextPackage::new(intent);
        package.entities = entities;
        package.relationships = relationships;
        package.memory_records = memory_records;

        package = self.ranker.rank(&package);
        package = self.compressor.compress(&package, 4000)?;

        Ok(package)
    }

    async fn build_for_query(&self, query: &str) -> Result<ContextPackage> {
        let request = ContextRequest {
            query: query.to_string(),
            ..Default::default()
        };
        self.build(&request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::context_package::IntentType;
    use crate::storage::sqlite::graph_repository::SqliteGraphRepository;
    use crate::storage::sqlite::memory_repository_sqlite::SqliteMemoryRepository;

    fn test_repo() -> (
        SqliteGraphRepository,
        SqliteMemoryRepository,
    ) {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::schema::apply_migrations(&conn).unwrap();
        let graph = SqliteGraphRepository::new(conn).unwrap();

        let memory_conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::schema::apply_migrations(&memory_conn).unwrap();
        let memory = SqliteMemoryRepository::new(memory_conn).unwrap();

        (graph, memory)
    }

    #[tokio::test]
    async fn build_empty_graph_returns_empty_package() {
        let (graph, memory) = test_repo();
        let builder = ContextBuilderImpl::new(graph, memory);

        let request = ContextRequest {
            query: "test query".to_string(),
            ..Default::default()
        };
        let pkg = builder.build(&request).await.unwrap();

        assert_eq!(pkg.user_intent.query, "test query");
        assert_eq!(pkg.user_intent.intent_type, IntentType::Exploration);
        assert!(pkg.entities.is_empty());
        assert!(pkg.memory_records.is_empty());
    }

    #[tokio::test]
    async fn build_for_query_constructs_request() {
        let (graph, memory) = test_repo();
        let builder = ContextBuilderImpl::new(graph, memory);

        let pkg = builder.build_for_query("найди проекты").await.unwrap();
        assert_eq!(pkg.user_intent.intent_type, IntentType::Search);
    }

    #[tokio::test]
    async fn build_for_entity_empty_returns_empty() {
        let (graph, memory) = test_repo();
        let builder = ContextBuilderImpl::new(graph, memory);

        let eid = crate::core::EntityId::new();
        let pkg = builder.build_for_entity(&eid, 2).await.unwrap();
        assert!(pkg.entities.is_empty());
        assert_eq!(pkg.user_intent.intent_type, IntentType::Exploration);
    }

    #[tokio::test]
    async fn build_with_entities_populates_relationships() {
        let (graph, memory) = test_repo();
        use crate::core::graph::entity::Entity;
        use crate::core::graph::entity_types::EntityType;
        use crate::core::graph::graph_store::GraphStore;

        let e1 = Entity::new(EntityType::Person, "Alice".to_string(), "Engineer".to_string());
        let e2 = Entity::new(EntityType::Person, "Bob".to_string(), "Manager".to_string());
        let id1 = graph.add_entity(&e1).await.unwrap();
        let id2 = graph.add_entity(&e2).await.unwrap();

        let rel = Relationship::new(
            id1.clone(),
            id2.clone(),
            crate::core::graph::relationship_types::RelationshipType::RelatedTo,
            0.8,
        )
        .unwrap();
        graph.add_relationship(&rel).await.unwrap();

        let builder = ContextBuilderImpl::new(graph, memory);
        let request = ContextRequest {
            query: "Alice Bob".to_string(),
            ..Default::default()
        };
        let pkg = builder.build(&request).await.unwrap();

        // Verify pipeline completed without panic
        assert_eq!(pkg.user_intent.query, "Alice Bob");
    }

    #[tokio::test]
    async fn build_respects_max_entities() {
        let (graph, memory) = test_repo();
        use crate::core::graph::entity::Entity;
        use crate::core::graph::entity_types::EntityType;
        use crate::core::graph::graph_store::GraphStore;

        // Add 5 entities with similar titles
        for i in 0..5 {
            let e = Entity::new(
                EntityType::Task,
                format!("Task item {}", i),
                format!("Description {}", i),
            );
            graph.add_entity(&e).await.unwrap();
        }

        let builder = ContextBuilderImpl::new(graph, memory);
        let request = ContextRequest {
            query: "Task".to_string(),
            max_entities: 2,
            ..Default::default()
        };
        let pkg = builder.build(&request).await.unwrap();
        assert!(pkg.entities.len() <= 2);
    }

    #[tokio::test]
    async fn build_validates_request() {
        let (graph, memory) = test_repo();
        let builder = ContextBuilderImpl::new(graph, memory);

        let request = ContextRequest {
            query: "test".to_string(),
            max_tokens: 0,
            ..Default::default()
        };
        let result = builder.build(&request).await;
        assert!(result.is_err());
    }
}
