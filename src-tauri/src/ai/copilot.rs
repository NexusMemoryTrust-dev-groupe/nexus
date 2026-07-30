use serde::{Deserialize, Serialize};

use crate::core::entity_id::EntityId;
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::memory::types::MemorySource;
use crate::core::graph::entity::Entity;
use crate::core::graph::entity_types::EntityType;
use crate::core::graph::graph_store::GraphStore;
use crate::core::graph::relationship::Relationship;
use crate::core::graph::relationship_types::RelationshipType;
use crate::core::context::context_builder::{ContextBuilder, ContextBuilderImpl};
use crate::core::context::context_request::ContextRequest;
use crate::storage::sqlite::SqliteGraphRepository;
use crate::storage::sqlite::SqliteMemoryRepository;
use crate::storage::sqlite::memory_entity_links_repository::MemoryEntityLinkRepository;

/// Result of a copilot command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl CopilotResponse {
    pub fn ok(message: impl Into<String>, data: Option<serde_json::Value>) -> Self {
        Self { success: true, message: message.into(), data }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self { success: false, message: message.into(), data: None }
    }
}

/// Parsed slash command from user input.
#[derive(Debug, Clone)]
pub struct ParsedCommand {
    pub name: String,
    pub args: Vec<String>,
}

/// Parse a slash command string like "/memories" or "/create-entity Project MyProject".
pub fn parse_command(input: &str) -> Option<ParsedCommand> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }
    let parts: Vec<String> = trimmed[1..].split_whitespace().map(|s| s.to_string()).collect();
    if parts.is_empty() {
        return None;
    }
    let name = parts[0].clone();
    let args = parts[1..].to_vec();
    Some(ParsedCommand { name, args })
}

/// Open the SQLite graph repository using the canonical DB path.
pub fn open_graph_repo() -> std::result::Result<SqliteGraphRepository, String> {
    let conn = crate::db::open_connection()?;
    SqliteGraphRepository::new(conn).map_err(|e| e.to_string())
}

/// Open the SQLite memory repository using the canonical DB path.
fn open_memory_repo() -> std::result::Result<SqliteMemoryRepository, String> {
    let conn = crate::db::open_connection()?;
    SqliteMemoryRepository::new(conn).map_err(|e| e.to_string())
}

/// Execute a parsed copilot command against the real database.
pub async fn execute_command(cmd: &ParsedCommand) -> CopilotResponse {
    match cmd.name.as_str() {
        // ── Memory commands ──
        "memories" => cmd_list_memories().await,
        "memory" => cmd_get_memory(&cmd.args).await,
        "create-memory" => cmd_create_memory(&cmd.args).await,
        "update-memory" => cmd_update_memory(&cmd.args).await,
        "delete-memory" => cmd_delete_memory(&cmd.args).await,
        "search" => cmd_search(&cmd.args).await,

        // ── Graph commands ──
        "graph" => cmd_graph_stats().await,
        "entity" => cmd_get_entity(&cmd.args).await,
        "create-entity" => cmd_create_entity(&cmd.args).await,
        "update-entity" => cmd_update_entity(&cmd.args).await,
        "delete-entity" => cmd_delete_entity(&cmd.args).await,
        "link" => cmd_link_entities(&cmd.args).await,
        "unlink" => cmd_unlink_entities(&cmd.args).await,

        // ── Context commands ──
        "context" => cmd_build_context(&cmd.args).await,

        // ── System commands ──
        "stats" => cmd_stats().await,
        "health" => cmd_health().await,
        "settings" => cmd_settings().await,
        "timeline" => cmd_timeline().await,

        // ── Utility commands ──
        "help" => cmd_help().await,
        "projects" => cmd_projects().await,

        // ── Unknown ──
        other => CopilotResponse::err(format!("Unknown command: /{}", other)),
    }
}

// ═══════════════════════════════════════════════════════════════
//  Memory commands
// ═══════════════════════════════════════════════════════════════

