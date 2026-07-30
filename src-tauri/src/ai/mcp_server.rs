use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};

use super::copilot::{self, CopilotResponse, ParsedCommand};
use crate::core::graph::graph_store::GraphStore;

// ═══════════════════════════════════════════════════════════════
//  JSON-RPC 2.0 types (MCP protocol)
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(rename = "jsonrpc")]
    _jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

// ═══════════════════════════════════════════════════════════════
//  MCP tool definitions
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Serialize)]
struct ToolDefinition {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "nexus_copilot_command".to_string(),
            description: "Execute a Nexus copilot slash command. Supported: /memories, /memory <id>, /create-memory <title> <content>, /update-memory <id> <content>, /delete-memory <id>, /search <query>, /graph, /entity <id>, /create-entity <type> <title>, /update-entity <id> <title>, /delete-entity <id>, /link <source_id> <target_id> [type] [weight], /unlink <rel_id>, /context <query>, /stats, /health, /settings, /timeline".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The slash command to execute"
                    }
                },
                "required": ["command"]
            }),
        },
        ToolDefinition {
            name: "nexus_list_memories".to_string(),
            description: "List all memory records in the Nexus database".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        ToolDefinition {
            name: "nexus_get_memory".to_string(),
            description: "Get a single memory record by ID".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Memory UUID" }
                },
                "required": ["id"]
            }),
        },
        ToolDefinition {
            name: "nexus_create_memory".to_string(),
            description: "Create a new memory record with title and content".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Memory title" },
                    "content": { "type": "string", "description": "Memory content" },
                    "author": { "type": "string", "description": "Author name", "default": "user" }
                },
                "required": ["title", "content"]
            }),
        },
        ToolDefinition {
            name: "nexus_update_memory".to_string(),
            description: "Update an existing memory record's content".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Memory UUID" },
                    "content": { "type": "string", "description": "New content" }
                },
                "required": ["id", "content"]
            }),
        },
        ToolDefinition {
            name: "nexus_delete_memory".to_string(),
            description: "Delete a memory record by ID".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Memory UUID" }
                },
                "required": ["id"]
            }),
        },
        ToolDefinition {
            name: "nexus_search_memories".to_string(),
            description: "Search memories by query string".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "nexus_graph_stats".to_string(),
            description: "Get knowledge graph statistics (entity counts by type)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        ToolDefinition {
            name: "nexus_get_entity".to_string(),
            description: "Get a single entity by ID".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Entity UUID" }
                },
                "required": ["id"]
            }),
        },
        ToolDefinition {
            name: "nexus_create_entity".to_string(),
            description: "Create a new entity in the knowledge graph".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "entity_type": { "type": "string", "description": "Entity type (Person, Organization, Project, Document, Meeting, Decision, Task, Technology, Memory)" },
                    "title": { "type": "string", "description": "Entity title" }
                },
                "required": ["entity_type", "title"]
            }),
        },
        ToolDefinition {
            name: "nexus_update_entity".to_string(),
            description: "Update an existing entity's title".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Entity UUID" },
                    "title": { "type": "string", "description": "New title" }
                },
                "required": ["id", "title"]
            }),
        },
        ToolDefinition {
            name: "nexus_delete_entity".to_string(),
            description: "Delete an entity by ID".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Entity UUID" }
                },
                "required": ["id"]
            }),
        },
        ToolDefinition {
            name: "nexus_link_entities".to_string(),
            description: "Create a relationship between two entities".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "source_id": { "type": "string", "description": "Source entity UUID" },
                    "target_id": { "type": "string", "description": "Target entity UUID" },
                    "relationship_type": { "type": "string", "description": "Relationship type (Uses, DependsOn, CreatedBy, RelatedTo, Implements, etc.)", "default": "RelatedTo" },
                    "weight": { "type": "number", "description": "Relationship weight (0.0-1.0)", "default": 0.8 }
                },
                "required": ["source_id", "target_id"]
            }),
        },
        ToolDefinition {
            name: "nexus_unlink_entities".to_string(),
            description: "Delete a relationship by ID".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "relationship_id": { "type": "string", "description": "Relationship UUID" }
                },
                "required": ["relationship_id"]
            }),
        },
        ToolDefinition {
            name: "nexus_build_context".to_string(),
            description: "Build a context package for a query (full M4 pipeline)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Context query" }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "nexus_stats".to_string(),
            description: "Show database statistics (memory and entity counts)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        ToolDefinition {
            name: "nexus_health".to_string(),
            description: "Check system health (database connectivity)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        ToolDefinition {
            name: "nexus_settings".to_string(),
            description: "Get application settings".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        ToolDefinition {
            name: "nexus_timeline".to_string(),
            description: "Get timeline of all entities sorted by creation date".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        // ── Enhanced Intelligence Tools ──
        ToolDefinition {
            name: "nexus_parse_markdown".to_string(),
            description: "Parse markdown text and extract entities and relationships (Auto Graph Builder)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Markdown text to parse" }
                },
                "required": ["text"]
            }),
        },
        ToolDefinition {
            name: "nexus_search_context".to_string(),
            description: "Enhanced context search with intent detection, keywords, and temporal reasoning".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query with optional temporal references" }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "nexus_get_recent_memories".to_string(),
            description: "Get recent memories from the last N days".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "days": { "type": "integer", "description": "Number of days to look back", "default": 7 }
                },
            }),
        },
        ToolDefinition {
            name: "nexus_get_important_memories".to_string(),
            description: "Get memories with importance above threshold".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "threshold": { "type": "number", "description": "Importance threshold (0.0-1.0)", "default": 0.7 }
                },
            }),
        },
        ToolDefinition {
            name: "nexus_analyze_text".to_string(),
            description: "Analyze text to extract keywords, entities, and temporal references".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Text to analyze" }
                },
                "required": ["text"]
            }),
        },
        // ── Graph Entity Listing ──
        ToolDefinition {
            name: "nexus_list_graph_entities".to_string(),
            description: "List all graph entities, optionally filtered by type".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "entity_type": {
                        "type": "string",
                        "description": "Filter by entity type (Person, Organization, Project, Document, Meeting, Decision, Task, Technology, Memory)",
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max results to return",
                        "default": 100
                    }
                },
            }),
        },
        // ── Semantic Search Tools ──
        ToolDefinition {
            name: "nexus_search_semantic".to_string(),
            description: "Search memories by semantic similarity using ONNX embeddings (AllMiniLML6V2, 384-dim)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "limit": { "type": "integer", "description": "Max results", "default": 10 }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "nexus_store_fingerprint".to_string(),
            description: "Store semantic fingerprint for a memory".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "memory_id": { "type": "string", "description": "Memory UUID" },
                    "text": { "type": "string", "description": "Text to extract keywords from" }
                },
                "required": ["memory_id", "text"]
            }),
        },
        // ── Memory-Entity Link Tools ──
        ToolDefinition {
            name: "nexus_link_memory_entity".to_string(),
            description: "Link a memory to an entity".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "memory_id": { "type": "string", "description": "Memory UUID" },
                    "entity_id": { "type": "string", "description": "Entity UUID" },
                    "relationship": { "type": "string", "description": "Relationship type", "default": "Related" },
                    "weight": { "type": "number", "description": "Link weight (0-1)", "default": 1.0 }
                },
                "required": ["memory_id", "entity_id"]
            }),
        },
        ToolDefinition {
            name: "nexus_unlink_memory_entity".to_string(),
            description: "Remove link between a memory and an entity".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "memory_id": { "type": "string", "description": "Memory UUID" },
                    "entity_id": { "type": "string", "description": "Entity UUID" },
                    "relationship": { "type": "string", "description": "Relationship type", "default": "Related" }
                },
                "required": ["memory_id", "entity_id"]
            }),
        },
        ToolDefinition {
            name: "nexus_get_memory_links".to_string(),
            description: "Get all entity links for a memory".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "memory_id": { "type": "string", "description": "Memory UUID" }
                },
                "required": ["memory_id"]
            }),
        },
        ToolDefinition {
            name: "nexus_get_entity_memory_links".to_string(),
            description: "Get all memory links for an entity".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "entity_id": { "type": "string", "description": "Entity UUID" }
                },
                "required": ["entity_id"]
            }),
        },
    ]
}

