use crate::core::graph::entity::Entity;
use crate::core::graph::entity_types::EntityType;

/// Result of parsing a markdown file
pub struct ParsedMarkdown {
    pub entities: Vec<Entity>,
    pub summary: String,
}

/// Parse markdown content and extract entities
pub fn parse(content: &str) -> ParsedMarkdown {
    let mut entities = Vec::new();
    let mut heading_count = 0;
    let mut link_count = 0;
    let mut code_block_count = 0;
    let mut in_code_block = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Track code blocks
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            if !in_code_block {
                code_block_count += 1;
            }
            continue;
        }

        // Skip content inside code blocks
        if in_code_block {
            continue;
        }

        // Headings: # Heading, ## Heading, etc.
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|&c| c == '#').count();
            if level <= 6 {
                let title = trimmed[level..].trim().to_string();
                if !title.is_empty() {
                    let desc = format!("Markdown H{} heading", level);
                    let mut e = Entity::new(EntityType::Document, title, desc);
                    e.metadata.insert("kind".into(), serde_json::json!("heading"));
                    e.metadata.insert("level".into(), serde_json::json!(level));
                    entities.push(e);
                    heading_count += 1;
                }
            }
        }

        // Links: [text](url) or bare URLs
        if trimmed.contains("](http") || trimmed.contains("](") {
            link_count += 1;
        }

        // Task lists: - [ ] or - [x]
        if trimmed.starts_with("- [") || trimmed.starts_with("* [") {
            let done = trimmed.starts_with("- [x]") || trimmed.starts_with("- [X]") || trimmed.starts_with("* [x]") || trimmed.starts_with("* [X]");
            let task_text = if done {
                trimmed.strip_prefix("- [x]").or_else(|| trimmed.strip_prefix("- [X]"))
                    .or_else(|| trimmed.strip_prefix("* [x]")).or_else(|| trimmed.strip_prefix("* [X]"))
                    .unwrap_or("").trim()
            } else {
                trimmed.strip_prefix("- [ ]").or_else(|| trimmed.strip_prefix("* [ ]"))
                    .unwrap_or("").trim()
            };
            if !task_text.is_empty() {
                let status = if done { "Done" } else { "Pending" };
                let desc = format!("Markdown task [{}]: {}", status, task_text);
                let mut e = Entity::new(EntityType::Task, task_text.to_string(), desc);
                e.metadata.insert("kind".into(), serde_json::json!("task"));
                e.metadata.insert("done".into(), serde_json::json!(done));
                entities.push(e);
            }
        }
    }

    let summary = format!("Markdown: {} headings, {} links, {} code blocks",
        heading_count, link_count, code_block_count);
    ParsedMarkdown { entities, summary }
}

/// Extract plain text from markdown (strip formatting)
pub fn strip_markdown(content: &str) -> String {
    let mut result = String::new();
    let mut in_code_block = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }

        if in_code_block {
            result.push_str(trimmed);
            result.push('\n');
            continue;
        }

        // Strip heading markers
        let clean = trimmed.trim_start_matches('#').trim();

        // Strip bold/italic markers
        let clean = clean.trim_start_matches("**").trim_end_matches("**");
        let clean = clean.trim_start_matches("*").trim_end_matches("*");
        let clean = clean.trim_start_matches("__").trim_end_matches("__");
        let clean = clean.trim_start_matches("_").trim_end_matches("_");

        // Strip inline code
        let clean = clean.trim_start_matches('`').trim_end_matches('`');

        // Strip links, keep text
        let clean = if let Some(text) = clean.strip_prefix("[").and_then(|s| s.split_once("]")) {
            text.0
        } else {
            clean
        };

        if !clean.is_empty() {
            result.push_str(clean);
            result.push('\n');
        }
    }

    result
}