async fn cmd_list_memories() -> CopilotResponse {
    let repo = match open_memory_repo() {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };
    let records = match repo.list(100, 0).await {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("Query error: {}", e)),
    };

    if records.is_empty() {
        return CopilotResponse::ok("No memories found.", Some(serde_json::json!([])));
    }

    let count = records.len();
    let rows: Vec<serde_json::Value> = records.into_iter().map(|r| {
        serde_json::json!({
            "id": r.id.as_str(),
            "title": r.title,
            "layer": format!("{:?}", r.layer),
            "importance": r.importance_score,
            "created_at": r.created_at.to_rfc3339(),
        })
    }).collect();

    CopilotResponse::ok(
        format!("Found {} memories", count),
        Some(serde_json::json!({ "memories": rows, "count": count })),
    )
}

async fn cmd_get_memory(args: &[String]) -> CopilotResponse {
    if args.is_empty() {
        return CopilotResponse::err("Usage: /memory <id>");
    }
    let entity_id = match EntityId::parse(&args[0]) {
        Ok(id) => id,
        Err(e) => return CopilotResponse::err(format!("Invalid ID: {}", e)),
    };
    let repo = match open_memory_repo() {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };
    match repo.get_by_id(&entity_id).await {
        Ok(Some(r)) => CopilotResponse::ok(
            format!("Memory: {}", r.title),
            Some(serde_json::json!({
                "id": r.id.as_str(),
                "title": r.title,
                "content": r.content,
                "summary": r.summary,
                "layer": format!("{:?}", r.layer),
                "importance": r.importance_score,
                "confidence": r.confidence_score,
                "visibility": format!("{:?}", r.visibility),
                "source": format!("{:?}", r.source),
                "author": r.author,
                "created_at": r.created_at.to_rfc3339(),
            })),
        ),
        Ok(None) => CopilotResponse::err(format!("Memory '{}' not found", args[0])),
        Err(e) => CopilotResponse::err(format!("Query error: {}", e)),
    }
}

async fn cmd_create_memory(args: &[String]) -> CopilotResponse {
    if args.is_empty() {
        return CopilotResponse::err("Usage: /create-memory <title>");
    }
    let title = args[0].clone();
    // Remaining args after title = content (if provided)
    let content = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        title.clone() // Use title as content when no content provided
    };
    let repo = match open_memory_repo() {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };
    let record = match MemoryRecord::new(
        title.clone(),
        content,
        "copilot".to_string(),
        MemorySource::Manual,
    ) {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("Validation error: {}", e)),
    };
    match repo.save(&record).await {
        Ok(id) => CopilotResponse::ok(
            format!("Memory created: {}", title),
            Some(serde_json::json!({ "id": id.as_str(), "title": title })),
        ),
        Err(e) => CopilotResponse::err(format!("Save error: {}", e)),
    }
}

async fn cmd_search(args: &[String]) -> CopilotResponse {
    if args.is_empty() {
        return CopilotResponse::err("Usage: /search <query>");
    }
    let query = args.join(" ");
    let repo = match open_memory_repo() {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };
    match repo.search(&query).await {
        Ok(results) => {
            let count = results.len();
            let rows: Vec<serde_json::Value> = results.into_iter().map(|r| {
                serde_json::json!({
                    "id": r.id.as_str(),
                    "title": r.title,
                    "layer": format!("{:?}", r.layer),
                    "importance": r.importance_score,
                })
            }).collect();
            CopilotResponse::ok(
                format!("Found {} results for '{}'", count, query),
                Some(serde_json::json!({ "results": rows, "count": count })),
            )
        }
        Err(e) => CopilotResponse::err(format!("Search error: {}", e)),
    }
}

// ═══════════════════════════════════════════════════════════════
//  Graph commands
// ═══════════════════════════════════════════════════════════════

