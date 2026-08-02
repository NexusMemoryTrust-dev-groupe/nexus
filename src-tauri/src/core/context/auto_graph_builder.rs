use crate::core::graph::entity::Entity;
use crate::core::graph::entity_types::EntityType;
use crate::core::graph::graph_store::GraphStore;
use crate::core::graph::relationship::Relationship;
use crate::core::graph::relationship_types::RelationshipType;
use crate::core::result::Result;

/// Automatic graph builder that extracts entities and relationships from text.
/// This is the intelligence layer that converts markdown/text into knowledge graph.
pub struct AutoGraphBuilder<G: GraphStore> {
    graph_store: G,
}

impl<G: GraphStore> AutoGraphBuilder<G> {
    pub fn new(graph_store: G) -> Self {
        Self { graph_store }
    }

    /// Parse markdown/text and extract entities and relationships.
    /// Returns (entities, relationships) found in the text.
    pub async fn parse_and_build(&self, text: &str) -> Result<(Vec<Entity>, Vec<Relationship>)> {
        let mut entities = Vec::new();
        let mut relationships = Vec::new();

        // Extract entities from text
        let extracted_entities = self.extract_entities(text);
        for (entity_type, title, description) in extracted_entities {
            // Check if entity already exists
            let existing = self.graph_store.search_entities(&title).await?;
            if let Some(e) = existing.into_iter().find(|e| e.title.to_lowercase() == title.to_lowercase()) {
                entities.push(e);
            } else {
                // Create new entity
                let entity = Entity::new(entity_type, title, description);
                let id = self.graph_store.add_entity(&entity).await?;
                let mut entity = entity;
                entity.id = id;
                entities.push(entity);
            }
        }

        // Extract relationships from text
        let extracted_relationships = self.extract_relationships(text, &entities);
        for (source_idx, target_idx, rel_type, weight) in extracted_relationships {
            if source_idx < entities.len() && target_idx < entities.len() {
                let source = &entities[source_idx];
                let target = &entities[target_idx];
                
                // Check if relationship already exists
                let existing_rels = self.graph_store.get_entity_relationships(&source.id).await?;
                let already_exists = existing_rels.iter().any(|r| {
                    (r.source_entity_id == source.id && r.target_entity_id == target.id) ||
                    (r.source_entity_id == target.id && r.target_entity_id == source.id)
                });

                if !already_exists {
                    if let Ok(rel) = Relationship::new(
                        source.id.clone(),
                        target.id.clone(),
                        rel_type,
                        weight,
                    ) {
                        if let Ok(rel_id) = self.graph_store.add_relationship(&rel).await {
                            let mut rel = rel;
                            rel.id = rel_id;
                            relationships.push(rel);
                        }
                    }
                }
            }
        }

        Ok((entities, relationships))
    }

    /// Extract entities from text using keyword patterns.
    fn extract_entities(&self, text: &str) -> Vec<(EntityType, String, String)> {
        let mut entities = Vec::new();
        let lines: Vec<&str> = text.lines().collect();

        for line in &lines {
            let trimmed = line.trim();
            
            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Extract project references
            if let Some(title) = self.extract_project_reference(trimmed) {
                entities.push((EntityType::Project, title, String::new()));
            }

            // Extract person references
            if let Some((name, role)) = self.extract_person_reference(trimmed) {
                entities.push((EntityType::Person, name, role));
            }

            // Extract technology references
            if let Some(tech) = self.extract_technology_reference(trimmed) {
                entities.push((EntityType::Technology, tech, String::new()));
            }

            // Extract task references
            if let Some((title, status)) = self.extract_task_reference(trimmed) {
                entities.push((EntityType::Task, title, format!("Status: {}", status)));
            }

            // Extract decision references
            if let Some(decision) = self.extract_decision_reference(trimmed) {
                entities.push((EntityType::Decision, decision, String::new()));
            }
        }

        // Deduplicate by title
        entities.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
        entities.dedup_by(|a, b| a.1.to_lowercase() == b.1.to_lowercase());

        entities
    }

