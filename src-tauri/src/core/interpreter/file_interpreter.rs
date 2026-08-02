use crate::core::graph::entity::Entity;
use crate::core::graph::entity_types::EntityType;
use std::path::Path;

/// Result of interpreting a file
#[derive(Debug, Clone)]
pub struct InterpretedFile {
    /// The file entity itself
    pub file_entity: Entity,
    /// Sub-entities extracted (classes, functions, headings, etc.)
    pub sub_entities: Vec<Entity>,
    /// Raw text content (for searchable indexing)
    pub text_content: String,
    /// Human-readable summary
    pub summary: String,
}

/// Interpret a file by its extension and content
pub fn interpret_file(path: &Path, content: &str) -> InterpretedFile {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let file_name = path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let (sub_entities, summary) = match ext.as_str() {
        // Code files
        "py" => {
            let parsed = super::code_parser::parse_python(content);
            (parsed.entities, parsed.summary)
        }
        "js" | "jsx" | "mjs" => {
            let parsed = super::code_parser::parse_javascript(content);
            (parsed.entities, parsed.summary)
        }
        "ts" | "tsx" => {
            let parsed = super::code_parser::parse_typescript(content);
            (parsed.entities, parsed.summary)
        }
        "rs" => {
            let parsed = super::code_parser::parse_rust(content);
            (parsed.entities, parsed.summary)
        }
        "go" => {
            let parsed = super::code_parser::parse_go(content);
            (parsed.entities, parsed.summary)
        }
        "java" => {
            let parsed = super::code_parser::parse_java(content);
            (parsed.entities, parsed.summary)
        }
        "c" | "cpp" | "h" | "hpp" => {
            let parsed = super::code_parser::parse_c_cpp(content, &ext);
            (parsed.entities, parsed.summary)
        }
        // Markdown
        "md" | "markdown" => {
            let parsed = super::markdown_parser::parse(content);
            (parsed.entities, parsed.summary)
        }
        // Config files
        "json" => {
            let parsed = super::config_parser::parse_json(content, &file_name);
            (parsed.entities, parsed.summary)
        }
        "yaml" | "yml" => {
            let parsed = super::config_parser::parse_yaml(content, &file_name);
            (parsed.entities, parsed.summary)
        }
        "toml" => {
            let parsed = super::config_parser::parse_toml(content, &file_name);
            (parsed.entities, parsed.summary)
        }
        // Everything else — just store as document
        _ => {
            (vec![], format!("File: {}", file_name))
        }
    };

    // Determine file type label
    let file_type = match ext.as_str() {
        "py" => "Python",
        "js" | "jsx" | "mjs" => "JavaScript",
        "ts" | "tsx" => "TypeScript",
        "rs" => "Rust",
        "go" => "Go",
        "java" => "Java",
        "c" | "cpp" | "h" | "hpp" => "C/C++",
        "md" | "markdown" => "Markdown",
        "json" => "JSON",
        "yaml" | "yml" => "YAML",
        "toml" => "TOML",
        "html" | "htm" => "HTML",
        "css" => "CSS",
        "sql" => "SQL",
        "sh" | "bat" | "ps1" => "Shell",
        _ => "Document",
    };

    // Create the file entity
    let description = if sub_entities.is_empty() {
        format!("{} file: {}", file_type, file_name)
    } else {
        format!("{} file: {} — {} items", file_type, file_name, sub_entities.len())
    };

    let mut file_entity = Entity::new(EntityType::Document, file_name.clone(), description);
    file_entity.metadata.insert("source_path".into(), serde_json::json!(path.to_string_lossy().to_string()));
    file_entity.metadata.insert("file_type".into(), serde_json::json!(file_type));

    InterpretedFile {
        file_entity,
        sub_entities,
        text_content: content.to_string(),
        summary,
    }
}

/// Check if a file extension is supported for interpretation
pub fn is_interpretable(ext: &str) -> bool {
    matches!(ext.to_lowercase().as_str(),
        "py" | "js" | "jsx" | "mjs" | "ts" | "tsx" |
        "rs" | "go" | "java" | "c" | "cpp" | "h" | "hpp" |
        "md" | "markdown" |
        "json" | "yaml" | "yml" | "toml" |
        "html" | "htm" | "css" | "sql" |
        "sh" | "bat" | "ps1"
    )
}

/// Get supported extensions list
pub fn supported_extensions() -> Vec<&'static str> {
    vec![
        "py", "js", "jsx", "mjs", "ts", "tsx",
        "rs", "go", "java", "c", "cpp", "h", "hpp",
        "md", "markdown",
        "json", "yaml", "yml", "toml",
        "html", "htm", "css", "sql",
        "sh", "bat", "ps1",
    ]
}
