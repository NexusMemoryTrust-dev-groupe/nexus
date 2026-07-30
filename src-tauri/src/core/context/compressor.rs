use crate::core::context::context_package::ContextPackage;

/// Compresses a context package to fit within token limits.
pub struct ContextCompressor;

impl ContextCompressor {
    pub fn new() -> Self {
        Self
    }

    /// Compress a context package to fit within max_tokens.
    /// Strategy: prune low-relevance → prune low-weight relationships → truncate if still too large.
    pub fn compress(&self, package: &ContextPackage, max_tokens: u32) -> Result<ContextPackage, crate::core::AppError> {
        let mut compressed = package.clone();

        // Step 1: Remove low-relevance items (entities + memories)
        compressed = self.prune_low_relevance(&compressed, 0.3);

        // Step 2: Remove low-weight relationships (< 0.3)
        compressed.relationships.retain(|r| r.weight >= 0.3);

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

        Ok(compressed)
    }

    /// Estimate token count for a context package.
    /// Counts: entities (title + desc), relationships (type + overhead), memories (title + content).
    pub fn calculate_token_count(&self, package: &ContextPackage) -> u32 {
        let mut count: u32 = 0;

        for entity in &package.entities {
            count += entity.title.len() as u32 / 4;
            count += entity.description.len() as u32 / 4;
        }

        // Relationships: each has type string + structural overhead (~8 tokens)
        for rel in &package.relationships {
            count += rel.relationship_type.as_str().len() as u32 / 4;
            count += 8;
        }

        for record in &package.memory_records {
            count += record.title.len() as u32 / 4;
            count += record.content.len() as u32 / 4;
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
        let compressed = c.compress(&pkg, 10).unwrap();
        assert!(c.calculate_token_count(&compressed) <= 10);
    }
}
