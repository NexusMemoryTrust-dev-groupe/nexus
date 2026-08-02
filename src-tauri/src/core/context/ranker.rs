use chrono::Utc;

use crate::core::context::context_package::{ContextPackage, UserIntent};
use crate::core::context::provenance::ScorePart;

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
}
