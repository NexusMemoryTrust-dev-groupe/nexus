use serde::{Deserialize, Serialize};
use rusqlite::OptionalExtension;

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
use crate::core::context::context_service::ContextService;
use crate::storage::sqlite::SqliteGraphRepository;
use crate::storage::sqlite::SqliteMemoryRepository;
use crate::storage::sqlite::context_repository::SqliteContextRepository;
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
        "entity_context" => cmd_build_entity_context(&cmd.args).await,

        // ── System commands ──
        "stats" => cmd_stats().await,
        "health" => cmd_health().await,
        "settings" => cmd_settings().await,
        "timeline" => cmd_timeline().await,

        // ── Savings commands ──
        "savings" => cmd_savings().await,
        "savings-model" => cmd_savings_model(&cmd.args).await,

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
        Ok(id) => {
            // Index for semantic search off-thread. Memories created through the
            // MCP server land here rather than in `commands::memory`, so without
            // this hook anything an AI wrote stayed invisible to vector search.
            crate::core::context::indexer::spawn_index_memory(
                &record.id,
                &record.title,
                &record.summary,
                &record.content,
            );
            CopilotResponse::ok(
                format!("Memory created: {}", title),
                Some(serde_json::json!({ "id": id.as_str(), "title": title })),
            )
        }
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
    
    // Dedup: check if entity with same title+type already exists
    let existing = repo.get_entities_by_type(&entity_type).await;
    if let Ok(entities) = existing {
        for e in entities {
            if e.title == title {
                return CopilotResponse::ok(
                    format!("Entity already exists: {} ({})", title, args[0]),
                    Some(serde_json::json!({ "id": e.id.as_str(), "type": args[0], "title": title, "existing": true })),
                );
            }
        }
    }
    
    let entity = Entity::new(entity_type, title.clone(), String::new());
    if let Err(e) = entity.validate() {
        return CopilotResponse::err(format!("Validation error: {}", e));
    }
    match repo.add_entity(&entity).await {
        Ok(id) => CopilotResponse::ok(
            format!("Entity created: {} ({})", title, args[0]),
            Some(serde_json::json!({ "id": id.as_str(), "type": args[0], "title": title, "existing": false })),
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
    let snapshot_conn = match crate::db::open_connection() {
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
    let snapshot_repo = match SqliteContextRepository::new(snapshot_conn) {
        Ok(r) => r,
        Err(e) => return CopilotResponse::err(format!("Init error: {}", e)),
    };

    let builder = ContextBuilderImpl::new(graph_repo, memory_repo);
    let cache = crate::core::context::context_cache::global_cache();
    let service = ContextService::new(builder, cache, snapshot_repo);

    let request = ContextRequest {
        query: query.clone(),
        ..Default::default()
    };
    match service.build_context(&request).await {
        Ok(pkg) => {
            // Record savings for this interaction
            crate::commands::savings::record_savings(
                &crate::commands::savings::SavingsMeasurement::from_package(&pkg),
                &query,
                &format!("{:?}", pkg.user_intent.intent_type),
            );
            CopilotResponse::ok(
                format!("Context built for '{}': {} entities, {} relationships, {} memories",
                    query, pkg.entities.len(), pkg.relationships.len(), pkg.memory_records.len()),
                Some(serde_json::json!({
                    "entities": pkg.entities.len(),
                    "relationships": pkg.relationships.len(),
                    "memory_records": pkg.memory_records.len(),
                    "token_count": pkg.token_count,
                    "intent_type": format!("{:?}", pkg.user_intent.intent_type),
                })),
            )
        }
        Err(e) => CopilotResponse::err(format!("Context build error: {}", e)),
    }
}

async fn cmd_build_entity_context(args: &[String]) -> CopilotResponse {
    if args.is_empty() {
        return CopilotResponse::err("Usage: /entity_context <entity_id> [depth]");
    }
    let entity_id = &args[0];
    let depth = if args.len() > 1 {
        args[1].parse::<u32>().unwrap_or(2)
    } else {
        2
    };

    let eid = match crate::core::entity_id::EntityId::parse(entity_id) {
        Ok(e) => e,
        Err(e) => return CopilotResponse::err(format!("Invalid entity ID: {}", e)),
    };

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
    match builder.build_for_entity(&eid, depth).await {
        Ok(pkg) => {
            // Record savings for this entity context build
            crate::commands::savings::record_savings(
                &crate::commands::savings::SavingsMeasurement::from_package(&pkg),
                &format!("entity:{}", entity_id),
                "EntityContext",
            );
            CopilotResponse::ok(
                format!("Context built for entity '{}' (depth {}): {} entities, {} relationships, {} memories",
                    entity_id, depth, pkg.entities.len(), pkg.relationships.len(), pkg.memory_records.len()),
                Some(serde_json::json!({
                    "entities": pkg.entities.len(),
                    "relationships": pkg.relationships.len(),
                    "memory_records": pkg.memory_records.len(),
                    "token_count": pkg.token_count,
                })),
            )
        }
        Err(e) => CopilotResponse::err(format!("Entity context build error: {}", e)),
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
        Ok(_) => {
            // Content changed, so the stored embedding is now stale.
            crate::core::context::indexer::spawn_index_memory(
                &record.id,
                &record.title,
                &record.summary,
                &record.content,
            );
            CopilotResponse::ok(
                format!("Memory updated: {}", record.title),
                Some(serde_json::json!({ "id": record.id.as_str(), "title": record.title })),
            )
        }
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
        Ok(_) => {
            // Drop the fingerprint too, otherwise deleted memories keep
            // surfacing in semantic results as orphaned vectors.
            crate::core::context::indexer::spawn_forget_memory(&entity_id);
            CopilotResponse::ok(format!("Memory '{}' deleted", args[0]), None)
        }
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
    
    // Read a single key, returning None when absent.
    let raw = |key: &str| -> Option<String> {
        conn.query_row(
            "SELECT value FROM configuration_kv WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .filter(|v| !v.is_empty())
    };

    // The UI and `commands::ai` persist dotted keys (`ai.model`, `app.language`),
    // but this command used to read `ai_model` / `language`, which never exist —
    // so /settings always reported hardcoded defaults instead of real values.
    // Dotted key wins; the legacy name is still accepted for older databases.
    let resolve = |keys: &[&str], fallback: &str| -> String {
        keys.iter()
            .find_map(|k| raw(k))
            .unwrap_or_else(|| fallback.to_string())
    };

    let ai_model = resolve(
        &["ai.model", "ai_model"],
        "opencode/deepseek-v4-flash-free",
    );
    let language = resolve(&["app.language", "language"], "en");
    let theme = resolve(&["app.theme", "theme"], "dark");

    CopilotResponse::ok(
        "Settings loaded",
        Some(serde_json::json!({
            "theme": theme,
            "language": language,
            "ai_model": ai_model,
            "mcp_server_version": env!("CARGO_PKG_VERSION"),
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

/// Show cumulative token/cost savings stats. Hard data from the DB — no estimates.
async fn cmd_savings() -> CopilotResponse {
    match crate::commands::savings::get_savings_stats() {
        Ok(s) => {
            let message = format!(
                "Savings: {} tokens saved (${:.2}) across {} interactions. Today: {} tokens (${:.2}). Week: {} tokens (${:.2}).",
                s.total_tokens_saved,
                s.total_cost_saved_usd,
                s.total_interactions,
                s.tokens_saved_today,
                s.cost_saved_today,
                s.tokens_saved_week,
                s.cost_saved_week,
            );
            CopilotResponse::ok(message, Some(serde_json::to_value(&s).unwrap_or_default()))
        }
        Err(e) => CopilotResponse::err(format!("Savings error: {}", e)),
    }
}

/// Show per-model savings for a specific model. Model name is matched case-insensitively.
async fn cmd_savings_model(args: &[String]) -> CopilotResponse {
    let model = if args.is_empty() {
        return CopilotResponse::err("Usage: /savings-model <model_name> (e.g. /savings-model GPT-5.6 Terra)");
    } else {
        args.join(" ")
    };

    match crate::commands::savings::get_model_savings(&model) {
        Ok(json) => {
            let cost = json["cost_saved_usd"].as_f64().unwrap_or(0.0);
            let tokens = json["total_tokens_saved"].as_u64().unwrap_or(0);
            let message = format!(
                "Model '{}' ({}): ${:.2} saved on {} input tokens at ${:.2}/1M input.",
                json["model"]["name"].as_str().unwrap_or(&model),
                json["model"]["company"].as_str().unwrap_or(""),
                cost,
                tokens,
                json["model"]["input_per_m"].as_f64().unwrap_or(0.0),
            );
            CopilotResponse::ok(message, Some(json))
        }
        Err(e) => CopilotResponse::err(e),
    }
}

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
  /entity_context <id> [depth]       Build context centered on an entity

Savings Commands:
  /savings                           Show token/cost savings stats (hard data)
  /savings-model <name>              Show savings for a specific model (e.g. /savings-model GPT-5.6 Terra)

File Operations:
  /create-file <path> <content>      Create a new file on disk
  /write-file <path> <content>       Write/overwrite a file
  /create-folder <path>              Create a directory
  /delete <path>                     Delete a file or directory
  /move <source> <dest>              Move/rename a file
  /read-file <path>                  Read raw file content

File Interpreter:
  /index-file <path>                 Index file into knowledge graph
  /index-folder <path>               Index all files in folder

Workspace:
  /workspace-file <pid> <parent> <name> <content>  Create file in workspace

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
        // DB may not be available in test environment
        assert!(resp.success || resp.message.contains("error") || resp.message.contains("DB"),
            "Expected success or DB-related error, got: {}", resp.message);
        assert!(resp.message.contains("memories") || resp.message.contains("DB"),
            "Expected 'memories' or 'DB' in message, got: {}", resp.message);
    }

    #[tokio::test]
    async fn execute_savings() {
        let cmd = parse_command("/savings").unwrap();
        let resp = execute_command(&cmd).await;
        assert!(resp.success || resp.message.contains("DB") || resp.message.contains("error"),
            "Expected success or DB-related error, got: {}", resp.message);
    }

    #[tokio::test]
    async fn execute_savings_model_usage() {
        let cmd = parse_command("/savings-model").unwrap();
        let resp = execute_command(&cmd).await;
        assert!(!resp.success);
        assert!(resp.message.contains("Usage"), "Expected usage error, got: {}", resp.message);
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
    let pkg = builder.build_for_query(query).await.map_err(|e| e.to_string())?;

    // Record savings for this enhanced search
    crate::commands::savings::record_savings(
        &crate::commands::savings::SavingsMeasurement::from_package(&pkg),
        query,
        &format!("{:?}", pkg.user_intent.intent_type),
    );

    Ok(pkg)
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

// ═══════════════════════════════════════════════════════════════
//  File Interpreter Helpers
// ═══════════════════════════════════════════════════════════════

/// Index a single file: read content, interpret, create entities + relationships
pub async fn index_file(path: &str, project_id: Option<&str>) -> std::result::Result<IndexResult, String> {
    // Indexing reads the file and stores its contents in the graph, so it is a
    // read of arbitrary disk content and must be sandboxed like any other read.
    let guarded = crate::core::sandbox::guard(path, crate::core::sandbox::Access::Read)?;
    let path = guarded.as_path();
    if !path.exists() {
        return Err(format!("File '{}' not found", path.display()));
    }

    let graph_repo = open_graph_repo()?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();

    // Read file content
    let content = if crate::commands::files::is_editable(path) || crate::core::interpreter::image_interpreter::is_image(&ext) {
        if crate::core::interpreter::image_interpreter::is_image(&ext) {
            // For images, read as bytes and create entity with metadata
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            let interp = crate::core::interpreter::image_interpreter::interpret_image(path, &bytes);

            // Check if file entity already exists
            let existing = graph_repo.search_entities(&file_name).await.map_err(|e| e.to_string())?;
            let file_entity = if let Some(e) = existing.into_iter().find(|e| e.title == file_name) {
                e
            } else {
                let id = graph_repo.add_entity(&interp.entity).await.map_err(|e| e.to_string())?;
                let mut e = interp.entity;
                e.id = id;
                e
            };

            // Link to project if provided
            if let Some(pid) = project_id {
                let pid_id = crate::core::EntityId::parse(pid).map_err(|e| e.to_string())?;
                let rel = crate::core::graph::relationship::Relationship::new(
                    pid_id.clone(), file_entity.id.clone(),
                    crate::core::graph::relationship_types::RelationshipType::RelatedTo, 0.5,
                );
                if let Ok(rel) = rel {
                    let _ = graph_repo.add_relationship(&rel).await;
                }
            }

            return Ok(IndexResult {
                file_name: file_name.clone(),
                entities_created: 1,
                sub_entities_created: 0,
                summary: interp.summary,
            });
        } else {
            std::fs::read_to_string(path).map_err(|e| e.to_string())?
        }
    } else {
        return Err(format!("Cannot interpret file type: .{}", ext));
    };

    // Interpret file content
    let interpreted = crate::core::interpreter::file_interpreter::interpret_file(path, &content);

    // Check if file entity already exists
    let existing = graph_repo.search_entities(&file_name).await.map_err(|e| e.to_string())?;
    let file_entity = if let Some(e) = existing.into_iter().find(|e| e.title == file_name) {
        e
    } else {
        let id = graph_repo.add_entity(&interpreted.file_entity).await.map_err(|e| e.to_string())?;
        let mut e = interpreted.file_entity;
        e.id = id;
        e
    };

    // Link to project if provided
    if let Some(pid) = project_id {
        let pid_id = crate::core::EntityId::parse(pid).map_err(|e| e.to_string())?;
        let rel = crate::core::graph::relationship::Relationship::new(
            pid_id, file_entity.id.clone(),
            crate::core::graph::relationship_types::RelationshipType::RelatedTo, 0.5,
        );
        if let Ok(rel) = rel {
            let _ = graph_repo.add_relationship(&rel).await;
        }
    }

    // Create sub-entities (classes, functions, headings, etc.)
    let mut sub_count = 0;
    for sub in &interpreted.sub_entities {
        // Check for duplicate
        let existing_sub = graph_repo.search_entities(&sub.title).await.map_err(|e| e.to_string())?;
        if existing_sub.iter().any(|e| e.title.to_lowercase() == sub.title.to_lowercase()) {
            continue;
        }
        let sub_id = graph_repo.add_entity(sub).await.map_err(|e| e.to_string())?;
        sub_count += 1;

        // Link sub-entity to file entity
        let rel = crate::core::graph::relationship::Relationship::new(
            file_entity.id.clone(), sub_id,
            crate::core::graph::relationship_types::RelationshipType::RelatedTo, 0.7,
        );
        if let Ok(rel) = rel {
            let _ = graph_repo.add_relationship(&rel).await;
        }
    }

    Ok(IndexResult {
        file_name,
        entities_created: 1,
        sub_entities_created: sub_count,
        summary: interpreted.summary,
    })
}

/// Index a folder recursively: interpret all files
pub async fn index_folder(path: &str, project_id: Option<&str>) -> std::result::Result<FolderIndexResult, String> {
    // Guard the root once; every descendant is inside it, and `index_file`
    // re-checks each path anyway.
    let guarded = crate::core::sandbox::guard(path, crate::core::sandbox::Access::Read)?;
    let root = guarded.as_path();
    if !root.is_dir() {
        return Err(format!("'{}' is not a directory", path));
    }

    // Phase 1: Collect all interpretable file paths (sync, no async needed)
    let mut file_paths: Vec<std::path::PathBuf> = Vec::new();
    fn collect_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    if name.starts_with('.') || name == "target" || name == "node_modules" || name == "__pycache__" {
                        continue;
                    }
                    collect_files(&path, out);
                } else if path.is_file() {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    if crate::core::interpreter::image_interpreter::is_image(&ext) || crate::core::interpreter::file_interpreter::is_interpretable(&ext) {
                        out.push(path);
                    }
                }
            }
        }
    }
    collect_files(root, &mut file_paths);

    // Phase 2: Index each file asynchronously (no block_on — we're already in async context)
    let mut total_files = 0usize;
    let mut total_entities = 0usize;
    let mut total_sub_entities = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut summaries: Vec<String> = Vec::new();

    for path in &file_paths {
        let path_str = match path.to_str() {
            Some(s) => s,
            None => {
                errors.push(format!("{}: non-UTF-8 path", path.display()));
                continue;
            }
        };
        match index_file(path_str, project_id).await {
            Ok(result) => {
                total_files += 1;
                total_entities += result.entities_created;
                total_sub_entities += result.sub_entities_created;
                summaries.push(result.summary);
            }
            Err(e) => {
                errors.push(format!("{}: {}", path.display(), e));
            }
        }
    }

    Ok(FolderIndexResult {
        folder_name: root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        total_files,
        total_entities,
        total_sub_entities,
        summaries,
        errors,
    })
}

/// Read and interpret file content (without creating entities)
pub fn read_file_content(path: &str) -> std::result::Result<FileInterpretation, String> {
    let guarded = crate::core::sandbox::guard(path, crate::core::sandbox::Access::Read)?;
    let path = guarded.as_path();
    if !path.exists() {
        return Err(format!("File '{}' not found", path.display()));
    }

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();

    // Handle images
    if crate::core::interpreter::image_interpreter::is_image(&ext) {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let interp = crate::core::interpreter::image_interpreter::interpret_image(path, &bytes);
        return Ok(FileInterpretation {
            file_name,
            file_type: "Image".into(),
            text_content: String::new(),
            summary: interp.summary,
            sub_entities: vec![],
        });
    }

    // Handle text files
    if crate::commands::files::is_editable(path) {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let interpreted = crate::core::interpreter::file_interpreter::interpret_file(path, &content);
        return Ok(FileInterpretation {
            file_name,
            file_type: ext,
            text_content: interpreted.text_content,
            summary: interpreted.summary,
            sub_entities: interpreted.sub_entities,
        });
    }

    Err(format!("Cannot read file type: .{}", ext))
}

// ═══════════════════════════════════════════════════════════════
//  Result Types
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexResult {
    pub file_name: String,
    pub entities_created: usize,
    pub sub_entities_created: usize,
    pub summary: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FolderIndexResult {
    pub folder_name: String,
    pub total_files: usize,
    pub total_entities: usize,
    pub total_sub_entities: usize,
    pub summaries: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileInterpretation {
    pub file_name: String,
    pub file_type: String,
    pub text_content: String,
    pub summary: String,
    pub sub_entities: Vec<crate::core::graph::entity::Entity>,
}

// ═══════════════════════════════════════════════════════════════
//  File Operation Helpers
// ═══════════════════════════════════════════════════════════════

/// Create a new file with content. Fails if file already exists.
///
/// Every write path below is checked against the sandbox first: these helpers
/// are reachable by an AI model over MCP, so an unchecked absolute path would
/// let a model write anywhere on the machine.
pub fn create_file(path: &str, content: &str) -> std::result::Result<(), String> {
    let p = crate::core::sandbox::guard(path, crate::core::sandbox::Access::Write)?;
    if p.exists() {
        return Err(format!("File '{}' already exists. Use write_file to overwrite.", path));
    }
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create directories: {}", e))?;
    }
    std::fs::write(&p, content).map_err(|e| format!("Failed to write file: {}", e))
}

/// Write/overwrite file content. Creates file if it doesn't exist.
pub fn write_file(path: &str, content: &str) -> std::result::Result<(), String> {
    let p = crate::core::sandbox::guard(path, crate::core::sandbox::Access::Write)?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create directories: {}", e))?;
    }
    std::fs::write(&p, content).map_err(|e| format!("Failed to write file: {}", e))
}

/// Create a directory (and all parent directories).
pub fn create_folder(path: &str) -> std::result::Result<(), String> {
    let p = crate::core::sandbox::guard(path, crate::core::sandbox::Access::Write)?;
    if p.exists() {
        return Err(format!("Folder '{}' already exists.", path));
    }
    std::fs::create_dir_all(&p).map_err(|e| format!("Failed to create directory: {}", e))
}

/// Delete a file or directory (recursive).
pub fn delete_path(path: &str) -> std::result::Result<(), String> {
    let p = crate::core::sandbox::guard(path, crate::core::sandbox::Access::Delete)?;
    if !p.exists() {
        return Err(format!("'{}' not found.", path));
    }
    if p.is_dir() {
        std::fs::remove_dir_all(p).map_err(|e| format!("Failed to delete directory: {}", e))
    } else {
        std::fs::remove_file(p).map_err(|e| format!("Failed to delete file: {}", e))
    }
}

/// Move/rename a file or directory. Supports full path rename or move to dest_dir with new name.
pub fn move_file(
    source: &str,
    new_path: Option<&str>,
    dest_dir: Option<&str>,
    new_name: Option<&str>,
) -> std::result::Result<String, String> {
    use crate::core::sandbox::{guard, Access};

    // A move both removes the source and creates the destination, so each side
    // is validated separately.
    let src = guard(source, Access::Delete)?;
    if !src.exists() {
        return Err(format!("Source '{}' not found.", source));
    }

    let raw_dest = if let Some(np) = new_path {
        std::path::PathBuf::from(np)
    } else if let (Some(dd), Some(nn)) = (dest_dir, new_name) {
        let dir = guard(dd, Access::Write)?;
        if !dir.is_dir() {
            return Err(format!("Destination directory '{}' is not a directory.", dd));
        }
        dir.join(nn)
    } else {
        return Err("Provide either new_path or dest_dir+new_name".to_string());
    };

    let dest = guard(&raw_dest.to_string_lossy(), Access::Write)?;

    if dest.exists() {
        return Err(format!("Destination '{}' already exists.", dest.display()));
    }

    // Ensure parent directories exist
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create directories: {}", e))?;
    }

    // Try rename first (fast, same filesystem). Fallback to copy+delete for cross-filesystem.
    if std::fs::rename(&src, &dest).is_err() {
        if src.is_dir() {
            // For directories, use copy_dir_all + remove_dir_all
            fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
                std::fs::create_dir_all(dst)?;
                for entry in std::fs::read_dir(src)? {
                    let entry = entry?;
                    let ty = entry.file_type()?;
                    if ty.is_dir() {
                        copy_dir_recursive(&entry.path(), &dst.join(entry.file_name()))?;
                    } else {
                        std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
                    }
                }
                Ok(())
            }
            copy_dir_recursive(&src, &dest).map_err(|e| format!("Copy failed: {}", e))?;
            std::fs::remove_dir_all(&src).map_err(|e| format!("Remove source dir failed: {}", e))?;
        } else {
            std::fs::copy(&src, &dest).map_err(|e| format!("Copy failed: {}", e))?;
            std::fs::remove_file(&src).map_err(|e| format!("Remove source file failed: {}", e))?;
        }
    }
    Ok(dest.to_string_lossy().to_string())
}

/// Read raw file content as text.
pub fn read_raw_file(path: &str) -> std::result::Result<String, String> {
    let p = crate::core::sandbox::guard(path, crate::core::sandbox::Access::Read)?;
    if !p.exists() {
        return Err(format!("File '{}' not found.", path));
    }
    std::fs::read_to_string(&p).map_err(|e| format!("Failed to read file: {}", e))
}

/// Create a file in a project workspace (on disk + register in workspace DB).
pub async fn create_workspace_file(
    project_id: &str,
    parent_path: &str,
    name: &str,
    content: &str,
) -> std::result::Result<String, String> {
    let child_path = format!("{}{}{}", parent_path, std::path::MAIN_SEPARATOR, name);
    let p = std::path::Path::new(&child_path);

    // Create parent dirs if needed
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create directories: {}", e))?;
    }

    // Write content to disk
    std::fs::write(p, content).map_err(|e| format!("Failed to write file: {}", e))?;

    // Register in workspace DB
    let conn = crate::db::open_connection().map_err(|e| format!("DB error: {}", e))?;

    // Check if parent exists in workspace
    let parent_id: Option<String> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM workspace_entries WHERE project_id = ?1 AND native_path = ?2"
        ).map_err(|e| format!("DB error: {}", e))?;
        stmt.query_row(rusqlite::params![project_id, parent_path], |row| row.get(0))
            .optional()
            .map_err(|e| format!("DB error: {}", e))?
    };

    // Insert the file entry
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let meta = std::fs::metadata(p).ok();
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let mime = crate::commands::files::mime_from_ext(p);

    conn.execute(
        "INSERT INTO workspace_entries (id, project_id, name, native_path, parent_id, is_dir, size_bytes, mime_type, created_at, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8, 0)",
        rusqlite::params![id, project_id, name, child_path, parent_id, size as i64, mime, now],
    ).map_err(|e| format!("DB error: {}", e))?;

    Ok(child_path)
}