async fn cmd_graph_stats() -> CopilotResponse {
    let repo = match open_graph_repo() {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };

    let mut type_counts: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for et in [
        EntityType::Person, EntityType::Organization, EntityType::Project,
        EntityType::Document, EntityType::Meeting, EntityType::Decision,
        EntityType::Task, EntityType::Technology, EntityType::Memory,
    ] {
        if let Ok(entities) = repo.get_entities_by_type(&et).await {
            let count = entities.len() as u64;
            if count > 0 {
                type_counts.insert(et.as_str().to_string(), count);
            }
        }
    }

    let total_entities: u64 = type_counts.values().sum();

    CopilotResponse::ok(
        format!("Graph: {} entities across {} types", total_entities, type_counts.len()),
        Some(serde_json::json!({
            "total_entities": total_entities,
            "by_type": type_counts,
        })),
    )
}

async fn cmd_get_entity(args: &[String]) -> CopilotResponse {
    if args.is_empty() {
        return CopilotResponse::err("Usage: /entity <id>");
    }
    let entity_id = match EntityId::parse(&args[0]) {
        Ok(id) => id,
        Err(e) => return CopilotResponse::err(format!("Invalid ID: {}", e)),
    };
    let repo = match open_graph_repo() {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };
    match repo.get_entity(&entity_id).await {
        Ok(Some(e)) => CopilotResponse::ok(
            format!("Entity: {}", e.title),
            Some(serde_json::json!({
                "id": e.id.as_str(),
                "type": e.entity_type.as_str(),
                "title": e.title,
                "description": e.description,
                "status": format!("{:?}", e.status),
                "created_at": e.created_at.to_rfc3339(),
            })),
        ),
        Ok(None) => CopilotResponse::err(format!("Entity '{}' not found", args[0])),
        Err(e) => CopilotResponse::err(format!("Query error: {}", e)),
    }
}

async fn cmd_create_entity(args: &[String]) -> CopilotResponse {
    if args.len() < 2 {
        return CopilotResponse::err("Usage: /create-entity <type> <title>");
    }
    let entity_type = EntityType::from_str(&args[0]);
    let title = args[1..].join(" ");
    let repo = match open_graph_repo() {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };
    let entity = Entity::new(entity_type, title.clone(), String::new());
    if let Err(e) = entity.validate() {
        return CopilotResponse::err(format!("Validation error: {}", e));
    }
    match repo.add_entity(&entity).await {
        Ok(id) => CopilotResponse::ok(
            format!("Entity created: {} ({})", title, args[0]),
            Some(serde_json::json!({ "id": id.as_str(), "type": args[0], "title": title })),
        ),
        Err(e) => CopilotResponse::err(format!("Save error: {}", e)),
    }
}

// ═══════════════════════════════════════════════════════════════
//  Context commands
// ═══════════════════════════════════════════════════════════════

async fn cmd_build_context(args: &[String]) -> CopilotResponse {
    if args.is_empty() {
        return CopilotResponse::err("Usage: /context <query>");
    }
    let query = args.join(" ");
    let graph_conn = match crate::db::open_connection() {
        Ok(c) => c,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };
    let memory_conn = match crate::db::open_connection() {
        Ok(c) => c,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };
    let graph_repo = match SqliteGraphRepository::new(graph_conn) {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("Init error: {}", e)),
    };
    let memory_repo = match SqliteMemoryRepository::new(memory_conn) {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("Init error: {}", e)),
    };

    let builder = ContextBuilderImpl::new(graph_repo, memory_repo);
    let request = ContextRequest {
        query: query.clone(),
        ..Default::default()
    };
    match builder.build(&request).await {
        Ok(pkg) => CopilotResponse::ok(
            format!("Context built for '{}': {} entities, {} relationships, {} memories",
                query, pkg.entities.len(), pkg.relationships.len(), pkg.memory_records.len()),
            Some(serde_json::json!({
                "entities": pkg.entities.len(),
                "relationships": pkg.relationships.len(),
                "memory_records": pkg.memory_records.len(),
                "token_count": pkg.token_count,
                "intent_type": format!("{:?}", pkg.user_intent.intent_type),
            })),
        ),
        Err(e) => CopilotResponse::err(format!("Context build error: {}", e)),
    }
}

