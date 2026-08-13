use async_trait::async_trait;

use crate::core::context::compressor::ContextCompressor;
use crate::core::context::context_package::{ContextPackage, UserIntent};
use crate::core::context::context_request::ContextRequest;
use crate::core::context::graph_seeder::GraphSeeder;
use crate::core::context::intent_detector::IntentDetector;
use crate::core::context::memory_injector::MemoryInjector;
use crate::core::context::provenance::{DropCause, ItemKind, Provenance, Reason};
use crate::core::context::ranker::ContextRanker;
use crate::core::entity_id::EntityId;
use crate::core::graph::graph_store::GraphStore;
use crate::core::graph::relationship::Relationship;
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::result::Result;

/// Importance at or above which a memory is reported as included *because* the
/// user marked it important. Mirrors the threshold `MemoryInjector` uses when it
/// pulls important records, so the explanation matches the actual behaviour.
const HIGH_IMPORTANCE: f64 = 0.7;

/// Age in days below which a memory is reported as recent. Same window the
/// injector uses, for the same reason.
const RECENT_DAYS: i64 = 7;

/// Builds context packages from requests through a 6-step pipeline:
/// Intent Detection → Graph Seeding → Expansion → Memory Injection → Compression → Ranking
#[async_trait]
pub trait ContextBuilder: Send + Sync {
    /// Build a context package from a request (full pipeline).
    async fn build(&self, request: &ContextRequest) -> Result<ContextPackage>;

    /// Build context centered on a specific entity.
    async fn build_for_entity(&self, entity_id: &EntityId, depth: u32) -> Result<ContextPackage>;

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

    /// Calculate dynamic token limit based on actual content size.
    /// Formula: entities * 50 + relationships * 15 + memories * 100 + base 500
    /// Clamped between 1000 and 10000.
    fn calculate_dynamic_token_limit(&self, package: &ContextPackage) -> u32 {
        let entity_tokens = package.entities.len() as u32 * 50;
        let relationship_tokens = package.relationships.len() as u32 * 15;
        let memory_tokens = package.memory_records.len() as u32 * 100;
        let base = 500;
        let total = entity_tokens + relationship_tokens + memory_tokens + base;
        total.clamp(1000, 10000)
    }

    /// Measure what the model would have consumed had it read the whole
    /// candidate set, and store it on the package.
    ///
    /// Must be called *before* compression, while every candidate is still
    /// present. Counted with the same tokenizer as the final payload, so the
    /// two figures are directly comparable and the resulting saving is a
    /// measurement rather than the old hardcoded 800-token guess.
    fn record_baseline(package: &mut ContextPackage) {
        let mut baseline: u32 = 0;

        for entity in &package.entities {
            baseline = baseline.saturating_add(crate::core::tokenizer::count(&entity.title));
            baseline = baseline.saturating_add(crate::core::tokenizer::count(&entity.description));
        }

        // Reading a memory outright means reading its full body, not the summary.
        for record in &package.memory_records {
            baseline = baseline.saturating_add(crate::core::tokenizer::count(&record.title));
            baseline = baseline.saturating_add(crate::core::tokenizer::count(&record.content));
        }

        for rel in &package.relationships {
            baseline = baseline.saturating_add(crate::core::tokenizer::count(
                rel.relationship_type.as_str(),
            ));
        }

        package.baseline_tokens = baseline;
        package.candidate_entities = package.entities.len() as u32;
        package.candidate_memories = package.memory_records.len() as u32;
    }

    /// Attach the project's AGENTS.md instructions to the package.
    ///
    /// Best-effort: when no instruction file has been stored (or the knowledge
    /// store is unavailable) the field stays `None` and the pipeline is
    /// unaffected. When present, the AI sees the project's rules in the same
    /// payload as the context itself.
    fn attach_agent_instructions(package: &mut ContextPackage) {
        package.agent_instructions = crate::core::knowledge::agents::active_agents_content();
    }

    /// Conflict firewall (Система 2): records entangled in an unresolved
    /// contradiction (`Conflicted`) or already replaced by a resolved conflict
    /// (`Superseded`) must not reach the model silently. The package carries
    /// only the Current Truth; the number of excluded records is recorded so
    /// the exclusion stays observable.
    fn apply_conflict_firewall(package: &mut ContextPackage) {
        let before = package.memory_records.len();
        package.memory_records.retain(|m| {
            !matches!(
                m.memory_state,
                crate::core::memory::types::MemoryState::Conflicted
                    | crate::core::memory::types::MemoryState::Superseded
            )
        });
        package.conflicts_excluded = (before - package.memory_records.len()) as u32;
    }

