use crate::core::context::context_package::ContextPackage;
use crate::core::context::provenance::DropCause;

/// Compresses a context package to fit within token limits.
pub struct ContextCompressor;

impl ContextCompressor {
    pub fn new() -> Self {
        Self
    }

    /// Default relevance floor, used when a caller has no request-level setting.
    pub const DEFAULT_MIN_RELEVANCE: f64 = 0.3;

    /// Compress a context package to fit within max_tokens.
    /// Strategy: prune low-relevance → prune low-weight relationships → truncate if still too large.
    ///
    /// `min_relevance` comes from the request so the caller's setting is actually
    /// honoured — it used to be hardcoded to 0.3, which silently ignored
    /// `ContextRequest::min_relevance`.
    pub fn compress(
        &self,
        package: &ContextPackage,
        max_tokens: u32,
        min_relevance: f64,
    ) -> Result<ContextPackage, crate::core::AppError> {
        let mut compressed = package.clone();

        // Step 1: Remove low-relevance items (entities + memories)
        compressed = self.prune_low_relevance(&compressed, min_relevance);

        // Snapshot of what survived the relevance floor. Captured here rather
        // than recomputed later because steps 3-4 remove further items for a
        // different reason, and the two causes must not be confused.
        let pruned_ids: Vec<String> = compressed
            .entities
            .iter()
            .map(|e| e.id.to_string())
            .chain(compressed.memory_records.iter().map(|m| m.id.to_string()))
            .collect();

        // Record *why* each item fell to the relevance floor, carrying the score
        // that failed and the floor it failed against. A bare "pruned" label
        // would leave the user unable to tell a near miss from an outlier.
        for trace_id in package
            .entities
            .iter()
            .map(|e| e.id.to_string())
            .chain(package.memory_records.iter().map(|m| m.id.to_string()))
        {
            if pruned_ids.contains(&trace_id) {
                continue;
            }
            let score = package
                .relevance_scores
                .get(&trace_id)
                .copied()
                .unwrap_or(0.0);
            compressed.provenance.mark_dropped(
                &trace_id,
                DropCause::BelowRelevance { score, floor: min_relevance },
            );
        }

        // Step 2: Remove relationships weaker than the same relevance floor
        compressed.relationships.retain(|r| r.weight >= min_relevance);

        // Step 3: If still too large, progressively remove lowest-scoring entities
        while self.calculate_token_count(&compressed) > max_tokens && !compressed.entities.is_empty() {
            // Remove the last entity (lowest score due to descending sort)
            compressed.entities.pop();
            // Also remove relationships that reference the removed entity
            let remaining_ids: std::collections::HashSet<String> = compressed.entities.iter().map(|e| e.id.to_string()).collect();
            compressed.relationships.retain(|r| {
                remaining_ids.contains(r.source_entity_id.as_str()) && remaining_ids.contains(r.target_entity_id.as_str())
            });
        }

        // Step 4: If still too large, trim memory records by truncating content
        while self.calculate_token_count(&compressed) > max_tokens && !compressed.memory_records.is_empty() {
            compressed.memory_records.pop();
        }

        // Update token count
        compressed.token_count = self.calculate_token_count(&compressed);
        compressed.compressed_size = compressed.token_count * 4; // approximate bytes

        // Record what steps 3-4 removed. The relevance verdict was already
        // recorded above with the real failing score, and `reconcile` never
        // overwrites an earlier cause, so those items keep their own explanation
        // instead of being relabelled as budget casualties.
        let final_ids: Vec<String> = compressed
            .entities
            .iter()
            .map(|e| e.id.to_string())
            .chain(compressed.memory_records.iter().map(|m| m.id.to_string()))
            .collect();
        compressed
            .provenance
            .reconcile(&final_ids, DropCause::TokenBudget { limit: max_tokens });

        // Per-item token cost, so the panel can show what each item is worth.
        for e in &compressed.entities {
            let tokens = crate::core::tokenizer::count(&e.title)
                + crate::core::tokenizer::count(&e.description);
            compressed.provenance.set_tokens(&e.id.to_string(), tokens);
        }
        for m in &compressed.memory_records {
            let tokens = crate::core::tokenizer::count(&m.title)
                + crate::core::tokenizer::count(&m.content);
            compressed.provenance.set_tokens(&m.id.to_string(), tokens);
        }

        Ok(compressed)
    }

    /// Structural overhead per relationship: the surrounding JSON keys, braces
    /// and the two entity ids that a relationship costs once serialised.
    const RELATIONSHIP_OVERHEAD_TOKENS: u32 = 8;