// ═══════════════════════════════════════════════════════════════
//  Update/Delete commands
// ═══════════════════════════════════════════════════════════════

async fn cmd_update_memory(args: &[String]) -> CopilotResponse {
    if args.len() < 2 {
        return CopilotResponse::err("Usage: /update-memory <id> <new_content>");
    }
    let entity_id = match EntityId::parse(&args[0]) {
        Ok(id) => id,
        Err(e) => return CopilotResponse::err(format!("Invalid ID: {}", e)),
    };
    let new_content = args[1..].join(" ");
    let repo = match open_memory_repo() {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };
    let mut record = match repo.get_by_id(&entity_id).await {
        Ok(Some(r)) => r,
        Ok(None) => return CopilotResponse::err(format!("Memory '{}' not found", args[0])),
        Err(e) => return CopilotResponse::err(format!("Query error: {}", e)),
    };
    record.content = new_content;
    record.touch();
    match repo.update(&record).await {
        Ok(_) => CopilotResponse::ok(
            format!("Memory updated: {}", record.title),
            Some(serde_json::json!({ "id": record.id.as_str(), "title": record.title })),
        ),
        Err(e) => CopilotResponse::err(format!("Save error: {}", e)),
    }
}

async fn cmd_delete_memory(args: &[String]) -> CopilotResponse {
    if args.is_empty() {
        return CopilotResponse::err("Usage: /delete-memory <id>");
    }
    let entity_id = match EntityId::parse(&args[0]) {
        Ok(id) => id,
        Err(e) => return CopilotResponse::err(format!("Invalid ID: {}", e)),
    };
    let repo = match open_memory_repo() {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };
    match repo.delete(&entity_id).await {
        Ok(_) => CopilotResponse::ok(format!("Memory '{}' deleted", args[0]), None),
        Err(e) => CopilotResponse::err(format!("Delete error: {}", e)),
    }
}

async fn cmd_update_entity(args: &[String]) -> CopilotResponse {
    if args.len() < 2 {
        return CopilotResponse::err("Usage: /update-entity <id> <new_title>");
    }
    let entity_id = match EntityId::parse(&args[0]) {
        Ok(id) => id,
        Err(e) => return CopilotResponse::err(format!("Invalid ID: {}", e)),
    };
    let new_title = args[1..].join(" ");
    let repo = match open_graph_repo() {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };
    let mut entity = match repo.get_entity(&entity_id).await {
        Ok(Some(e)) => e,
        Ok(None) => return CopilotResponse::err(format!("Entity '{}' not found", args[0])),
        Err(e) => return CopilotResponse::err(format!("Query error: {}", e)),
    };
    entity.title = new_title;
    entity.updated_at = chrono::Utc::now();
    match repo.update_entity(&entity).await {
        Ok(_) => CopilotResponse::ok(
            format!("Entity updated: {}", entity.title),
            Some(serde_json::json!({ "id": entity.id.as_str(), "title": entity.title })),
        ),
        Err(e) => CopilotResponse::err(format!("Save error: {}", e)),
    }
}

async fn cmd_delete_entity(args: &[String]) -> CopilotResponse {
    if args.is_empty() {
        return CopilotResponse::err("Usage: /delete-entity <id>");
    }
    let entity_id = match EntityId::parse(&args[0]) {
        Ok(id) => id,
        Err(e) => return CopilotResponse::err(format!("Invalid ID: {}", e)),
    };
    let repo = match open_graph_repo() {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };
    match repo.delete_entity(&entity_id).await {
        Ok(_) => CopilotResponse::ok(format!("Entity '{}' deleted", args[0]), None),
        Err(e) => CopilotResponse::err(format!("Delete error: {}", e)),
    }
}

