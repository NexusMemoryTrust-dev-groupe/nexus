use chrono::Utc;

use crate::core::context::context_package::{ContextPackage, UserIntent};
use crate::core::context::provenance::ScorePart;
use crate::core::context::semantic_search::HybridBreakdownHit;
use crate::core::entity_id::EntityId;
use crate::core::graph::graph_traversal::GraphTraversal;
use crate::core::result::Result;

/// Ranks entities and memory records by relevance to the user's intent.
/// Now with enhanced recency scoring and importance weighting.
pub struct ContextRanker;

impl Default for ContextRanker {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextRanker {
    pub fn new() -> Self {
        Self
    }

    /// Calculate and assign relevance scores, then sort by score.
    pub fn rank(&self, package: &ContextPackage) -> ContextPackage {
        let mut ranked = package.clone();

        // Score and rank entities.
        //
        // The breakdown is captured alongside the score so the "why is this in
        // my context?" panel can show the arithmetic instead of a bare number.
        // Collected first, then written, because `ranked` is borrowed here.
        let entity_scores: Vec<(String, f64, Vec<ScorePart>)> = ranked
            .entities
            .iter()
            .map(|entity| {
                let (score, parts) = self.score_entity_parts(entity, &ranked.user_intent);
                (entity.id.to_string(), score, parts)
            })
            .collect();

        for (id, score, parts) in entity_scores {
            ranked.relevance_scores.insert(id.clone(), score);
            ranked.provenance.set_score(&id, score, parts);
        }

        // Sort entities by score (descending)
        ranked.entities.sort_by(|a, b| {
            let score_a = ranked
                .relevance_scores
                .get(&a.id.to_string())
                .unwrap_or(&0.0);
            let score_b = ranked
                .relevance_scores
                .get(&b.id.to_string())
                .unwrap_or(&0.0);
            score_b
                .partial_cmp(score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Score and rank memory records — store scores in relevance_scores
        // so compressor can prune low-relevance memories
        let memory_scores: Vec<(String, f64, Vec<ScorePart>)> = ranked
            .memory_records
            .iter()
            .map(|memory| {
                let (score, parts) = self.score_memory_parts(memory, &ranked.user_intent);
                (memory.id.to_string(), score, parts)
            })
            .collect();

        for (id, score, parts) in memory_scores {
            ranked.relevance_scores.insert(id.clone(), score);
            ranked.provenance.set_score(&id, score, parts);
        }

        // Sort memory records by score (descending)
        ranked.memory_records.sort_by(|a, b| {
            let score_a = ranked
                .relevance_scores
                .get(&a.id.to_string())
                .unwrap_or(&0.0);
            let score_b = ranked
                .relevance_scores
                .get(&b.id.to_string())
                .unwrap_or(&0.0);
            score_b
                .partial_cmp(score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        ranked
    }

    /// Calculate relevance score for a single entity.
    pub fn calculate_score(
        &self,
        entity: &crate::core::graph::entity::Entity,
        intent: &UserIntent,
    ) -> f64 {
        self.score_entity_parts(entity, intent).0
    }

    /// Score an entity *and* return the breakdown that produced it.
    ///
    /// The plain score alone cannot answer "why is this in my context?" — a
    /// single 0.7 tells the user nothing. Returning the addends lets the UI show
    /// the arithmetic, which is what makes the ranking auditable rather than
    /// something to be taken on faith.
    pub fn score_entity_parts(
        &self,
        entity: &crate::core::graph::entity::Entity,
        intent: &UserIntent,
    ) -> (f64, Vec<ScorePart>) {
        let mut parts: Vec<ScorePart> = Vec::new();
        let mut score = 0.0;

        // Relevance to intent (keyword matching)
        let query_lower = intent.query.to_lowercase();
        let title_lower = entity.title.to_lowercase();
        if !query_lower.is_empty() && title_lower.contains(&query_lower) {
            score += 0.4;
            parts.push(ScorePart::new("titleMatch", 0.4));
        }

        // Keyword matching from extracted keywords
        for keyword in &intent.keywords {
            if title_lower.contains(&keyword.to_lowercase()) {
                score += 0.2;
                parts.push(ScorePart::new("keywordMatch", 0.2));
                break;
            }
        }

        // Importance from metadata
        if let Some(importance) = entity.metadata.get("importance")
            && let Some(val) = importance.as_f64()
        {
            let points = val * 0.3;
            score += points;
            parts.push(ScorePart::new("importance", points));
        }

        // Recency (newer = more relevant) with exponential decay
        let age_days = (Utc::now() - entity.updated_at).num_days().max(0) as f64;
        let recency_score = 1.0 / (1.0 + age_days / 7.0); // Faster decay
        let recency_points = recency_score * 0.2;
        score += recency_points;
        parts.push(ScorePart::new("recency", recency_points));

        // Base confidence
        score += 0.1;
        parts.push(ScorePart::new("base", 0.1));

        (score.min(1.0), parts)
    }

    /// Score a memory record *and* return the breakdown.
    pub fn score_memory_parts(
        &self,
        memory: &crate::core::memory::memory_record::MemoryRecord,
        intent: &UserIntent,
    ) -> (f64, Vec<ScorePart>) {
        let mut parts: Vec<ScorePart> = Vec::new();
        let mut score = 0.0;

        let query_lower = intent.query.to_lowercase();
        let title_lower = memory.title.to_lowercase();
        if !query_lower.is_empty() && title_lower.contains(&query_lower) {
            score += 0.3;
            parts.push(ScorePart::new("titleMatch", 0.3));
        }

        let content_lower = memory.content.to_lowercase();
        if !query_lower.is_empty() && content_lower.contains(&query_lower) {
            score += 0.2;
            parts.push(ScorePart::new("contentMatch", 0.2));
        }

        for keyword in &intent.keywords {
            if title_lower.contains(&keyword.to_lowercase())
                || content_lower.contains(&keyword.to_lowercase())
            {
                score += 0.2;
                parts.push(ScorePart::new("keywordMatch", 0.2));
                break;
            }
        }

        let importance_points = memory.importance_score * 0.2;
        score += importance_points;
        parts.push(ScorePart::new("importance", importance_points));

        let confidence_points = memory.confidence_score * 0.1;
        score += confidence_points;
        parts.push(ScorePart::new("confidence", confidence_points));

        let age_days = (Utc::now() - memory.created_at).num_days().max(0) as f64;
        let recency_points = (1.0 / (1.0 + age_days / 7.0)) * 0.1;
        score += recency_points;
        parts.push(ScorePart::new("recency", recency_points));

        (score.min(1.0), parts)
    }

    /// Calculate relevance score for a memory record.
    pub fn calculate_memory_score(
        &self,
        memory: &crate::core::memory::memory_record::MemoryRecord,
        intent: &UserIntent,
    ) -> f64 {
        self.score_memory_parts(memory, intent).0
    }
}

/// Decay applied to a graph neighbor's inherited seed score.
///
/// A neighbor is never ranked above the seed that pulled it in, but stays
/// high enough to surface related-but-otherwise-invisible files.
const GRAPH_EXPANSION_DECAY: f64 = 0.85;

/// How many seeds the graph expansion starts from (the hybrid top-N).
const GRAPH_EXPANSION_SEEDS: usize = 20;

/// Boost applied per additional graph link a candidate has to other top
/// candidates — a file that is *connected* to the retrieved set is more
/// likely to be part of the same feature than an isolated file.
const GRAPH_RERANK_LINK_BONUS: f64 = 0.04;

/// Multi-stage reranker (plan item 1.3): takes the hybrid top-K with its
/// per-channel breakdown, sharpens the ordering with graph evidence, and
/// returns a re-ranked `(entity_id, score)` list.
///
/// Stage 1 (hybrid retrieval) is done by the caller via
/// [`SemanticSearch::search_hybrid_breakdown`]; this type implements stage 2:
/// graph-aware reranking. Pure hybrid search can be fooled on homogeneous
/// corpora (hundreds of files scoring ≈0.6); the graph link density breaks
/// those ties by asking "which candidate is actually *connected* to the rest
/// of the retrieved set?"
pub struct HybridReranker;

impl HybridReranker {
    pub fn new() -> Self {
        Self
    }

    /// Rerank hybrid hits (each `(id, cosine, lexical, filename, total)`)
    /// using per-candidate graph link density as a tie-breaker.
    ///
    /// `neighbors_of` returns the entity ids reachable from a candidate in one
    /// step; candidates with more links to the other top candidates get a
    /// small additive bonus. The `total` channel from hybrid scoring is kept
    /// as the primary signal, so the reranker never *invents* relevance — it
    /// only re-orders near-ties.
    pub fn rerank<F>(&self, hits: &[HybridBreakdownHit], neighbors_of: F) -> Vec<(EntityId, f64)>
    where
        F: Fn(&EntityId) -> Vec<EntityId>,
    {
        let top_ids: Vec<EntityId> = hits.iter().map(|(id, _, _, _, _)| id.clone()).collect();
        let mut scored: Vec<(EntityId, f64)> = Vec::with_capacity(hits.len());

        for (id, _cos, _lex, _file, total) in hits {
            let links = neighbors_of(id)
                .iter()
                .filter(|n| top_ids.contains(*n))
                .count();
            scored.push((id.clone(), total + GRAPH_RERANK_LINK_BONUS * links as f64));
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
    }
}

impl Default for HybridReranker {
    fn default() -> Self {
        Self::new()
    }
}

/// Graph expansion (plan item 1.2): query → seeds → neighborhood → candidates.
///
/// Takes the hybrid top-N as seeds, walks one hop through the knowledge graph
/// for each seed, and merges the neighbors into the candidate list with a
/// decayed score (`seed_score * GRAPH_EXPANSION_DECAY`). A file that the
/// hybrid stage misses but that sits next to a strong seed (e.g. a component's
/// companion test or its style module) still becomes visible.
///
/// The graph is passed as [`GraphTraversal`]; the returned list is sorted by
/// score descending and truncated to `limit`.
pub async fn graph_expand(
    graph: &dyn GraphTraversal,
    seeds: &[(EntityId, f64)],
    limit: usize,
) -> Result<Vec<(EntityId, f64)>> {
    let mut candidates: Vec<(EntityId, f64)> = Vec::new();
    let mut best: std::collections::HashMap<String, f64> = std::collections::HashMap::new();

    let seed_count = seeds.len().min(GRAPH_EXPANSION_SEEDS);
    for (seed_id, seed_score) in seeds.iter().take(seed_count) {
        // The seed itself always participates.
        best.entry(seed_id.as_str().to_string())
            .and_modify(|s| *s = s.max(*seed_score))
            .or_insert(*seed_score);

        // One-hop neighborhood. A seed that has no graph node (e.g. a plain
        // memory record or a benchmark file) simply contributes no neighbors;
        // the seed itself stays a candidate.
        let Ok(neighborhood) = graph.get_neighbors(seed_id, 1).await else {
            continue;
        };
        for neighbor in &neighborhood.entities {
            if neighbor.id == *seed_id {
                continue;
            }
            let inherited = seed_score * GRAPH_EXPANSION_DECAY;
            best.entry(neighbor.id.as_str().to_string())
                .and_modify(|s| *s = s.max(inherited))
                .or_insert(inherited);
        }
    }

    for (id, score) in best {
        candidates.push((
            EntityId::parse(&id).unwrap_or_else(|_| EntityId::new()),
            score,
        ));
    }
    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    candidates.truncate(limit);
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::graph::entity::Entity;
    use crate::core::graph::entity_types::EntityType;

    fn sample_package() -> ContextPackage {
        let mut pkg = ContextPackage::new(UserIntent {
            query: "Alice".to_string(),
            intent_type: crate::core::context::context_package::IntentType::Search,
            confidence: 0.8,
            keywords: vec!["alice".to_string()],
            temporal: None,
        });
        let e1 = Entity::new(
            EntityType::Person,
            "Alice".to_string(),
            "Engineer".to_string(),
        );
        let e2 = Entity::new(EntityType::Person, "Bob".to_string(), "Manager".to_string());
        pkg.entities = vec![e1, e2];
        pkg
    }

    #[test]
    fn rank_sorts_by_score() {
        let r = ContextRanker::new();
        let pkg = sample_package();
        let ranked = r.rank(&pkg);
        assert_eq!(ranked.entities.len(), 2);
        // Alice should score higher (title contains query)
        let alice_score = ranked
            .relevance_scores
            .get(&ranked.entities[0].id.to_string())
            .unwrap();
        let bob_score = ranked
            .relevance_scores
            .get(&ranked.entities[1].id.to_string())
            .unwrap();
        assert!(alice_score >= bob_score);
    }

    #[test]
    fn calculate_score_keyword_match() {
        let r = ContextRanker::new();
        let entity = Entity::new(EntityType::Person, "Alice".to_string(), "desc".to_string());
        let intent = UserIntent {
            query: "Alice".to_string(),
            intent_type: crate::core::context::context_package::IntentType::Search,
            confidence: 0.8,
            keywords: vec!["alice".to_string()],
            temporal: None,
        };
        let score = r.calculate_score(&entity, &intent);
        assert!(score >= 0.4); // At least keyword match bonus
    }

    #[test]
    fn calculate_score_no_match() {
        let r = ContextRanker::new();
        let entity = Entity::new(EntityType::Person, "Bob".to_string(), "desc".to_string());
        let intent = UserIntent {
            query: "Alice".to_string(),
            intent_type: crate::core::context::context_package::IntentType::Search,
            confidence: 0.8,
            keywords: vec!["alice".to_string()],
            temporal: None,
        };
        let score = r.calculate_score(&entity, &intent);
        assert!(score < 0.5); // No keyword match
    }

    #[test]
    fn score_bounded_at_one() {
        let r = ContextRanker::new();
        let mut entity = Entity::new(EntityType::Person, "Alice".to_string(), "desc".to_string());
        entity
            .metadata
            .insert("importance".to_string(), serde_json::json!(1.0));
        let intent = UserIntent {
            query: "Alice".to_string(),
            intent_type: crate::core::context::context_package::IntentType::Search,
            confidence: 0.8,
            keywords: vec!["alice".to_string()],
            temporal: None,
        };
        let score = r.calculate_score(&entity, &intent);
        assert!(score <= 1.0);
    }

    #[test]
    fn calculate_memory_score_basic() {
        use crate::core::memory::memory_record::MemoryRecord;
        use crate::core::memory::types::MemorySource;

        let r = ContextRanker::new();
        let memory = MemoryRecord::new(
            "Alice Project".to_string(),
            "Alice is working on the project".to_string(),
            "author".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        let intent = UserIntent {
            query: "Alice".to_string(),
            intent_type: crate::core::context::context_package::IntentType::Search,
            confidence: 0.8,
            keywords: vec!["alice".to_string()],
            temporal: None,
        };
        let score = r.calculate_memory_score(&memory, &intent);
        assert!(score > 0.0); // Should have some score
    }

    // ── HybridReranker ─────────────────────────────────────────────────────

    #[test]
    fn reranker_keeps_hybrid_order_when_no_links() {
        let reranker = HybridReranker::new();
        let id_a = EntityId::new();
        let id_b = EntityId::new();
        let hits: Vec<HybridBreakdownHit> = vec![
            (id_a.clone(), 0.6, 0.2, 0.8, 0.9),
            (id_b.clone(), 0.5, 0.1, 0.2, 0.5),
        ];
        let ranked = reranker.rerank(&hits, |_| vec![]);
        assert_eq!(ranked[0].0, id_a);
        assert_eq!(ranked[1].0, id_b);
    }

    #[test]
    fn reranker_breaks_ties_with_graph_links() {
        let reranker = HybridReranker::new();
        let id_a = EntityId::new();
        let id_b = EntityId::new();
        let id_c = EntityId::new();
        let id_d = EntityId::new();
        // a and b are near-ties; a is connected to c and d, b to nobody.
        let hits: Vec<HybridBreakdownHit> = vec![
            (id_a.clone(), 0.6, 0.2, 0.5, 0.62),
            (id_b.clone(), 0.6, 0.2, 0.5, 0.62),
            (id_c.clone(), 0.5, 0.1, 0.2, 0.5),
            (id_d.clone(), 0.5, 0.1, 0.2, 0.5),
        ];
        let links: Vec<EntityId> = vec![id_c.clone(), id_d.clone()];
        let ranked = reranker.rerank(&hits, |id| if *id == id_a { links.clone() } else { vec![] });
        assert_eq!(ranked[0].0, id_a, "graph-linked candidate must win the tie");
    }

    #[test]
    fn reranker_returns_all_hits() {
        let reranker = HybridReranker::new();
        let ids: Vec<EntityId> = (0..4).map(|_| EntityId::new()).collect();
        let hits: Vec<HybridBreakdownHit> = ids
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, id)| (id, 0.5, 0.2, 0.4, 0.4 + 0.1 * i as f64))
            .collect();
        let ranked = reranker.rerank(&hits, |_| vec![]);
        assert_eq!(ranked.len(), 4);
    }

    // ── graph_expand ───────────────────────────────────────────────────────

    /// In-memory graph with a center entity and one neighbor.
    #[tokio::test]
    async fn graph_expand_adds_neighbors_with_decayed_score() {
        use crate::core::graph::GraphStore;
        use crate::core::graph::relationship::Relationship;
        use crate::core::graph::relationship_types::RelationshipType;
        use crate::storage::sqlite::SqliteGraphRepository;

        let conn = crate::db::open_connection_at(
            &std::env::temp_dir().join(format!("nexus-rank-test-{}.db", std::process::id())),
        )
        .expect("conn");
        crate::storage::sqlite::schema::apply_migrations(&conn).expect("migrate");
        let repo = SqliteGraphRepository::new(conn).expect("repo");
        let center = Entity::new(EntityType::Document, "Button.js".into(), "button".into());
        let neighbor = Entity::new(EntityType::Document, "Button.test.js".into(), "test".into());
        let other = Entity::new(EntityType::Document, "App.js".into(), "app".into());
        repo.add_entity(&center).await.unwrap();
        repo.add_entity(&neighbor).await.unwrap();
        repo.add_entity(&other).await.unwrap();
        let rel = Relationship::new(
            center.id.clone(),
            neighbor.id.clone(),
            RelationshipType::RelatedTo,
            0.9,
        )
        .unwrap();
        repo.add_relationship(&rel).await.unwrap();

        let seeds = vec![(center.id.clone(), 0.8)];
        let expanded = graph_expand(&repo, &seeds, 10).await.unwrap();
        assert!(expanded.len() >= 2, "neighbor must be pulled in");
        let neighbor_found = expanded.iter().any(|(id, _)| *id == neighbor.id);
        assert!(
            neighbor_found,
            "neighbor must appear in expanded candidates"
        );
        let neighbor_score = expanded
            .iter()
            .find(|(id, _)| *id == neighbor.id)
            .map(|(_, s)| *s)
            .unwrap();
        assert!(
            (neighbor_score - 0.8 * GRAPH_EXPANSION_DECAY).abs() < 1e-9,
            "neighbor score must be seed * decay"
        );
        // The unrelated entity is never expanded to.
        assert!(
            !expanded.iter().any(|(id, _)| *id == other.id),
            "unrelated entity must stay out"
        );
        let _ = std::fs::remove_file(
            std::env::temp_dir().join(format!("nexus-rank-test-{}.db", std::process::id())),
        );
    }

    #[tokio::test]
    async fn graph_expand_keeps_seed_even_without_graph() {
        use crate::storage::sqlite::SqliteGraphRepository;

        let conn = crate::db::open_connection_at(
            &std::env::temp_dir().join(format!("nexus-rank-test-empty-{}.db", std::process::id())),
        )
        .expect("conn");
        crate::storage::sqlite::schema::apply_migrations(&conn).expect("migrate");
        let repo = SqliteGraphRepository::new(conn).expect("repo");
        let seed_id = EntityId::new();
        let expanded = graph_expand(&repo, &[(seed_id.clone(), 0.7)], 10)
            .await
            .unwrap();
        assert_eq!(expanded.len(), 1);
        assert_eq!(expanded[0].0, seed_id);
        let _ = std::fs::remove_file(
            std::env::temp_dir().join(format!("nexus-rank-test-empty-{}.db", std::process::id())),
        );
    }
}