// ═══════════════════════════════════════════════════════════════
//  Tool dispatch
// ═══════════════════════════════════════════════════════════════

async fn dispatch_tool(name: &str, args: &serde_json::Value) -> CopilotResponse {
    match name {
        "nexus_copilot_command" => {
            let cmd_str = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
            match copilot::parse_command(cmd_str) {
                Some(cmd) => copilot::execute_command(&cmd).await,
                None => CopilotResponse::err("Invalid command format. Must start with '/'"),
            }
        }
        "nexus_list_memories" => {
            match copilot::parse_command("/memories") {
                Some(cmd) => copilot::execute_command(&cmd).await,
                None => unreachable!(),
            }
        }
        "nexus_get_memory" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand { name: "memory".into(), args: vec![id.into()] };
            copilot::execute_command(&cmd).await
        }
        "nexus_create_memory" => {
            let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or(title);
            let cmd_args = vec![title.to_string(), content.to_string()];
            let cmd = ParsedCommand { name: "create-memory".into(), args: cmd_args };
            copilot::execute_command(&cmd).await
        }
        "nexus_update_memory" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand { name: "update-memory".into(), args: vec![id.into(), content.into()] };
            copilot::execute_command(&cmd).await
        }
        "nexus_delete_memory" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand { name: "delete-memory".into(), args: vec![id.into()] };
            copilot::execute_command(&cmd).await
        }
        "nexus_search_memories" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand { name: "search".into(), args: vec![query.to_string()] };
            copilot::execute_command(&cmd).await
        }
        "nexus_graph_stats" => {
            match copilot::parse_command("/graph") {
                Some(cmd) => copilot::execute_command(&cmd).await,
                None => unreachable!(),
            }
        }
        "nexus_get_entity" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand { name: "entity".into(), args: vec![id.into()] };
            copilot::execute_command(&cmd).await
        }
        "nexus_create_entity" => {
            let et = args.get("entity_type").and_then(|v| v.as_str()).unwrap_or("");
            let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand { name: "create-entity".into(), args: vec![et.into(), title.into()] };
            copilot::execute_command(&cmd).await
        }
        "nexus_update_entity" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand { name: "update-entity".into(), args: vec![id.into(), title.into()] };
            copilot::execute_command(&cmd).await
        }
        "nexus_delete_entity" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand { name: "delete-entity".into(), args: vec![id.into()] };
            copilot::execute_command(&cmd).await
        }
        "nexus_link_entities" => {
            let source = args.get("source_id").and_then(|v| v.as_str()).unwrap_or("");
            let target = args.get("target_id").and_then(|v| v.as_str()).unwrap_or("");
            let rel_type = args.get("relationship_type").and_then(|v| v.as_str()).unwrap_or("RelatedTo");
            let weight = args.get("weight").and_then(|v| v.as_f64()).unwrap_or(0.8).to_string();
            let cmd = ParsedCommand { name: "link".into(), args: vec![source.into(), target.into(), rel_type.into(), weight] };
            copilot::execute_command(&cmd).await
        }
        "nexus_unlink_entities" => {
            let id = args.get("relationship_id").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand { name: "unlink".into(), args: vec![id.into()] };
            copilot::execute_command(&cmd).await
        }
        "nexus_build_context" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand { name: "context".into(), args: vec![query.to_string()] };
            copilot::execute_command(&cmd).await
        }
        "nexus_stats" => {
            match copilot::parse_command("/stats") {
                Some(cmd) => copilot::execute_command(&cmd).await,
                None => unreachable!(),
            }
        }
        "nexus_health" => {
            match copilot::parse_command("/health") {
                Some(cmd) => copilot::execute_command(&cmd).await,
                None => unreachable!(),
            }
        }
        "nexus_settings" => {
            match copilot::parse_command("/settings") {
                Some(cmd) => copilot::execute_command(&cmd).await,
                None => unreachable!(),
            }
        }
        "nexus_timeline" => {
            match copilot::parse_command("/timeline") {
                Some(cmd) => copilot::execute_command(&cmd).await,
                None => unreachable!(),
            }
        }
        // ── Enhanced Intelligence Tools ──
        "nexus_parse_markdown" => {
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            match copilot::parse_and_build_graph(text).await {
                Ok(result) => CopilotResponse::ok(
                    format!("Parsed markdown: {} entities, {} relationships created", result.0.len(), result.1.len()),
                    Some(serde_json::json!({
                        "entities": result.0.len(),
                        "relationships": result.1.len(),
                    })),
                ),
                Err(e) => CopilotResponse::err(format!("Parse error: {}", e)),
            }
        }
        "nexus_search_context" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            match copilot::enhanced_context_search(query).await {
                Ok(result) => CopilotResponse::ok(
                    format!("Context built for '{}': {} entities, {} relationships, {} memories, {} keywords, temporal: {:?}",
                        query, result.entities.len(), result.relationships.len(), result.memory_records.len(),
                        result.user_intent.keywords.len(), result.user_intent.temporal),
                    Some(serde_json::json!({
                        "entities": result.entities.len(),
                        "relationships": result.relationships.len(),
                        "memories": result.memory_records.len(),
                        "keywords": result.user_intent.keywords,
                        "temporal": result.user_intent.temporal,
                        "intent": result.user_intent.intent_type,
                        "confidence": result.user_intent.confidence,
                        "token_count": result.token_count,
                    })),
                ),
                Err(e) => CopilotResponse::err(format!("Context error: {}", e)),
            }
        }
        "nexus_get_recent_memories" => {
            let days = args.get("days").and_then(|v| v.as_i64()).unwrap_or(7) as u32;
            match copilot::get_recent_memories(days).await {
                Ok(memories) => CopilotResponse::ok(
                    format!("Found {} recent memories (last {} days)", memories.len(), days),
                    Some(serde_json::json!({
                        "count": memories.len(),
                        "days": days,
                        "memories": memories.iter().map(|m| serde_json::json!({
                            "id": m.id,
                            "title": m.title,
                            "created_at": m.created_at,
                            "importance": m.importance_score,
                        })).collect::<Vec<_>>(),
                    })),
                ),
                Err(e) => CopilotResponse::err(format!("Error: {}", e)),
            }
        }
        "nexus_get_important_memories" => {
            let threshold = args.get("threshold").and_then(|v| v.as_f64()).unwrap_or(0.7);
            match copilot::get_important_memories(threshold).await {
                Ok(memories) => CopilotResponse::ok(
                    format!("Found {} important memories (threshold: {})", memories.len(), threshold),
                    Some(serde_json::json!({
                        "count": memories.len(),
                        "threshold": threshold,
                        "memories": memories.iter().map(|m| serde_json::json!({
                            "id": m.id,
                            "title": m.title,
                            "importance": m.importance_score,
                            "confidence": m.confidence_score,
                        })).collect::<Vec<_>>(),
                    })),
                ),
                Err(e) => CopilotResponse::err(format!("Error: {}", e)),
            }
        }
        "nexus_analyze_text" => {
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let detector = crate::core::context::IntentDetector::new();
            let keywords = detector.extract_keywords(text);
            let temporal = detector.detect_temporal(text);
            let intent = detector.detect(text);
            
            CopilotResponse::ok(
                format!("Analyzed text: {} keywords, temporal: {:?}, intent: {:?}", keywords.len(), temporal, intent.intent_type),
                Some(serde_json::json!({
                    "keywords": keywords,
                    "temporal": temporal,
                    "intent": intent.intent_type,
                    "confidence": intent.confidence,
                })),
            )
        }
        // ── Graph Entity Listing ──
        "nexus_list_graph_entities" => {
            let entity_type_filter = args.get("entity_type").and_then(|v| v.as_str());
            let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(100) as usize;
            
            let graph_repo = match crate::ai::copilot::open_graph_repo() {
                Ok(r) => r,
                Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
            };

            let mut all_entities: Vec<serde_json::Value> = Vec::new();
            let types_to_scan: Vec<crate::core::graph::entity_types::EntityType> = if let Some(et_str) = entity_type_filter {
                vec![crate::core::graph::entity_types::EntityType::from_str(et_str)]
            } else {
                vec![
                    crate::core::graph::entity_types::EntityType::Person,
                    crate::core::graph::entity_types::EntityType::Organization,
                    crate::core::graph::entity_types::EntityType::Project,
                    crate::core::graph::entity_types::EntityType::Document,
                    crate::core::graph::entity_types::EntityType::Meeting,
                    crate::core::graph::entity_types::EntityType::Decision,
                    crate::core::graph::entity_types::EntityType::Task,
                    crate::core::graph::entity_types::EntityType::Technology,
                    crate::core::graph::entity_types::EntityType::Memory,
                ]
            };

            for et in &types_to_scan {
                if let Ok(entities) = graph_repo.get_entities_by_type(et).await {
                    for e in entities {
                        if all_entities.len() >= limit { break; }
                        all_entities.push(serde_json::json!({
                            "id": e.id.as_str(),
                            "type": e.entity_type.as_str(),
                            "title": e.title,
                            "status": format!("{:?}", e.status),
                            "created_at": e.created_at.to_rfc3339(),
                        }));
                    }
                }
                if all_entities.len() >= limit { break; }
            }

            let count = all_entities.len();
            CopilotResponse::ok(
                format!("Found {} entities", count),
                Some(serde_json::json!({ "entities": all_entities, "count": count })),
            )
        }
        // ── Semantic Search Tools ──
        "nexus_search_semantic" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(10) as u32;
            match copilot::semantic_search(query, limit).await {
                Ok(results) => CopilotResponse::ok(
                    format!("Found {} semantically similar memories", results.len()),
                    Some(serde_json::json!({
                        "count": results.len(),
                        "results": results.iter().map(|(id, score)| serde_json::json!({
                            "memory_id": id,
                            "similarity": score,
                        })).collect::<Vec<_>>(),
                    })),
                ),
                Err(e) => CopilotResponse::err(format!("Semantic search error: {}", e)),
            }
        }
        "nexus_store_fingerprint" => {
            let memory_id = args.get("memory_id").and_then(|v| v.as_str()).unwrap_or("");
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            match copilot::store_fingerprint(memory_id, text).await {
                Ok(_) => CopilotResponse::ok(
                    format!("Stored fingerprint for memory {}", memory_id),
                    None,
                ),
                Err(e) => CopilotResponse::err(format!("Error: {}", e)),
            }
        }
        // ── Memory-Entity Link Tools ──
        "nexus_link_memory_entity" => {
            let memory_id = args.get("memory_id").and_then(|v| v.as_str()).unwrap_or("");
            let entity_id = args.get("entity_id").and_then(|v| v.as_str()).unwrap_or("");
            let relationship = args.get("relationship").and_then(|v| v.as_str()).unwrap_or("Related");
            let weight = args.get("weight").and_then(|v| v.as_f64()).unwrap_or(1.0);
            match copilot::link_memory_entity(memory_id, entity_id, relationship, weight).await {
                Ok(_) => CopilotResponse::ok(
                    format!("Linked memory {} to entity {} ({})", memory_id, entity_id, relationship),
                    None,
                ),
                Err(e) => CopilotResponse::err(format!("Error: {}", e)),
            }
        }
        "nexus_unlink_memory_entity" => {
            let memory_id = args.get("memory_id").and_then(|v| v.as_str()).unwrap_or("");
            let entity_id = args.get("entity_id").and_then(|v| v.as_str()).unwrap_or("");
            let relationship = args.get("relationship").and_then(|v| v.as_str()).unwrap_or("Related");
            match copilot::unlink_memory_entity(memory_id, entity_id, relationship).await {
                Ok(_) => CopilotResponse::ok(
                    format!("Unlinked memory {} from entity {} ({})", memory_id, entity_id, relationship),
                    None,
                ),
                Err(e) => CopilotResponse::err(format!("Error: {}", e)),
            }
        }
        "nexus_get_memory_links" => {
            let memory_id = args.get("memory_id").and_then(|v| v.as_str()).unwrap_or("");
            match copilot::get_memory_links(memory_id).await {
                Ok(links) => CopilotResponse::ok(
                    format!("Found {} links for memory {}", links.len(), memory_id),
                    Some(serde_json::json!({
                        "count": links.len(),
                        "links": links.iter().map(|l| serde_json::json!({
                            "entity_id": l.entity_id,
                            "relationship": l.relationship,
                            "weight": l.weight,
                        })).collect::<Vec<_>>(),
                    })),
                ),
                Err(e) => CopilotResponse::err(format!("Error: {}", e)),
            }
        }
        "nexus_get_entity_memory_links" => {
            let entity_id = args.get("entity_id").and_then(|v| v.as_str()).unwrap_or("");
            match copilot::get_entity_memory_links(entity_id).await {
                Ok(links) => CopilotResponse::ok(
                    format!("Found {} links for entity {}", links.len(), entity_id),
                    Some(serde_json::json!({
                        "count": links.len(),
                        "links": links.iter().map(|l| serde_json::json!({
                            "memory_id": l.memory_id,
                            "relationship": l.relationship,
                            "weight": l.weight,
                        })).collect::<Vec<_>>(),
                    })),
                ),
                Err(e) => CopilotResponse::err(format!("Error: {}", e)),
            }
        }
        other => CopilotResponse::err(format!("Unknown tool: {}", other)),
    }
}