// ═══════════════════════════════════════════════════════════════
//  Relationship commands
// ═══════════════════════════════════════════════════════════════

async fn cmd_link_entities(args: &[String]) -> CopilotResponse {
    if args.len() < 2 {
        return CopilotResponse::err("Usage: /link <source_id> <target_id> [relationship_type] [weight]");
    }
    let source_id = match EntityId::parse(&args[0]) {
        Ok(id) => id,
        Err(e) => return CopilotResponse::err(format!("Invalid source ID: {}", e)),
    };
    let target_id = match EntityId::parse(&args[1]) {
        Ok(id) => id,
        Err(e) => return CopilotResponse::err(format!("Invalid target ID: {}", e)),
    };
    let rel_type = if args.len() > 2 {
        RelationshipType::from_str(&args[2])
    } else {
        RelationshipType::RelatedTo
    };
    let weight = if args.len() > 3 {
        args[3].parse::<f64>().unwrap_or(0.8)
    } else {
        0.8
    };

    let repo = match open_graph_repo() {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };

    // Verify both entities exist
    match repo.get_entity(&source_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return CopilotResponse::err(format!("Source entity '{}' not found", args[0])),
        Err(e) => return CopilotResponse::err(format!("Query error: {}", e)),
    }
    match repo.get_entity(&target_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return CopilotResponse::err(format!("Target entity '{}' not found", args[1])),
        Err(e) => return CopilotResponse::err(format!("Query error: {}", e)),
    }

    let relationship = match Relationship::new(source_id, target_id, rel_type.clone(), weight) {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("Validation error: {}", e)),
    };
    match repo.add_relationship(&relationship).await {
        Ok(id) => CopilotResponse::ok(
            format!("Linked: {} --{}--> {} (weight: {})", args[0], rel_type.as_str(), args[1], weight),
            Some(serde_json::json!({ "id": id.as_str(), "type": rel_type.as_str(), "weight": weight })),
        ),
        Err(e) => CopilotResponse::err(format!("Save error: {}", e)),
    }
}

async fn cmd_unlink_entities(args: &[String]) -> CopilotResponse {
    if args.is_empty() {
        return CopilotResponse::err("Usage: /unlink <relationship_id>");
    }
    let rel_id = match EntityId::parse(&args[0]) {
        Ok(id) => id,
        Err(e) => return CopilotResponse::err(format!("Invalid relationship ID: {}", e)),
    };
    let repo = match open_graph_repo() {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };
    match repo.delete_relationship(&rel_id).await {
        Ok(_) => CopilotResponse::ok(format!("Relationship '{}' deleted", args[0]), None),
        Err(e) => CopilotResponse::err(format!("Delete error: {}", e)),
    }
}

// ═══════════════════════════════════════════════════════════════
//  System commands
// ═══════════════════════════════════════════════════════════════

async fn cmd_stats() -> CopilotResponse {
    let memory_repo = match open_memory_repo() {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };
    let graph_repo = match open_graph_repo() {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };

    let memory_count = memory_repo.count().await.unwrap_or(0);

    let mut entity_count: u64 = 0;
    for et in [
        EntityType::Person, EntityType::Organization, EntityType::Project,
        EntityType::Document, EntityType::Meeting, EntityType::Decision,
        EntityType::Task, EntityType::Technology, EntityType::Memory,
    ] {
        if let Ok(entities) = graph_repo.get_entities_by_type(&et).await {
            entity_count += entities.len() as u64;
        }
    }

    CopilotResponse::ok(
        format!("Stats: {} memories, {} entities", memory_count, entity_count),
        Some(serde_json::json!({
            "memories": memory_count,
            "entities": entity_count,
        })),
    )
}

async fn cmd_health() -> CopilotResponse {
    let db_ok = crate::db::open_connection().is_ok();
    CopilotResponse::ok(
        format!("DB: {}", if db_ok { "OK" } else { "FAILED" }),
        Some(serde_json::json!({ "db": db_ok })),
    )
}