    /// Count the tokens a context package costs.
    ///
    /// Uses the real BPE vocabulary via [`crate::core::tokenizer`]. The previous
    /// `len() / 4` heuristic undercounted Cyrillic by roughly half — a 2-byte
    /// character divided by 4 yields half a token per character — so Russian
    /// content silently overflowed the model's context window even though the
    /// package looked well inside the limit.
    pub fn calculate_token_count(&self, package: &ContextPackage) -> u32 {
        use crate::core::tokenizer;

        let mut count: u32 = 0;

        for entity in &package.entities {
            count = count.saturating_add(tokenizer::count(&entity.title));
            count = count.saturating_add(tokenizer::count(&entity.description));
        }

        for rel in &package.relationships {
            count = count.saturating_add(tokenizer::count(rel.relationship_type.as_str()));
            count = count.saturating_add(Self::RELATIONSHIP_OVERHEAD_TOKENS);
        }

        for record in &package.memory_records {
            count = count.saturating_add(tokenizer::count(&record.title));
            count = count.saturating_add(tokenizer::count(&record.content));
        }

        count
    }

    /// Remove entities and memory records below min_relevance.
    pub fn prune_low_relevance(&self, package: &ContextPackage, min_relevance: f64) -> ContextPackage {
        let mut pruned = package.clone();

        pruned.entities.retain(|e| {
            package
                .relevance_scores
                .get(&e.id.to_string())
                .map(|score| *score >= min_relevance)
                .unwrap_or(true)
        });

        pruned.memory_records.retain(|r| {
            package
                .relevance_scores
                .get(&r.id.to_string())
                .map(|score| *score >= min_relevance)
                .unwrap_or(true)
        });

        pruned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::context_package::{IntentType, UserIntent};
    use crate::core::graph::entity::Entity;
    use crate::core::graph::entity_types::EntityType;

    fn sample_package() -> ContextPackage {
        let mut pkg = ContextPackage::new(UserIntent {
            query: "test".to_string(),
            intent_type: IntentType::Search,
            confidence: 0.8,
            keywords: vec!["test".to_string()],
            temporal: None,
        });
        // Add some entities with varying relevance
        let e1 = Entity::new(EntityType::Person, "Alice".to_string(), "Engineer".to_string());
        let e2 = Entity::new(EntityType::Project, "Nexus".to_string(), "AI Memory OS".to_string());
        let e3 = Entity::new(EntityType::Task, "Task1".to_string(), "Do stuff".to_string());
        pkg.relevance_scores.insert(e1.id.to_string(), 0.9);
        pkg.relevance_scores.insert(e2.id.to_string(), 0.5);
        pkg.relevance_scores.insert(e3.id.to_string(), 0.1);
        pkg.entities = vec![e1, e2, e3];
        pkg
    }

    #[test]
    fn calculate_token_count_empty() {
        let c = ContextCompressor::new();
        let pkg = ContextPackage::new(UserIntent {
            query: "q".to_string(),
            intent_type: IntentType::Search,
            confidence: 0.8,
            keywords: vec![],
            temporal: None,
        });
        assert_eq!(c.calculate_token_count(&pkg), 0);
    }

    #[test]
    fn calculate_token_count_with_entities() {
        let c = ContextCompressor::new();
        let pkg = sample_package();
        let count = c.calculate_token_count(&pkg);
        assert!(count > 0);
    }

    #[test]
    fn prune_low_relevance() {
        let c = ContextCompressor::new();
        let pkg = sample_package();
        let pruned = c.prune_low_relevance(&pkg, 0.3);
        // e3 has relevance 0.1 < 0.3, should be pruned
        assert_eq!(pruned.entities.len(), 2);
    }

    #[test]
    fn compress_fits_within_tokens() {
        let c = ContextCompressor::new();
        let pkg = sample_package();
        let compressed = c
            .compress(&pkg, 10, ContextCompressor::DEFAULT_MIN_RELEVANCE)
            .unwrap();
        assert!(c.calculate_token_count(&compressed) <= 10);
    }

    #[test]
    fn compress_honours_min_relevance() {
        let c = ContextCompressor::new();
        let pkg = sample_package();

        // Scores in sample_package are 0.9 / 0.5 / 0.1.
        let lenient = c.compress(&pkg, 10_000, 0.0).unwrap();
        assert_eq!(lenient.entities.len(), 3, "floor of 0.0 keeps everything");

        let strict = c.compress(&pkg, 10_000, 0.6).unwrap();
        assert_eq!(strict.entities.len(), 1, "floor of 0.6 keeps only the 0.9 entity");
    }
}