    /// Extract project references from text.
    fn extract_project_reference(&self, text: &str) -> Option<String> {
        let lower = text.to_lowercase();
        
        // Pattern: "project: ..." or "проект: ..."
        if lower.starts_with("project:") || lower.starts_with("проект:") {
            let title = text.splitn(2, ':').nth(1)?.trim().to_string();
            if !title.is_empty() {
                return Some(title);
            }
        }

        // Pattern: "## Project Name" (markdown heading)
        if text.starts_with("## ") {
            let title = text[3..].trim().to_string();
            if title.to_lowercase().contains("project") || title.to_lowercase().contains("проект") {
                return Some(title);
            }
        }

        None
    }

    /// Extract person references from text.
    fn extract_person_reference(&self, text: &str) -> Option<(String, String)> {
        let lower = text.to_lowercase();
        
        // Pattern: "person: Name (Role)" or "person: Name"
        if lower.starts_with("person:") || lower.starts_with("человек:") {
            let rest = text.splitn(2, ':').nth(1)?.trim().to_string();
            if let Some((name, role)) = rest.split_once('(') {
                let name = name.trim().to_string();
                let role = role.trim_end_matches(')').trim().to_string();
                return Some((name, role));
            }
            return Some((rest, String::new()));
        }

        // Pattern: "@Name" mention
        if let Some(name) = text.find('@') {
            let after_at = &text[name + 1..];
            let word: String = after_at.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            if !word.is_empty() {
                return Some((word, String::new()));
            }
        }

        None
    }

    /// Extract technology references from text.
    fn extract_technology_reference(&self, text: &str) -> Option<String> {
        let lower = text.to_lowercase();
        
        // Pattern: "tech: ..." or "технология: ..."
        if lower.starts_with("tech:") || lower.starts_with("технология:") {
            let title = text.splitn(2, ':').nth(1)?.trim().to_string();
            if !title.is_empty() {
                return Some(title);
            }
        }

        // Common technology keywords
        let tech_keywords = [
            "rust", "python", "javascript", "typescript", "react", "vue", "angular",
            "node", "deno", "bun", "postgresql", "mysql", "sqlite", "redis", "mongodb",
            "docker", "kubernetes", "aws", "gcp", "azure", "tauri", "electron",
            "openai", "anthropic", "gemini", "claude", "gpt", "llm", "ai",
            "mcp", "api", "rest", "graphql", "grpc", "websocket",
        ];

        for keyword in &tech_keywords {
            if lower.contains(keyword) {
                // Capitalize first letter
                let mut chars = keyword.chars();
                let capitalized: String = match chars.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                };
                return Some(capitalized);
            }
        }