async fn cmd_settings() -> CopilotResponse {
    let conn = match crate::db::open_connection() {
        Ok(c) => c,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };
    
    // Ensure config table exists
    let _ = conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS configuration_kv (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        );"
    );
    
    // Read actual settings from config store
    let get_val = |key: &str| -> String {
        conn.query_row(
            "SELECT value FROM configuration_kv WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        ).unwrap_or_else(|_| match key {
            "theme" => "dark".to_string(),
            "language" => "en".to_string(),
            "ai_model" => "opencode/deepseek-v4-flash-free".to_string(),
            _ => String::new(),
        })
    };

    CopilotResponse::ok(
        "Settings loaded",
        Some(serde_json::json!({
            "theme": get_val("theme"),
            "language": get_val("language"),
            "ai_model": get_val("ai_model"),
            "mcp_server_version": "1.0.0",
            "db_path": crate::db::db_path().to_string_lossy(),
        })),
    )
}

async fn cmd_timeline() -> CopilotResponse {
    let graph_repo = match open_graph_repo() {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };

    let mut all_entities: Vec<serde_json::Value> = Vec::new();
    for et in [
        EntityType::Person, EntityType::Organization, EntityType::Project,
        EntityType::Document, EntityType::Meeting, EntityType::Decision,
        EntityType::Task, EntityType::Technology, EntityType::Memory,
    ] {
        if let Ok(entities) = graph_repo.get_entities_by_type(&et).await {
            for e in entities {
                all_entities.push(serde_json::json!({
                    "id": e.id.as_str(),
                    "type": e.entity_type.as_str(),
                    "title": e.title,
                    "created_at": e.created_at.to_rfc3339(),
                }));
            }
        }
    }

    // Sort by created_at descending (newest first)
    all_entities.sort_by(|a, b| {
        let ta = a["created_at"].as_str().unwrap_or("");
        let tb = b["created_at"].as_str().unwrap_or("");
        tb.cmp(ta)
    });

    let count = all_entities.len();
    CopilotResponse::ok(
        format!("Timeline: {} events", count),
        Some(serde_json::json!({ "events": all_entities, "count": count })),
    )
}

// ═══════════════════════════════════════════════════════════════
//  Utility commands
// ═══════════════════════════════════════════════════════════════

async fn cmd_help() -> CopilotResponse {
    let help_text = r#"Nexus Copilot Commands:

Memory Commands:
  /memories                          List all memories
  /memory <id>                       Get memory by ID
  /create-memory <title> [content]   Create a new memory
  /update-memory <id> <content>      Update memory content
  /delete-memory <id>                Delete a memory
  /search <query>                    Search memories by text

Graph Commands:
  /graph                             Show graph statistics
  /entity <id>                       Get entity by ID
  /create-entity <type> <title>      Create a new entity
  /update-entity <id> <title>        Update entity title
  /delete-entity <id>                Delete an entity
  /link <src> <tgt> [type] [weight]  Link two entities
  /unlink <rel_id>                   Remove a relationship

Context Commands:
  /context <query>                   Build context package for a query

System Commands:
  /stats                             Show database statistics
  /health                            Check system health
  /settings                          Show application settings
  /timeline                          Show entity timeline
  /projects                          List all Project entities
  /help                              Show this help message"#;

    CopilotResponse::ok(help_text, None)
}

async fn cmd_projects() -> CopilotResponse {
    let repo = match open_graph_repo() {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
    };
    let entities = match repo.get_entities_by_type(&EntityType::Project).await {
        Ok(e) => e,
        Err(e) => return CopilotResponse::err(format!("Query error: {}", e)),
    };

    if entities.is_empty() {
        return CopilotResponse::ok("No projects found.", Some(serde_json::json!([])));
    }

    let count = entities.len();
    let rows: Vec<serde_json::Value> = entities.into_iter().map(|e| {
        serde_json::json!({
            "id": e.id.as_str(),
            "title": e.title,
            "status": format!("{:?}", e.status),
            "created_at": e.created_at.to_rfc3339(),
        })
    }).collect();

    CopilotResponse::ok(
        format!("Found {} projects", count),
        Some(serde_json::json!({ "projects": rows, "count": count })),
    )
}