// ═══════════════════════════════════════════════════════════════
//  MCP server (stdio JSON-RPC)
// ═══════════════════════════════════════════════════════════════

fn ok_response(id: Option<serde_json::Value>, result: serde_json::Value) -> JsonRpcResponse {
    JsonRpcResponse { jsonrpc: "2.0".into(), id, result: Some(result), error: None }
}

fn err_response(id: Option<serde_json::Value>, code: i64, msg: String) -> JsonRpcResponse {
    JsonRpcResponse { jsonrpc: "2.0".into(), id, result: None, error: Some(JsonRpcError { code, message: msg }) }
}

async fn handle_request(req: JsonRpcRequest) -> Option<JsonRpcResponse> {
    let response = match req.method.as_str() {
        "initialize" => {
            let capabilities = serde_json::json!({
                "tools": { "listChanged": false },
                "resources": { "listChanged": false }
            });
            let info = serde_json::json!({
                "name": "nexus-mcp-server",
                "version": "1.0.0"
            });
            ok_response(req.id, serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": capabilities,
                "serverInfo": info,
            }))
        }
        "notifications/initialized" => {
            // Per JSON-RPC 2.0 spec: notifications must NOT receive a response.
            return None;
        }
        "tools/list" => {
            let tools: Vec<serde_json::Value> = tool_definitions().into_iter().map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": t.input_schema,
                })
            }).collect();
            ok_response(req.id, serde_json::json!({ "tools": tools }))
        }
        "tools/call" => {
            let tool_name = req.params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let arguments = req.params.get("arguments").cloned().unwrap_or(serde_json::json!({}));
            let resp = dispatch_tool(tool_name, &arguments).await;

            let content = if resp.success {
                let mut text = resp.message.clone();
                if let Some(data) = &resp.data {
                    text.push_str("\n\n");
                    text.push_str(&serde_json::to_string_pretty(data).unwrap_or_default());
                }
                vec![serde_json::json!({ "type": "text", "text": text })]
            } else {
                vec![serde_json::json!({ "type": "text", "text": format!("Error: {}", resp.message) })]
            };

            ok_response(req.id, serde_json::json!({
                "content": content,
                "isError": !resp.success,
            }))
        }
        // ── MCP Resources ──
        "resources/list" => {
            let resources = vec![
                serde_json::json!({
                    "uri": "nexus://stats",
                    "name": "Database Statistics",
                    "description": "Memory and entity counts from the database",
                    "mimeType": "application/json",
                }),
                serde_json::json!({
                    "uri": "nexus://health",
                    "name": "System Health",
                    "description": "Database connectivity and system status",
                    "mimeType": "application/json",
                }),
                serde_json::json!({
                    "uri": "nexus://settings",
                    "name": "Application Settings",
                    "description": "Current application configuration",
                    "mimeType": "application/json",
                }),
            ];
            ok_response(req.id, serde_json::json!({ "resources": resources }))
        }
        "resources/read" => {
            let uri = req.params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
            match uri {
                "nexus://stats" => {
                    let resp = dispatch_tool("nexus_stats", &serde_json::json!({})).await;
                    let content = if resp.success {
                        let mut text = resp.message.clone();
                        if let Some(data) = &resp.data {
                            text.push_str("\n\n");
                            text.push_str(&serde_json::to_string_pretty(data).unwrap_or_default());
                        }
                        vec![serde_json::json!({ "type": "text", "text": text })]
                    } else {
                        vec![serde_json::json!({ "type": "text", "text": format!("Error: {}", resp.message) })]
                    };
                    ok_response(req.id, serde_json::json!({ "contents": [{ "uri": uri, "mimeType": "application/json", "text": content[0]["text"] }] }))
                }
                "nexus://health" => {
                    let resp = dispatch_tool("nexus_health", &serde_json::json!({})).await;
                    let text = resp.message.clone();
                    ok_response(req.id, serde_json::json!({ "contents": [{ "uri": uri, "mimeType": "application/json", "text": text }] }))
                }
                "nexus://settings" => {
                    let resp = dispatch_tool("nexus_settings", &serde_json::json!({})).await;
                    let mut text = resp.message.clone();
                    if let Some(data) = &resp.data {
                        text.push_str("\n\n");
                        text.push_str(&serde_json::to_string_pretty(data).unwrap_or_default());
                    }
                    ok_response(req.id, serde_json::json!({ "contents": [{ "uri": uri, "mimeType": "application/json", "text": text }] }))
                }
                _ => err_response(req.id, -32602, format!("Resource not found: {}", uri)),
            }
        }
        "ping" => {
            ok_response(req.id, serde_json::json!({}))
        }
        _ => {
            err_response(req.id, -32601, format!("Method not found: {}", req.method))
        }
    };
    Some(response)
}