        None
    }

    /// Extract task references from text.
    fn extract_task_reference(&self, text: &str) -> Option<(String, String)> {
        let lower = text.to_lowercase();
        
        // Pattern: "task: ..." or "задача: ..."
        if lower.starts_with("task:") || lower.starts_with("задача:") {
            let rest = text.splitn(2, ':').nth(1)?.trim().to_string();
            
            // Check for status indicators
            let status = if lower.contains("done") || lower.contains("выполнено") || lower.contains("✓") {
                "Done"
            } else if lower.contains("in progress") || lower.contains("в процессе") || lower.contains("⏳") {
                "In Progress"
            } else if lower.contains("pending") || lower.contains("ожидание") {
                "Pending"
            } else {
                "Active"
            };

            return Some((rest, status.to_string()));
        }

        // Pattern: "- [ ] Task" or "- [x] Task" (markdown checkbox).
        // Split on the closing bracket instead of slicing at a fixed byte offset:
        // `text[5..]` panics on short input ("- [x") and on multi-byte markers ("- [✓]").
        if let Some(rest) = text.strip_prefix("- [")
            && let Some((marker, task)) = rest.split_once(']')
        {
            let status = match marker.trim() {
                "x" | "X" => "Done",
                "" => "Pending",
                _ => "Active",
            };

            let task_text = task.trim().to_string();
            if !task_text.is_empty() {
                return Some((task_text, status.to_string()));
            }
        }

        None
    }

    /// Extract decision references from text.
    fn extract_decision_reference(&self, text: &str) -> Option<String> {
        let lower = text.to_lowercase();
        
        // Pattern: "decision: ..." or "решение: ..."
        if lower.starts_with("decision:") || lower.starts_with("решение:") {
            let title = text.splitn(2, ':').nth(1)?.trim().to_string();
            if !title.is_empty() {
                return Some(title);
            }
        }

        // Pattern: "DECIDED: ..." or "РЕШЕНО: ..."
        if lower.starts_with("decided:") || lower.starts_with("решено:") {
            let title = text.splitn(2, ':').nth(1)?.trim().to_string();
            if !title.is_empty() {
                return Some(title);
            }
        }

        None
    }

    /// Extract relationships from text based on entity co-occurrence.
    fn extract_relationships(&self, text: &str, entities: &[Entity]) -> Vec<(usize, usize, RelationshipType, f64)> {
        let mut relationships = Vec::new();
        let lines: Vec<&str> = text.lines().collect();

        for line in &lines {
            let lower = line.to_lowercase();
            
            // Find entities mentioned in this line
            let mentioned: Vec<usize> = entities
                .iter()
                .enumerate()
                .filter(|(_, e)| lower.contains(&e.title.to_lowercase()))
                .map(|(i, _)| i)
                .collect();

            // Create relationships between all mentioned entities
            for i in 0..mentioned.len() {
                for j in (i + 1)..mentioned.len() {
                    let source_idx = mentioned[i];
                    let target_idx = mentioned[j];
                    
                    // Determine relationship type based on context
                    let rel_type = self.infer_relationship_type(&lower);
                    let weight = self.calculate_relationship_weight(&lower, &entities[source_idx], &entities[target_idx]);

                    relationships.push((source_idx, target_idx, rel_type, weight));
                }
            }
        }

        relationships
    }

    /// Infer relationship type from context.
    fn infer_relationship_type(&self, context: &str) -> RelationshipType {
        if context.contains("uses") || context.contains("использует") || context.contains("using") {
            RelationshipType::Uses
        } else if context.contains("depends") || context.contains("зависит") {
            RelationshipType::DependsOn
        } else if context.contains("created") || context.contains("создал") {
            RelationshipType::Created
        } else if context.contains("related") || context.contains("связан") {
            RelationshipType::RelatedTo
        } else if context.contains("owns") || context.contains("владеет") {
            RelationshipType::Owns
        } else if context.contains("modified") || context.contains("изменил") {
            RelationshipType::Modified
        } else if context.contains("participated") || context.contains("участвовал") {
            RelationshipType::ParticipatedIn
        } else if context.contains("caused") || context.contains("вызвал") {
            RelationshipType::CausedBy
        } else if context.contains("mentions") || context.contains("упоминает") {
            RelationshipType::Mentions
        } else if context.contains("derived") || context.contains("произведен") {
            RelationshipType::DerivedFrom
        } else if context.contains("blocked") || context.contains("блокирует") {
            RelationshipType::BlockedBy
        } else if context.contains("replaced") || context.contains("заменяет") {
            RelationshipType::ReplacedBy
        } else {
            RelationshipType::RelatedTo
        }
    }

    /// Calculate relationship weight based on context strength.
    fn calculate_relationship_weight(&self, context: &str, source: &Entity, target: &Entity) -> f64 {
        let mut weight: f64 = 0.5; // Base weight

        // Strong indicators increase weight
        if context.contains("strongly") || context.contains("сильно") || context.contains("actively") {
            weight += 0.2;
        }

        // Explicit relationship verbs increase weight
        if context.contains("uses") || context.contains("depends") || context.contains("created") {
            weight += 0.1;
        }

        // Same type entities have stronger relationships
        if source.entity_type == target.entity_type {
            weight += 0.1;
        }

        // Clamp to 0.0-1.0
        weight.min(1.0).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests require a GraphStore mock — tested via integration
}