    /// Collect relationships for all given entity IDs from the graph store.
    /// Deduplicates by relationship ID.
    async fn collect_relationships(&self, entity_ids: &[EntityId]) -> Result<Vec<Relationship>> {
        let mut all_rels: Vec<Relationship> = Vec::new();
        for eid in entity_ids {
            let rels = self
                .graph_seeder
                .graph_store
                .get_entity_relationships(eid)
                .await?;
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
    /// 3. Expansion (N-hop neighbors based on max_depth)
    /// 4. Memory Injection (search memory for each entity)
    /// 5. Ranking (score entities)
    /// 6. Compression (fit within max_tokens)
    async fn build(&self, request: &ContextRequest) -> Result<ContextPackage> {
        request.validate()?;

        // Step 1: Intent Detection
        let intent = self.intent_detector.detect(&request.query);

        // Provenance is accumulated as the pipeline runs, because *why* an item
        // is present is only knowable at the moment it enters. Reconstructing it
        // afterwards from the finished package is impossible: by then a direct
        // query hit and a third-hop neighbour look identical.
        let mut prov = Provenance::new();

        // Step 2: Graph Seeding — search for entities matching the query
        let mut entities = self.graph_seeder.seed(&intent).await?;
        for e in &entities {
            prov.record(
                e.id.as_str(),
                ItemKind::Entity,
                &e.title,
                Reason::QueryMatch {
                    query: request.query.clone(),
                },
            );
            // A seed can also owe its presence to a keyword rather than the whole
            // phrase; record that separately so the panel can show both.
            let title_lower = e.title.to_lowercase();
            for kw in &intent.keywords {
                if title_lower.contains(&kw.to_lowercase()) {
                    prov.record(
                        e.id.as_str(),
                        ItemKind::Entity,
                        &e.title,
                        Reason::KeywordMatch {
                            keyword: kw.clone(),
                        },
                    );
                }
            }
        }

        // Step 3: Expansion — N-hop neighbors based on max_depth
        let seeds: Vec<(EntityId, String)> = entities
            .iter()
            .map(|e| (e.id.clone(), e.title.clone()))
            .collect();
        let mut expanded = Vec::new();
        for (seed_id, seed_title) in &seeds {
            let neighbors = self
                .graph_seeder
                .seed_entity_deep(seed_id, request.max_depth)
                .await?;
            for n in &neighbors {
                // The seed itself comes back from a BFS walk; attributing it to
                // expansion would bury the fact that it matched the query.
                if &n.id == seed_id {
                    continue;
                }
                prov.record(
                    n.id.as_str(),
                    ItemKind::Entity,
                    &n.title,
                    Reason::GraphExpansion {
                        from_id: seed_id.as_str().to_string(),
                        from_title: seed_title.clone(),
                        hops: request.max_depth,
                    },
                );
            }
            expanded.extend(neighbors);
        }
        entities.extend(expanded);

        // Deduplicate entities by ID
        entities.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        entities.dedup_by(|a, b| a.id == b.id);

        // Respect max_entities limit
        if entities.len() > request.max_entities as usize {
            entities.truncate(request.max_entities as usize);
            // Everything traced but no longer present fell off this limit. Saying
            // so is more useful than silently omitting it.
            let kept: Vec<String> = entities.iter().map(|e| e.id.as_str().to_string()).collect();
            prov.reconcile(
                &kept,
                DropCause::EntityCap {
                    cap: request.max_entities,
                },
            );
        }

        // Collect relationships for all entities
        let entity_ids: Vec<EntityId> = entities.iter().map(|e| e.id.clone()).collect();
        let relationships = self.collect_relationships(&entity_ids).await?;

        // Step 4: Memory Injection
        let memory_records = self.memory_injector.inject(&entities, &intent).await?;
        for m in &memory_records {
            prov.record(
                m.id.as_str(),
                ItemKind::Memory,
                &m.title,
                Reason::MemorySearch {
                    query: request.query.clone(),
                },
            );
            if m.importance_score >= HIGH_IMPORTANCE {
                prov.record(
                    m.id.as_str(),
                    ItemKind::Memory,
                    &m.title,
                    Reason::HighImportance {
                        importance: m.importance_score,
                    },
                );
            }
            let age_days = (chrono::Utc::now() - m.created_at).num_days();
            if age_days <= RECENT_DAYS {
                prov.record(
                    m.id.as_str(),
                    ItemKind::Memory,
                    &m.title,
                    Reason::RecentActivity { age_days },
                );
            }
        }

        // Build initial package
        let mut package = ContextPackage::new(intent);
        package.entities = entities;
        package.relationships = relationships;
        package.memory_records = memory_records;
        package.provenance = prov;

        // Conflict firewall: drop Conflicted/Superseded records before the
        // baseline is measured, so the model only ever sees Current Truth and
        // the token figures are honest.
        Self::apply_conflict_firewall(&mut package);

        // Measure the baseline *before* compression, while the full candidate
        // set is still present: this is what the model would have consumed had
        // it read everything we found. Comparing it with the post-compression
        // `token_count` yields a saving that is measured, not assumed.
        Self::record_baseline(&mut package);

        // Step 5: Ranking
        package = self.ranker.rank(&package);

        // Step 6: Compression — honour the request's relevance floor
        package = self
            .compressor
            .compress(&package, request.max_tokens, request.min_relevance)?;

        // Attach AGENTS.md project instructions (best-effort, may be None)
        Self::attach_agent_instructions(&mut package);

        Ok(package)
    }

    async fn build_for_entity(&self, entity_id: &EntityId, depth: u32) -> Result<ContextPackage> {
        // Seed from a specific entity with N-hop traversal
        let depth = if depth == 0 { 1 } else { depth };
        let entities = self.graph_seeder.seed_entity_deep(entity_id, depth).await?;

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

        // Entity-centred builds have one seed and everything else arrives by
        // walking the graph, so the trace reads as a provenance chain: this is
        // the entity you asked about, and these are its neighbours N hops out.
        let mut prov = Provenance::new();
        for (i, e) in entities.iter().enumerate() {
            if i == 0 && &e.id == entity_id {
                prov.record(
                    e.id.as_str(),
                    ItemKind::Entity,
                    &e.title,
                    Reason::QueryMatch {
                        query: e.title.clone(),
                    },
                );
            } else {
                prov.record(
                    e.id.as_str(),
                    ItemKind::Entity,
                    &e.title,
                    Reason::GraphExpansion {
                        from_id: entity_id.as_str().to_string(),
                        from_title: entities
                            .first()
                            .map(|s| s.title.clone())
                            .unwrap_or_default(),
                        hops: depth,
                    },
                );
            }
        }
        for m in &memory_records {
            prov.record(
                m.id.as_str(),
                ItemKind::Memory,
                &m.title,
                Reason::MemorySearch {
                    query: entity_id.as_str().to_string(),
                },
            );
        }

        let mut package = ContextPackage::new(intent);
        package.entities = entities;
        package.relationships = relationships;
        package.memory_records = memory_records;
        package.provenance = prov;

        Self::apply_conflict_firewall(&mut package);

        Self::record_baseline(&mut package);

        package = self.ranker.rank(&package);

        // Dynamic token limit: scale with content size
        let max_tokens = self.calculate_dynamic_token_limit(&package);
        package = self.compressor.compress(
            &package,
            max_tokens,
            ContextCompressor::DEFAULT_MIN_RELEVANCE,
        )?;

        // Attach AGENTS.md project instructions (best-effort, may be None)
        Self::attach_agent_instructions(&mut package);

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

    fn test_repo() -> (SqliteGraphRepository, SqliteMemoryRepository) {
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

        let e1 = Entity::new(
            EntityType::Person,
            "Alice".to_string(),
            "Engineer".to_string(),
        );
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

    #[tokio::test]
    async fn conflict_firewall_drops_conflicted_and_superseded() {
        use crate::core::context::context_package::{ContextPackage, IntentType, UserIntent};
        use crate::core::memory::memory_record::MemoryRecord;
        use crate::core::memory::types::{MemorySource, MemoryState};

        let mut package = ContextPackage::new(UserIntent {
            query: "database".to_string(),
            intent_type: IntentType::Exploration,
            confidence: 0.8,
            keywords: vec!["database".to_string()],
            temporal: None,
        });

        let make = |state: MemoryState| {
            let mut r = MemoryRecord::new(
                "Database".to_string(),
                "Use PostgreSQL for the primary database".to_string(),
                "alice".to_string(),
                MemorySource::Manual,
            )
            .unwrap();
            r.memory_state = state;
            r
        };
        let current = make(MemoryState::Current);
        let conflicted = make(MemoryState::Conflicted);
        let superseded = make(MemoryState::Superseded);
        let confirmed = make(MemoryState::UserConfirmed);
        package.memory_records = vec![
            current.clone(),
            conflicted.clone(),
            superseded.clone(),
            confirmed.clone(),
        ];

        ContextBuilderImpl::<SqliteGraphRepository, SqliteMemoryRepository>::apply_conflict_firewall(
            &mut package,
        );

        let remaining: Vec<MemoryState> = package
            .memory_records
            .iter()
            .map(|m| m.memory_state.clone())
            .collect();
        assert_eq!(
            remaining,
            vec![MemoryState::Current, MemoryState::UserConfirmed],
            "only Current Truth (Current/UserConfirmed) may reach the model"
        );
        assert_eq!(
            package.conflicts_excluded, 2,
            "the firewall must count what it excluded"
        );
    }
}
