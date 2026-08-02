use crate::core::graph::entity::Entity;
use crate::core::graph::entity_types::EntityType;

/// Result of parsing a config file
pub struct ParsedConfig {
    pub entities: Vec<Entity>,
    pub summary: String,
}

/// Parse JSON config file
pub fn parse_json(content: &str, file_name: &str) -> ParsedConfig {
    let mut entities = Vec::new();
    let mut key_count = 0;

    // Try to parse as JSON
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(content) {
        extract_json_keys(&value, "", &mut entities, &mut key_count);
    }

    let summary = format!("JSON config ({}): {} top-level keys", file_name, key_count);
    ParsedConfig { entities, summary }
}

/// Parse YAML config file
pub fn parse_yaml(content: &str, file_name: &str) -> ParsedConfig {
    let mut entities = Vec::new();
    let mut key_count = 0;

    // Simple line-based YAML key extraction
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // YAML keys are at the start of line, followed by colon
        if let Some(colon_pos) = trimmed.find(':') {
            let key = trimmed[..colon_pos].trim().to_string();
            if !key.is_empty() && !key.starts_with('-') {
                // Check indentation level for hierarchy
                let indent = line.len() - line.trim_start().len();
                let desc = format!("YAML key (indent: {})", indent);
                let mut e = Entity::new(EntityType::Document, key, desc);
                e.metadata
                    .insert("kind".into(), serde_json::json!("yaml_key"));
                e.metadata
                    .insert("file".into(), serde_json::json!(file_name));
                entities.push(e);
                key_count += 1;
            }
        }
    }

    let summary = format!("YAML config ({}): {} keys", file_name, key_count);
    ParsedConfig { entities, summary }
}

/// Parse TOML config file
pub fn parse_toml(content: &str, file_name: &str) -> ParsedConfig {
    let mut entities = Vec::new();
    let mut key_count = 0;
    let mut section_count = 0;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // TOML sections: [section] or [section.subsection]
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section = trimmed[1..trimmed.len() - 1].trim().to_string();
            if !section.is_empty() {
                let mut e = Entity::new(EntityType::Document, section, "TOML section".into());
                e.metadata
                    .insert("kind".into(), serde_json::json!("toml_section"));
                e.metadata
                    .insert("file".into(), serde_json::json!(file_name));
                entities.push(e);
                section_count += 1;
            }
        }

        // TOML key = value
        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim().to_string();
            if !key.is_empty() && !key.starts_with('[') {
                key_count += 1;
            }
        }
    }

    let summary = format!(
        "TOML config ({}): {} sections, {} keys",
        file_name, section_count, key_count
    );
    ParsedConfig { entities, summary }
}

/// Recursively extract keys from JSON value
fn extract_json_keys(
    value: &serde_json::Value,
    prefix: &str,
    entities: &mut Vec<Entity>,
    count: &mut usize,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map {
                let full_key = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };

                let desc = match val {
                    serde_json::Value::String(s) => format!(
                        "JSON string: \"{}\"",
                        crate::core::text::truncate_chars(s, 50)
                    ),
                    serde_json::Value::Number(n) => format!("JSON number: {}", n),
                    serde_json::Value::Bool(b) => format!("JSON bool: {}", b),
                    serde_json::Value::Array(_) => "JSON array".to_string(),
                    serde_json::Value::Object(_) => "JSON object".to_string(),
                    serde_json::Value::Null => "JSON null".to_string(),
                };

                let mut e = Entity::new(EntityType::Document, full_key.clone(), desc);
                e.metadata
                    .insert("kind".into(), serde_json::json!("json_key"));
                entities.push(e);
                *count += 1;

                // Recurse into nested objects
                if let serde_json::Value::Object(_) = val {
                    extract_json_keys(val, &full_key, entities, count);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            // Extract first few items if they're objects
            for (i, item) in arr.iter().take(3).enumerate() {
                if let serde_json::Value::Object(_) = item {
                    extract_json_keys(item, &format!("{}[{}]", prefix, i), entities, count);
                }
            }
        }
        _ => {}
    }
}