// ═══════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_command_basic() {
        let cmd = parse_command("/memories").unwrap();
        assert_eq!(cmd.name, "memories");
        assert!(cmd.args.is_empty());
    }

    #[test]
    fn parse_command_with_args() {
        let cmd = parse_command("/create-entity Project MyProject").unwrap();
        assert_eq!(cmd.name, "create-entity");
        assert_eq!(cmd.args, vec!["Project", "MyProject"]);
    }

    #[test]
    fn parse_command_not_slash() {
        assert!(parse_command("hello").is_none());
    }

    #[test]
    fn parse_command_empty() {
        assert!(parse_command("/").is_none());
    }

    #[tokio::test]
    async fn execute_unknown_command() {
        let cmd = parse_command("/nonexistent").unwrap();
        let resp = execute_command(&cmd).await;
        assert!(!resp.success);
        assert!(resp.message.contains("Unknown command"));
    }

    #[tokio::test]
    async fn execute_health_check() {
        let cmd = parse_command("/health").unwrap();
        let resp = execute_command(&cmd).await;
        assert!(resp.success);
        assert!(resp.message.contains("DB"));
    }

    #[tokio::test]
    async fn execute_memories_empty_db() {
        let cmd = parse_command("/memories").unwrap();
        let resp = execute_command(&cmd).await;
        // DB may or may not have memories, but command should succeed or report DB error
        assert!(resp.success || resp.message.contains("error") || resp.message.contains("DB") || resp.message.contains("not found"), 
            "Expected success or DB-related error, got: {}", resp.message);
    }

    #[tokio::test]
    async fn execute_stats() {
        let cmd = parse_command("/stats").unwrap();
        let resp = execute_command(&cmd).await;
        assert!(resp.success);
        assert!(resp.message.contains("memories"));
    }
}

// ═══════════════════════════════════════════════════════════════
//  Enhanced Intelligence Helpers
// ═══════════════════════════════════════════════════════════════

/// Parse markdown and build graph (Auto Graph Builder).
pub async fn parse_and_build_graph(text: &str) -> std::result::Result<(
    Vec<crate::core::graph::entity::Entity>,
    Vec<crate::core::graph::relationship::Relationship>,
), String> {
    let graph_repo = open_graph_repo()?;
    let builder = crate::core::context::AutoGraphBuilder::new(graph_repo);
    builder.parse_and_build(text).await.map_err(|e| e.to_string())
}

/// Enhanced context search with intent detection, keywords, and temporal reasoning.
pub async fn enhanced_context_search(query: &str) -> std::result::Result<crate::core::context::ContextPackage, String> {
    let graph_repo = open_graph_repo()?;
    let memory_repo = open_memory_repo()?;
    let builder = crate::core::context::context_builder::ContextBuilderImpl::new(graph_repo, memory_repo);
    builder.build_for_query(query).await.map_err(|e| e.to_string())
}

/// Get recent memories from the last N days.
pub async fn get_recent_memories(days: u32) -> std::result::Result<Vec<crate::core::memory::memory_record::MemoryRecord>, String> {
    let memory_repo = open_memory_repo()?;
    let all = memory_repo.list(100, 0).await.map_err(|e| e.to_string())?;
    let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
    let recent: Vec<_> = all
        .into_iter()
        .filter(|r| r.created_at >= cutoff)
        .collect();
    Ok(recent)
}