/// Run the MCP server on stdio (blocking). Reads JSON-RPC messages line-by-line.
pub async fn run_stdio() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let reader = stdin.lock();
    let mut line_iter = reader.lines();

    eprintln!("[nexus-mcp] Server started on stdio");

    while let Some(line_result) = line_iter.next() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[nexus-mcp] Read error: {}", e);
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let resp = err_response(None, -32700, format!("Parse error: {}", e));
                let json = serde_json::to_string(&resp).unwrap_or_default();
                let _ = writeln!(stdout, "{}", json);
                let _ = stdout.flush();
                continue;
            }
        };

        let resp = handle_request(req).await;
        // Skip response for notifications (JSON-RPC 2.0 spec)
        if let Some(resp) = resp {
            let json = serde_json::to_string(&resp).unwrap_or_default();
            let _ = writeln!(stdout, "{}", json);
            let _ = stdout.flush();
        }
    }

    eprintln!("[nexus-mcp] Server stopped");
}

// ═══════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definitions_not_empty() {
        let tools = tool_definitions();
        assert!(!tools.is_empty());
        assert_eq!(tools.len(), 31);
    }

    #[test]
    fn tool_definitions_have_required_fields() {
        let tools = tool_definitions();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert!(tool.input_schema.is_object());
        }
    }

    #[tokio::test]
    async fn dispatch_unknown_tool() {
        let resp = dispatch_tool("unknown_tool", &serde_json::json!({})).await;
        assert!(!resp.success);
        assert!(resp.message.contains("Unknown tool"));
    }

    #[tokio::test]
    async fn dispatch_health_tool() {
        let resp = dispatch_tool("nexus_health", &serde_json::json!({})).await;
        assert!(resp.success);
        assert!(resp.message.contains("DB"));
    }

    #[tokio::test]
    async fn dispatch_list_memories_tool() {
        let resp = dispatch_tool("nexus_list_memories", &serde_json::json!({})).await;
        // DB may or may not have memories, but command should succeed or report error
        assert!(resp.success || resp.message.contains("error") || resp.message.contains("DB") || resp.message.contains("not found"),
            "Expected success or DB-related error, got: {}", resp.message);
    }

    #[tokio::test]
    async fn dispatch_stats_tool() {
        let resp = dispatch_tool("nexus_stats", &serde_json::json!({})).await;
        // DB may not be available in test environment
        assert!(resp.success || resp.message.contains("error") || resp.message.contains("DB"),
            "Expected success or DB-related error, got: {}", resp.message);
        assert!(resp.message.contains("memories") || resp.message.contains("DB"),
            "Expected 'memories' or 'DB' in message, got: {}", resp.message);
    }

    #[tokio::test]
    async fn dispatch_copilot_command_tool() {
        let resp = dispatch_tool("nexus_copilot_command", &serde_json::json!({
            "command": "/health"
        })).await;
        assert!(resp.success);
    }

    #[tokio::test]
    async fn handle_initialize_request() {
        let req = JsonRpcRequest {
            _jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: "initialize".into(),
            params: serde_json::json!({}),
        };
        let resp = handle_request(req).await.unwrap();
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert_eq!(result["serverInfo"]["name"], "nexus-mcp-server");
    }

    #[tokio::test]
    async fn handle_notifications_initialized_returns_none() {
        let req = JsonRpcRequest {
            _jsonrpc: "2.0".into(),
            id: None,
            method: "notifications/initialized".into(),
            params: serde_json::json!({}),
        };
        let resp = handle_request(req).await;
        assert!(resp.is_none(), "Notifications must not receive responses per JSON-RPC 2.0");
    }

    #[tokio::test]
    async fn handle_tools_list_request() {
        let req = JsonRpcRequest {
            _jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: "tools/list".into(),
            params: serde_json::json!({}),
        };
        let resp = handle_request(req).await.unwrap();
        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 31);
    }

    #[tokio::test]
    async fn handle_tools_call_health() {
        let req = JsonRpcRequest {
            _jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: "tools/call".into(),
            params: serde_json::json!({
                "name": "nexus_health",
                "arguments": {}
            }),
        };
        let resp = handle_request(req).await.unwrap();
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], false);
    }
}