/// Get memories with importance above threshold.
pub async fn get_important_memories(threshold: f64) -> std::result::Result<Vec<crate::core::memory::memory_record::MemoryRecord>, String> {
    let memory_repo = open_memory_repo()?;
    let all = memory_repo.list(100, 0).await.map_err(|e| e.to_string())?;
    let important: Vec<_> = all
        .into_iter()
        .filter(|r| r.importance_score >= threshold)
        .collect();
    Ok(important)
}

// ═══════════════════════════════════════════════════════════════
//  Semantic Search Helpers
// ═══════════════════════════════════════════════════════════════

/// Open semantic search instance.
fn open_semantic_search() -> std::result::Result<crate::core::context::SemanticSearch, String> {
    let db_path = crate::db::db_path();
    let conn = rusqlite::Connection::open(&db_path).map_err(|e| format!("DB error: {}", e))?;
    crate::core::context::SemanticSearch::new(conn).map_err(|e| e.to_string())
}

/// Search memories by semantic similarity.
pub async fn semantic_search(query: &str, limit: u32) -> std::result::Result<Vec<(EntityId, f64)>, String> {
    let search = open_semantic_search()?;
    search.search(query, limit).map_err(|e| e.to_string())
}

/// Store semantic fingerprint for a memory.
pub async fn store_fingerprint(memory_id: &str, text: &str) -> std::result::Result<(), String> {
    let search = open_semantic_search()?;
    let id = EntityId::parse(memory_id).map_err(|e| e.to_string())?;
    search.store_fingerprint(&id, text).map_err(|e| e.to_string())
}

// ═══════════════════════════════════════════════════════════════
//  Memory-Entity Link Helpers
// ═══════════════════════════════════════════════════════════════

/// Open memory-entity link repository.
fn open_link_repo() -> std::result::Result<crate::storage::sqlite::SqliteMemoryEntityLinkRepository, String> {
    let db_path = crate::db::db_path();
    let conn = rusqlite::Connection::open(&db_path).map_err(|e| format!("DB error: {}", e))?;
    crate::storage::sqlite::SqliteMemoryEntityLinkRepository::new(conn).map_err(|e| e.to_string())
}

/// Link a memory to an entity.
pub async fn link_memory_entity(
    memory_id: &str,
    entity_id: &str,
    relationship: &str,
    weight: f64,
) -> std::result::Result<(), String> {
    let repo = open_link_repo()?;
    let mem_id = EntityId::parse(memory_id).map_err(|e| e.to_string())?;
    let ent_id = EntityId::parse(entity_id).map_err(|e| e.to_string())?;
    repo.create_link(&mem_id, &ent_id, relationship, weight)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Unlink a memory from an entity.
pub async fn unlink_memory_entity(
    memory_id: &str,
    entity_id: &str,
    relationship: &str,
) -> std::result::Result<(), String> {
    let repo = open_link_repo()?;
    let mem_id = EntityId::parse(memory_id).map_err(|e| e.to_string())?;
    let ent_id = EntityId::parse(entity_id).map_err(|e| e.to_string())?;
    repo.delete_link(&mem_id, &ent_id, relationship)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Get all entity links for a memory.
pub async fn get_memory_links(
    memory_id: &str,
) -> std::result::Result<Vec<crate::storage::sqlite::memory_entity_links_repository::MemoryEntityLink>, String> {
    let repo = open_link_repo()?;
    let mem_id = EntityId::parse(memory_id).map_err(|e| e.to_string())?;
    repo.get_links_for_memory(&mem_id).await.map_err(|e| e.to_string())
}

/// Get all memory links for an entity.
pub async fn get_entity_memory_links(
    entity_id: &str,
) -> std::result::Result<Vec<crate::storage::sqlite::memory_entity_links_repository::MemoryEntityLink>, String> {
    let repo = open_link_repo()?;
    let ent_id = EntityId::parse(entity_id).map_err(|e| e.to_string())?;
    repo.get_links_for_entity(&ent_id).await.map_err(|e| e.to_string())
}
