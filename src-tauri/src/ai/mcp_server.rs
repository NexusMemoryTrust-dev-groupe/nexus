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
            description: "Execute a Nexus copilot slash command. Supported: /memories, /memory <id>, /create-memory <title> <content>, /update-memory <id> <content>, /delete-memory <id>, /search <query>, /graph, /entity <id>, /create-entity <type> <title>, /update-entity <id> <title>, /delete-entity <id>, /link <source_id> <target_id> [type] [weight], /unlink <rel_id>, /context <query>, /entity_context <id> [depth], /stats, /health, /settings, /timeline, /savings, /savings-model <model_name>, /help, /projects".to_string(),
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
            name: "nexus_build_context_for_entity".to_string(),
            description: "Build a context package centered on a specific entity with configurable traversal depth".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "entity_id": { "type": "string", "description": "Entity UUID" },
                    "depth": { "type": "integer", "description": "Traversal depth (1=hops only, 2=hops of hops, default=2)", "default": 2 }
                },
                "required": ["entity_id"]
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
        // ── Workspace Tools ──
        ToolDefinition {
            name: "nexus_add_to_workspace".to_string(),
            description: "Add native file(s)/folder(s) to a project workspace. Scans directories recursively and registers all files.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "Project entity UUID" },
                    "paths": { "type": "array", "items": { "type": "string" }, "description": "Native paths to add (files or folders)" }
                },
                "required": ["project_id", "paths"]
            }),
        },
        ToolDefinition {
            name: "nexus_get_workspace".to_string(),
            description: "Get the workspace file tree for a project".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "Project entity UUID" }
                },
                "required": ["project_id"]
            }),
        },
        ToolDefinition {
            name: "nexus_sync_workspace".to_string(),
            description: "Sync workspace: rescan root dirs, remove stale entries, add new files from disk".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "Project entity UUID" }
                },
                "required": ["project_id"]
            }),
        },
        // ── File Interpreter Tools ──
        ToolDefinition {
            name: "nexus_index_file".to_string(),
            description: "Index a file into the knowledge graph: reads content, extracts entities (classes, functions, headings, etc.), creates Document entity + sub-entities with relationships. Supports: py, js, ts, rs, go, java, c, cpp, md, json, yaml, toml, html, css, images.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute file path" },
                    "project_id": { "type": "string", "description": "Optional: Project entity UUID to link file to" }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "nexus_index_folder".to_string(),
            description: "Index all interpretable files in a folder recursively into the knowledge graph. Skips hidden dirs, target/, node_modules/, __pycache__/.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute folder path" },
                    "project_id": { "type": "string", "description": "Optional: Project entity UUID to link files to" }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "nexus_read_file_content".to_string(),
            description: "Read and interpret file content: returns summary, extracted entities, and raw text. Does NOT create entities in the graph — use nexus_index_file for that.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute file path" }
                },
                "required": ["path"]
            }),
        },
        // ── File Operation Tools ──
        ToolDefinition {
            name: "nexus_create_file".to_string(),
            description: "Create a new file on disk with content. Creates parent directories automatically. Fails if file already exists — use nexus_write_file to overwrite.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute file path to create" },
                    "content": { "type": "string", "description": "File content to write" }
                },
                "required": ["path", "content"]
            }),
        },
        ToolDefinition {
            name: "nexus_write_file".to_string(),
            description: "Write content to a file. Creates file if it doesn't exist, overwrites if it does. Creates parent directories automatically.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute file path to write" },
                    "content": { "type": "string", "description": "Content to write" }
                },
                "required": ["path", "content"]
            }),
        },
        ToolDefinition {
            name: "nexus_create_folder".to_string(),
            description: "Create a directory (and all parent directories) on disk.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute directory path to create" }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "nexus_delete_file".to_string(),
            description: "Delete a file or directory (recursive for directories). Use with caution.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute path to delete" }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "nexus_move_file".to_string(),
            description: "Move or rename a file/directory. Provide either new_path (full destination) or dest_dir + new_name.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "source_path": { "type": "string", "description": "Source file/directory path" },
                    "new_path": { "type": "string", "description": "New full destination path (for rename/move)" },
                    "dest_dir": { "type": "string", "description": "Destination directory (for move)" },
                    "new_name": { "type": "string", "description": "New name (used with dest_dir)" }
                },
                "required": ["source_path"]
            }),
        },
        ToolDefinition {
            name: "nexus_read_file".to_string(),
            description: "Read raw file content as text. Returns the file content without interpretation or entity extraction. Use for reading code, config files, etc.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute file path to read" }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "nexus_create_workspace_file".to_string(),
            description: "Create a file in a project workspace — creates on disk AND registers in the workspace database. Use this when an AI wants to save generated code into a Nexus project.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "Project entity UUID" },
                    "parent_path": { "type": "string", "description": "Absolute path of parent directory in workspace" },
                    "name": { "type": "string", "description": "File name (e.g. 'main.rs', 'index.ts')" },
                    "content": { "type": "string", "description": "File content to write" }
                },
                "required": ["project_id", "parent_path", "name", "content"]
            }),
        },
        // ── Savings / Token Tracking Tools ──
        ToolDefinition {
            name: "nexus_savings_stats".to_string(),
            description: "Get cumulative token and cost savings statistics: total tokens saved, cost saved (USD), per-day/week/month/year breakdown, average per interaction, and recent interactions. Real data from the database — no estimates.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        ToolDefinition {
            name: "nexus_savings_report".to_string(),
            description: "Get a full savings report: aggregate stats PLUS per-model cost breakdown for all 21 supported LLMs (how much the saved tokens would have cost with each model's input price). Use this to answer 'how much money did Nexus save me?'".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        ToolDefinition {
            name: "nexus_savings_per_model".to_string(),
            description: "Calculate savings for a specific LLM model: how much the saved tokens would have cost with that model's input price. Model names are case-insensitive, e.g. 'GPT-5.6 Terra', 'deepseek v4 flash', 'Claude Opus 5'.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "model": { "type": "string", "description": "Model display name (e.g. 'GPT-5.6 Terra', 'DeepSeek V4 Flash')" }
                },
                "required": ["model"]
            }),
        },
        // ── Project Tools ──
        ToolDefinition {
            name: "nexus_projects".to_string(),
            description: "List all projects (entities with type Project) in the knowledge graph. Use this to enumerate projects and get their IDs for workspace/project-scoped operations.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        ToolDefinition {
            name: "nexus_project_entities".to_string(),
            description: "Get all entities linked to a project via relationships, plus the relationships themselves. Use this to see what a project contains (documents, people, decisions, etc.).".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "Project entity UUID" }
                },
                "required": ["project_id"]
            }),
        },
        ToolDefinition {
            name: "nexus_project_memories".to_string(),
            description: "Get all memory records linked to a specific project. Use this to list the memories saved in a project's space.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "Project entity UUID" }
                },
                "required": ["project_id"]
            }),
        },
        ToolDefinition {
            name: "nexus_link_project_entity".to_string(),
            description: "Link an entity to a project by creating a relationship (default type: Uses). Use this to attach documents, people, decisions and other entities to a project.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "Project entity UUID" },
                    "entity_id": { "type": "string", "description": "Entity UUID to link to the project" },
                    "relationship_type": { "type": "string", "description": "Relationship type (default: Uses)", "default": "Uses" },
                    "weight": { "type": "number", "description": "Relationship weight (default: 0.8)", "default": 0.8 }
                },
                "required": ["project_id", "entity_id"]
            }),
        },
        // ── Workspace CRUD Tools ──
        ToolDefinition {
            name: "nexus_workspace_rename".to_string(),
            description: "Rename a workspace entry (file or folder) — renames on disk AND updates the workspace database, including all children. Returns the new absolute path.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "Project entity UUID" },
                    "old_path": { "type": "string", "description": "Current absolute path of the entry" },
                    "new_name": { "type": "string", "description": "New name (file/folder name only, not a full path)" }
                },
                "required": ["project_id", "old_path", "new_name"]
            }),
        },
        ToolDefinition {
            name: "nexus_workspace_move".to_string(),
            description: "Move a workspace entry (file or folder) to another directory — moves on disk (with cross-filesystem fallback) AND updates the workspace database. Returns the new absolute path.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "Project entity UUID" },
                    "source_path": { "type": "string", "description": "Absolute path of the entry to move" },
                    "dest_dir": { "type": "string", "description": "Absolute path of the destination directory" }
                },
                "required": ["project_id", "source_path", "dest_dir"]
            }),
        },
        ToolDefinition {
            name: "nexus_workspace_delete".to_string(),
            description: "Delete a workspace entry (file or folder) — deletes from disk AND removes it from the workspace database (including all descendants). Irreversible.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "Project entity UUID" },
                    "file_path": { "type": "string", "description": "Absolute path of the entry to delete" }
                },
                "required": ["project_id", "file_path"]
            }),
        },
        ToolDefinition {
            name: "nexus_workspace_remove".to_string(),
            description: "Remove an entry from the workspace database ONLY — does NOT delete the file/folder from disk. Use this to un-register a file from a project without touching the filesystem.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "Project entity UUID" },
                    "file_path": { "type": "string", "description": "Absolute path of the entry to remove from the workspace DB" }
                },
                "required": ["project_id", "file_path"]
            }),
        },
        ToolDefinition {
            name: "nexus_workspace_check_stale".to_string(),
            description: "Check all projects for stale folders — returns the list of project_ids whose ALL workspace root directories no longer exist on disk. Use this to detect dead projects before cleanup.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        // ── File Management Tools (standalone, outside workspace) ──
        ToolDefinition {
            name: "nexus_rename_file".to_string(),
            description: "Rename a file or folder on disk (not tied to a workspace project). Returns the new absolute path.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "old_path": { "type": "string", "description": "Current absolute path" },
                    "new_name": { "type": "string", "description": "New name (file/folder name only, not a full path)" }
                },
                "required": ["old_path", "new_name"]
            }),
        },
        ToolDefinition {
            name: "nexus_delete_folder".to_string(),
            description: "Recursively delete a folder on disk (not tied to a workspace project). Irreversible.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "folder_path": { "type": "string", "description": "Absolute path of the folder to delete" }
                },
                "required": ["folder_path"]
            }),
        },
        ToolDefinition {
            name: "nexus_scan_folder".to_string(),
            description: "Scan a folder on disk and return its file tree (FileEntry with children). Use this to inspect a directory before indexing or linking it to a project.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "folder_path": { "type": "string", "description": "Absolute path of the folder to scan" }
                },
                "required": ["folder_path"]
            }),
        },
        // ── Entity / AI / DB Tools ──
        ToolDefinition {
            name: "nexus_entity_metadata".to_string(),
            description: "Get the metadata map of an entity (key/value pairs stored on the entity). Returns an empty object if the entity has no metadata.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Entity UUID" }
                },
                "required": ["id"]
            }),
        },
        ToolDefinition {
            name: "nexus_savings_record".to_string(),
            description: "Record a measured token-savings event manually (baseline vs context usage). Use this to log a savings measurement that the UI would normally record automatically.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "baseline_tokens": { "type": "integer", "description": "Baseline token count" },
                    "context_tokens": { "type": "integer", "description": "Context token count" },
                    "entities_count": { "type": "integer", "description": "Entities included", "default": 0 },
                    "memories_count": { "type": "integer", "description": "Memories included", "default": 0 },
                    "relationships_count": { "type": "integer", "description": "Relationships included", "default": 0 },
                    "candidate_entities": { "type": "integer", "description": "Candidate entities filtered", "default": 0 },
                    "candidate_memories": { "type": "integer", "description": "Candidate memories filtered", "default": 0 },
                    "query": { "type": "string", "description": "Query text", "default": "" },
                    "intent_type": { "type": "string", "description": "Detected intent type", "default": "unknown" },
                    "latency_ms": { "type": "integer", "description": "Context build latency in milliseconds", "default": 0 },
                    "precision": { "type": "number", "description": "Precision of collected context (included/considered), 0..1", "default": 0 },
                    "used_fragments": { "type": "integer", "description": "Fragments actually used in the final answer", "default": 0 },
                    "irrelevant_fragments": { "type": "integer", "description": "Fragments dropped as below the relevance floor", "default": 0 },
                    "manual_context": { "type": "integer", "description": "1 if the user added context manually this round", "default": 0 }
                },
                "required": ["baseline_tokens", "context_tokens"]
            }),
        },
        ToolDefinition {
            name: "nexus_ai_models".to_string(),
            description: "List all available LLM models (via the opencode CLI). Pass free_only=true to list only free models. Use this to discover which models can be selected for AI features.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "free_only": { "type": "boolean", "description": "Only list free models", "default": false }
                },
            }),
        },
        ToolDefinition {
            name: "nexus_db_stats".to_string(),
            description: "Get database statistics: memory count, entity count, relationship count, commit count, snapshot count, and DB file size. Use this for health/status reports.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        // ── Config Tools ──
        ToolDefinition {
            name: "nexus_config_get".to_string(),
            description: "Get configuration values. Pass a key to read a single value, or omit key to list ALL configuration entries. Use this to inspect app settings.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "Config key (optional — omit to list all)" }
                },
            }),
        },
        ToolDefinition {
            name: "nexus_config_set".to_string(),
            description: "Set a configuration value (creates or updates the key). Use this to change app settings programmatically.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "Config key" },
                    "value": { "type": "string", "description": "Config value" }
                },
                "required": ["key", "value"]
            }),
        },
        // ── Memory Lifecycle Tools ──
        ToolDefinition {
            name: "nexus_memory_set_state".to_string(),
            description: "Set the trust state of a memory explicitly: Current, Inferred, Superseded, or Conflicted. Use this to mark a memory as outdated, disputed, or re-verified.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Memory UUID" },
                    "state": { "type": "string", "description": "New state: Current | Inferred | Superseded | Conflicted", "enum": ["Current", "Inferred", "Superseded", "Conflicted"] }
                },
                "required": ["id", "state"]
            }),
        },
        ToolDefinition {
            name: "nexus_memory_confirm".to_string(),
            description: "Mark a memory as explicitly confirmed by a human. The memory state becomes UserConfirmed with a timestamp. Use this to lock in a verified fact.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Memory UUID" },
                    "by": { "type": "string", "description": "Who confirmed it (optional)" }
                },
                "required": ["id"]
            }),
        },
        ToolDefinition {
            name: "nexus_memory_feedback".to_string(),
            description: "Record user feedback on a memory: useful, irrelevant, or wrong. One vote per memory — voting the same kind again removes the vote, a different kind switches it. Optionally explain why in 'note'; the explanation is kept and used by the copilot to understand what is right or wrong about the memory. A 'wrong' verdict also marks the memory Conflicted so it stops being trusted as-is.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Memory UUID" },
                    "kind": { "type": "string", "description": "Feedback kind", "enum": ["useful", "irrelevant", "wrong"] },
                    "note": { "type": "string", "description": "Optional explanation of why this feedback was given" }
                },
                "required": ["id", "kind"]
            }),
        },
        ToolDefinition {
            name: "nexus_memory_supersede".to_string(),
            description: "Replace an outdated memory with a newer one. The old memory is marked Superseded (never deleted), and a new Current record is created with the new title/content.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "old_id": { "type": "string", "description": "UUID of the memory to replace" },
                    "new_title": { "type": "string", "description": "Title of the new memory" },
                    "new_content": { "type": "string", "description": "Content of the new memory" },
                    "author": { "type": "string", "description": "Author of the new memory (optional)" }
                },
                "required": ["old_id", "new_title", "new_content"]
            }),
        },
        ToolDefinition {
            name: "nexus_lifecycle_overview".to_string(),
            description: "Get the memory trust lifecycle overview: how many memories are Current, UserConfirmed, Inferred, Superseded, and Conflicted. Use this for a memory-health dashboard.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        // ── Entity Resolution Tools ──
        ToolDefinition {
            name: "nexus_find_duplicates".to_string(),
            description: "Scan the knowledge graph for duplicate entities (exact + normalized + fuzzy name match). Returns groups of 2+ entities that look like the same thing, with a bestId merge target per group. Use this before merging to review what would be combined.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "min_score": { "type": "number", "description": "Minimum Dice similarity (default 0.78). Lower finds more (noisier) groups, higher finds only strong matches.", "default": 0.78 }
                },
            }),
        },
        ToolDefinition {
            name: "nexus_merge_entities".to_string(),
            description: "Merge duplicate entities into one canonical node. The primary is kept; every id in duplicates is merged into it (metadata combined, relationships redirected, duplicates marked Merged). Idempotent.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "primary": { "type": "string", "description": "UUID of the entity to keep (use bestId from nexus_find_duplicates)" },
                    "duplicates": { "type": "array", "items": { "type": "string" }, "description": "UUIDs of the entities to merge into primary" }
                },
                "required": ["primary", "duplicates"]
            }),
        },
        // ── Product Metrics Tools ──
        ToolDefinition {
            name: "nexus_product_metrics".to_string(),
            description: "Get product metrics that prove Nexus' value: share of queries without manual context, average context precision, used/irrelevant fragments, token savings vs baseline, average build latency, stale memories, memory fixes, and cross-session memory reuse. Use this to answer 'does Nexus actually help?' with measured data.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        // ── Project Knowledge Base Tools (RAG / AGENTS.md / skills) ──
        ToolDefinition {
            name: "nexus_docs_import".to_string(),
            description: "Import all .md/.markdown/.txt files from a folder into the project knowledge base (RAG corpus). Idempotent: unchanged files are skipped, changed files are re-indexed, files removed from disk are pruned. Use this to make a project's documentation searchable by the AI.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "folder_path": { "type": "string", "description": "Absolute path of the folder to import" }
                },
                "required": ["folder_path"]
            }),
        },
        ToolDefinition {
            name: "nexus_docs_list".to_string(),
            description: "List imported project documents (RAG corpus), newest first. Use this to see what documentation is already indexed before searching.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "description": "Max results (default 100)" }
                },
            }),
        },
        ToolDefinition {
            name: "nexus_docs_search".to_string(),
            description: "Search the imported project documentation (RAG corpus) by a query. Combines keyword overlap with semantic similarity (ONNX embeddings when available). Returns matching documents with relevance scores so the AI can answer questions about the project's own docs.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "limit": { "type": "integer", "description": "Max results (default 10)" }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "nexus_agents_read".to_string(),
            description: "Read the project's AGENTS.md instruction file (or another agents file by name). The content is already injected into context packages automatically, but use this to see the exact rules the AI is expected to follow.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Agents file name (default: AGENTS.md)" }
                },
            }),
        },
        ToolDefinition {
            name: "nexus_agents_generate".to_string(),
            description: "Generate an AGENTS.md from live Nexus data (modules, commands, knowledge base state) and store it as the active instruction file. The 'documentation skill': use this to create or refresh project instructions without writing them by hand.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        ToolDefinition {
            name: "nexus_skills_list".to_string(),
            description: "List all registered skills (runnable commands like JS scripts) with their descriptions. Skills are the lightweight alternative to MCP tools — an agent reads the list, picks the relevant one, and runs only it instead of carrying every tool in context.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        ToolDefinition {
            name: "nexus_skills_run".to_string(),
            description: "Run a registered skill by name with optional arguments. Captures stdout/stderr with a 30-second timeout. Use this to execute a project script or automation without loading the full MCP tool surface.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Skill name" },
                    "args": { "type": "array", "items": { "type": "string" }, "description": "Arguments passed to the skill" }
                },
                "required": ["name"]
            }),
        },
        // ── Code Graph Tools (structure over source files) ──
        ToolDefinition {
            name: "nexus_code_import".to_string(),
            description: "Index a folder of source files into the code graph. Symbols (classes, functions, structs, traits) are extracted with the built-in language parsers, and dependency edges (import / require / use / #include / mod) are recorded. Use this to let the AI answer structural questions about a codebase: what depends on what, where a symbol is defined.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "folder_path": { "type": "string", "description": "Absolute path of the folder to index" }
                },
                "required": ["folder_path"]
            }),
        },
        ToolDefinition {
            name: "nexus_code_list".to_string(),
            description: "List indexed source files in the code graph, newest first. Use this to see what code has been indexed before searching symbols.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "description": "Max results (default 100)" }
                },
            }),
        },
        ToolDefinition {
            name: "nexus_code_search".to_string(),
            description: "Search symbols (classes, functions, structs, traits, interfaces) by name across all indexed source files. Returns the defining file and language. Use this to locate where something is defined in the project.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Symbol name or substring" },
                    "limit": { "type": "integer", "description": "Max results (default 20)" }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "nexus_code_deps".to_string(),
            description: "Return the dependencies of one indexed source file (by path): what it imports, requires, includes or uses, with internal/external classification. Use this to understand a file's connections in the code graph.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path of the indexed file" }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "nexus_code_dependents".to_string(),
            description: "Return the files in the code graph that depend on the given target (reverse edges, internal only). Use this to answer 'what would be affected by changing X?'".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Dependency target (module or file)" }
                },
                "required": ["target"]
            }),
        },
        // ── Memory Radar Tools (proactive recall) ──
        ToolDefinition {
            name: "nexus_radar_snapshot".to_string(),
            description: "Proactive memory radar: scans the whole memory pool and returns what needs attention right now — unresolved conflicts, memories expiring soon, inferred memories never confirmed by a human, and important memories created or changed since the last radar scan. Use this at the start of a session (or when opening a project) to see what the user should review, instead of waiting for a query. Optionally pass markSeen=true to advance the radar checkpoint to now so the next scan only reports what changed afterwards.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "markSeen": { "type": "boolean", "description": "Advance the scan checkpoint to now after building the snapshot (default false)" }
                },
            }),
        },
        // ── Team Memory Tools (shared trusted layer) ──
        ToolDefinition {
            name: "nexus_team_add_member".to_string(),
            description: "Add a new member to the team roster. The team roster powers the trusted decision layer (who confirmed what, what went stale, what is in conflict). Role is one of admin, member, viewer (default member).".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Member name (must be unique)" },
                    "role": { "type": "string", "description": "admin | member | viewer (default member)" }
                },
                "required": ["name"]
            }),
        },
        ToolDefinition {
            name: "nexus_team_list_members".to_string(),
            description: "List all members of the team roster with their roles and active flags.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        ToolDefinition {
            name: "nexus_team_update_member".to_string(),
            description: "Update a team member's role and/or active flag by id.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Member id" },
                    "role": { "type": "string", "description": "admin | member | viewer" },
                    "active": { "type": "boolean", "description": "Whether the member is active" }
                },
                "required": ["id"]
            }),
        },
        ToolDefinition {
            name: "nexus_team_remove_member".to_string(),
            description: "Remove a team member from the roster by id.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Member id" }
                },
                "required": ["id"]
            }),
        },
        ToolDefinition {
            name: "nexus_team_overview".to_string(),
            description: "The trusted decision layer of the team: who confirmed which decision, what went stale (superseded), what is in conflict, and per-member activity (authored/confirmed/updated counts). This is the answer to 'what does the team actually know and agree on' — teams cannot get this from chat history.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        // ── Audit Memory Tools (decision chain / compliance) ──
        ToolDefinition {
            name: "nexus_audit_trail".to_string(),
            description: "Reconstruct the full decision chain for one memory — the answer to 'why did we decide this?'. Returns the decision context (reason), the alternatives that were considered and rejected, who confirmed the decision and when, which memory it superseded and which replaced it, and the full version history (who changed what, with diff reasons). Use this for compliance questions like 'why did we choose PostgreSQL in March?' — prove the team knew and why it decided so.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "memoryId": { "type": "string", "description": "Memory id to reconstruct the audit trail for" }
                },
                "required": ["memoryId"]
            }),
        },
        ToolDefinition {
            name: "nexus_audit_add_event".to_string(),
            description: "Append a raw event to a memory's decision journal: Created, Confirmed, Superseded or Note. Every auditable action on a memory gets one row so the full chain 'why did we decide this' can be reconstructed. actor is who performed the action (member / user / system). For Superseded events pass relatedMemoryId pointing at the memory that replaced it.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "memoryId": { "type": "string", "description": "Memory id the event belongs to" },
                    "eventType": { "type": "string", "description": "Created | Confirmed | Superseded | Note" },
                    "actor": { "type": "string", "description": "Who performed the action" },
                    "detail": { "type": "string", "description": "Optional free text" },
                    "relatedMemoryId": { "type": "string", "description": "For Superseded: the memory that replaced this one" }
                },
                "required": ["memoryId", "eventType", "actor"]
            }),
        },
        ToolDefinition {
            name: "nexus_audit_alternative".to_string(),
            description: "Record that an alternative was considered for a decision (and rejected). Appends an Alternative event with { title, reason } to the memory's decision journal, so the compliance chain shows which options were weighed and why they lost.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "memoryId": { "type": "string", "description": "Memory id the decision belongs to" },
                    "title": { "type": "string", "description": "The alternative that was considered (e.g. MySQL)" },
                    "reason": { "type": "string", "description": "Why it was not chosen (e.g. license costs)" },
                    "actor": { "type": "string", "description": "Who considered it" }
                },
                "required": ["memoryId", "title", "reason", "actor"]
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
        "nexus_list_memories" => match copilot::parse_command("/memories") {
            Some(cmd) => copilot::execute_command(&cmd).await,
            None => unreachable!(),
        },
        "nexus_get_memory" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand {
                name: "memory".into(),
                args: vec![id.into()],
            };
            copilot::execute_command(&cmd).await
        }
        "nexus_create_memory" => {
            let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let content = args
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or(title);
            let cmd_args = vec![title.to_string(), content.to_string()];
            let cmd = ParsedCommand {
                name: "create-memory".into(),
                args: cmd_args,
            };
            copilot::execute_command(&cmd).await
        }
        "nexus_update_memory" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand {
                name: "update-memory".into(),
                args: vec![id.into(), content.into()],
            };
            copilot::execute_command(&cmd).await
        }
        "nexus_delete_memory" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand {
                name: "delete-memory".into(),
                args: vec![id.into()],
            };
            copilot::execute_command(&cmd).await
        }
        "nexus_search_memories" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand {
                name: "search".into(),
                args: vec![query.to_string()],
            };
            copilot::execute_command(&cmd).await
        }
        "nexus_graph_stats" => match copilot::parse_command("/graph") {
            Some(cmd) => copilot::execute_command(&cmd).await,
            None => unreachable!(),
        },
        "nexus_get_entity" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand {
                name: "entity".into(),
                args: vec![id.into()],
            };
            copilot::execute_command(&cmd).await
        }
        "nexus_create_entity" => {
            let et = args
                .get("entity_type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand {
                name: "create-entity".into(),
                args: vec![et.into(), title.into()],
            };
            copilot::execute_command(&cmd).await
        }
        "nexus_update_entity" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand {
                name: "update-entity".into(),
                args: vec![id.into(), title.into()],
            };
            copilot::execute_command(&cmd).await
        }
        "nexus_delete_entity" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand {
                name: "delete-entity".into(),
                args: vec![id.into()],
            };
            copilot::execute_command(&cmd).await
        }
        "nexus_link_entities" => {
            let source = args.get("source_id").and_then(|v| v.as_str()).unwrap_or("");
            let target = args.get("target_id").and_then(|v| v.as_str()).unwrap_or("");
            let rel_type = args
                .get("relationship_type")
                .and_then(|v| v.as_str())
                .unwrap_or("RelatedTo");
            let weight = args
                .get("weight")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.8)
                .to_string();
            let cmd = ParsedCommand {
                name: "link".into(),
                args: vec![source.into(), target.into(), rel_type.into(), weight],
            };
            copilot::execute_command(&cmd).await
        }
        "nexus_unlink_entities" => {
            let id = args
                .get("relationship_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let cmd = ParsedCommand {
                name: "unlink".into(),
                args: vec![id.into()],
            };
            copilot::execute_command(&cmd).await
        }
        "nexus_build_context" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = ParsedCommand {
                name: "context".into(),
                args: vec![query.to_string()],
            };
            copilot::execute_command(&cmd).await
        }
        "nexus_build_context_for_entity" => {
            let entity_id = args.get("entity_id").and_then(|v| v.as_str()).unwrap_or("");
            let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(2) as u32;
            let cmd = ParsedCommand {
                name: "entity_context".into(),
                args: vec![entity_id.to_string(), depth.to_string()],
            };
            copilot::execute_command(&cmd).await
        }
        "nexus_stats" => match copilot::parse_command("/stats") {
            Some(cmd) => copilot::execute_command(&cmd).await,
            None => unreachable!(),
        },
        "nexus_health" => match copilot::parse_command("/health") {
            Some(cmd) => copilot::execute_command(&cmd).await,
            None => unreachable!(),
        },
        "nexus_settings" => match copilot::parse_command("/settings") {
            Some(cmd) => copilot::execute_command(&cmd).await,
            None => unreachable!(),
        },
        "nexus_timeline" => match copilot::parse_command("/timeline") {
            Some(cmd) => copilot::execute_command(&cmd).await,
            None => unreachable!(),
        },
        // ── Enhanced Intelligence Tools ──
        "nexus_parse_markdown" => {
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            match copilot::parse_and_build_graph(text).await {
                Ok(result) => CopilotResponse::ok(
                    format!(
                        "Parsed markdown: {} entities, {} relationships created",
                        result.0.len(),
                        result.1.len()
                    ),
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
                    format!(
                        "Context built for '{}': {} entities, {} relationships, {} memories, {} keywords, temporal: {:?}",
                        query,
                        result.entities.len(),
                        result.relationships.len(),
                        result.memory_records.len(),
                        result.user_intent.keywords.len(),
                        result.user_intent.temporal
                    ),
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
                    format!(
                        "Found {} recent memories (last {} days)",
                        memories.len(),
                        days
                    ),
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
            let threshold = args
                .get("threshold")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.7);
            match copilot::get_important_memories(threshold).await {
                Ok(memories) => CopilotResponse::ok(
                    format!(
                        "Found {} important memories (threshold: {})",
                        memories.len(),
                        threshold
                    ),
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
                format!(
                    "Analyzed text: {} keywords, temporal: {:?}, intent: {:?}",
                    keywords.len(),
                    temporal,
                    intent.intent_type
                ),
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
            let types_to_scan: Vec<crate::core::graph::entity_types::EntityType> =
                if let Some(et_str) = entity_type_filter {
                    vec![crate::core::graph::entity_types::EntityType::from(et_str)]
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
                        if all_entities.len() >= limit {
                            break;
                        }
                        all_entities.push(serde_json::json!({
                            "id": e.id.as_str(),
                            "type": e.entity_type.as_str(),
                            "title": e.title,
                            "status": format!("{:?}", e.status),
                            "created_at": e.created_at.to_rfc3339(),
                        }));
                    }
                }
                if all_entities.len() >= limit {
                    break;
                }
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
            let relationship = args
                .get("relationship")
                .and_then(|v| v.as_str())
                .unwrap_or("Related");
            let weight = args.get("weight").and_then(|v| v.as_f64()).unwrap_or(1.0);
            match copilot::link_memory_entity(memory_id, entity_id, relationship, weight).await {
                Ok(_) => CopilotResponse::ok(
                    format!(
                        "Linked memory {} to entity {} ({})",
                        memory_id, entity_id, relationship
                    ),
                    None,
                ),
                Err(e) => CopilotResponse::err(format!("Error: {}", e)),
            }
        }
        "nexus_unlink_memory_entity" => {
            let memory_id = args.get("memory_id").and_then(|v| v.as_str()).unwrap_or("");
            let entity_id = args.get("entity_id").and_then(|v| v.as_str()).unwrap_or("");
            let relationship = args
                .get("relationship")
                .and_then(|v| v.as_str())
                .unwrap_or("Related");
            match copilot::unlink_memory_entity(memory_id, entity_id, relationship).await {
                Ok(_) => CopilotResponse::ok(
                    format!(
                        "Unlinked memory {} from entity {} ({})",
                        memory_id, entity_id, relationship
                    ),
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
        // ── Workspace Tools ──
        "nexus_add_to_workspace" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let paths: Vec<String> = args
                .get("paths")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            match crate::commands::workspace::add_to_workspace(project_id.to_string(), paths).await
            {
                Ok(tree) => {
                    let count = tree
                        .as_ref()
                        .and_then(|t| t.children.as_ref())
                        .map(|c| c.len())
                        .unwrap_or(0);
                    CopilotResponse::ok(
                        format!("Added files to workspace: {} entries in tree", count),
                        Some(serde_json::json!({ "tree": tree })),
                    )
                }
                Err(e) => CopilotResponse::err(format!("Workspace error: {}", e)),
            }
        }
        "nexus_get_workspace" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match crate::commands::workspace::get_workspace_tree(project_id.to_string()).await {
                Ok(tree) => {
                    let count = tree
                        .as_ref()
                        .and_then(|t| t.children.as_ref())
                        .map(|c| c.len())
                        .unwrap_or(0);
                    CopilotResponse::ok(
                        format!("Workspace has {} top-level entries", count),
                        Some(serde_json::json!({ "tree": tree })),
                    )
                }
                Err(e) => CopilotResponse::err(format!("Workspace error: {}", e)),
            }
        }
        "nexus_sync_workspace" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match crate::commands::workspace::sync_workspace(project_id.to_string()).await {
                Ok(result) => {
                    let count = result
                        .tree
                        .as_ref()
                        .and_then(|t| t.children.as_ref())
                        .map(|c| c.len())
                        .unwrap_or(0);
                    CopilotResponse::ok(
                        format!(
                            "Workspace synced: {} top-level entries, stale: {}",
                            count, result.stale_found
                        ),
                        Some(
                            serde_json::json!({ "tree": result.tree, "stale_found": result.stale_found }),
                        ),
                    )
                }
                Err(e) => CopilotResponse::err(format!("Workspace sync error: {}", e)),
            }
        }
        // ── Project Tools ──
        "nexus_projects" => match crate::commands::graph::get_projects().await {
            Ok(projects) => CopilotResponse::ok(
                format!("Found {} projects", projects.len()),
                Some(serde_json::json!({ "projects": projects })),
            ),
            Err(e) => CopilotResponse::err(format!("Projects error: {}", e)),
        },
        "nexus_project_entities" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match crate::commands::graph::get_project_entities(project_id.to_string()).await {
                Ok(data) => CopilotResponse::ok(
                    format!(
                        "Project {} has {} linked entities and {} relationships",
                        project_id,
                        data.nodes.len(),
                        data.edges.len()
                    ),
                    Some(serde_json::json!({
                        "nodes": data.nodes,
                        "edges": data.edges,
                    })),
                ),
                Err(e) => CopilotResponse::err(format!("Project entities error: {}", e)),
            }
        }
        "nexus_project_memories" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match crate::commands::memory::get_project_memories(project_id.to_string()).await {
                Ok(memories) => CopilotResponse::ok(
                    format!(
                        "Project {} has {} linked memories",
                        project_id,
                        memories.len()
                    ),
                    Some(serde_json::json!({ "memories": memories })),
                ),
                Err(e) => CopilotResponse::err(format!("Project memories error: {}", e)),
            }
        }
        "nexus_link_project_entity" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let entity_id = args.get("entity_id").and_then(|v| v.as_str()).unwrap_or("");
            let rel_type = args
                .get("relationship_type")
                .and_then(|v| v.as_str())
                .map(String::from);
            let weight = args.get("weight").and_then(|v| v.as_f64());
            match crate::commands::graph::link_entity_to_project(
                project_id.to_string(),
                entity_id.to_string(),
                rel_type,
                weight,
            )
            .await
            {
                Ok(edge) => CopilotResponse::ok(
                    format!(
                        "Linked entity {} to project {} (relationship: {}, weight: {})",
                        entity_id, project_id, edge.relationship_type, edge.weight
                    ),
                    Some(serde_json::json!({ "relationship": edge })),
                ),
                Err(e) => CopilotResponse::err(format!("Link error: {}", e)),
            }
        }
        "nexus_workspace_rename" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let old_path = args.get("old_path").and_then(|v| v.as_str()).unwrap_or("");
            let new_name = args.get("new_name").and_then(|v| v.as_str()).unwrap_or("");
            match crate::commands::workspace::rename_workspace_entry(
                project_id.to_string(),
                old_path.to_string(),
                new_name.to_string(),
            )
            .await
            {
                Ok(new_abs) => CopilotResponse::ok(
                    format!("Renamed '{}' → '{}'", old_path, new_abs),
                    Some(serde_json::json!({ "new_path": new_abs })),
                ),
                Err(e) => CopilotResponse::err(format!("Rename error: {}", e)),
            }
        }
        "nexus_workspace_move" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let source_path = args
                .get("source_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let dest_dir = args.get("dest_dir").and_then(|v| v.as_str()).unwrap_or("");
            match crate::commands::workspace::move_workspace_entry(
                project_id.to_string(),
                source_path.to_string(),
                dest_dir.to_string(),
            )
            .await
            {
                Ok(new_abs) => CopilotResponse::ok(
                    format!("Moved '{}' → '{}'", source_path, new_abs),
                    Some(serde_json::json!({ "new_path": new_abs })),
                ),
                Err(e) => CopilotResponse::err(format!("Move error: {}", e)),
            }
        }
        "nexus_workspace_delete" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let file_path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
            match crate::commands::workspace::delete_workspace_entry(
                project_id.to_string(),
                file_path.to_string(),
            )
            .await
            {
                Ok(()) => CopilotResponse::ok(
                    format!("Deleted '{}' from disk and workspace", file_path),
                    None,
                ),
                Err(e) => CopilotResponse::err(format!("Delete error: {}", e)),
            }
        }
        "nexus_workspace_remove" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let file_path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
            match crate::commands::workspace::remove_from_workspace(
                project_id.to_string(),
                file_path.to_string(),
            )
            .await
            {
                Ok(()) => CopilotResponse::ok(
                    format!("Removed '{}' from workspace DB (disk untouched)", file_path),
                    None,
                ),
                Err(e) => CopilotResponse::err(format!("Remove error: {}", e)),
            }
        }
        "nexus_workspace_check_stale" => {
            match crate::commands::workspace::check_stale_projects().await {
                Ok(stale) => CopilotResponse::ok(
                    format!("Found {} stale projects", stale.len()),
                    Some(serde_json::json!({ "stale_project_ids": stale })),
                ),
                Err(e) => CopilotResponse::err(format!("Stale check error: {}", e)),
            }
        }
        "nexus_rename_file" => {
            let old_path = args.get("old_path").and_then(|v| v.as_str()).unwrap_or("");
            let new_name = args.get("new_name").and_then(|v| v.as_str()).unwrap_or("");
            match crate::commands::files::rename_file(old_path.to_string(), new_name.to_string())
                .await
            {
                Ok(new_abs) => CopilotResponse::ok(
                    format!("Renamed '{}' → '{}'", old_path, new_abs),
                    Some(serde_json::json!({ "new_path": new_abs })),
                ),
                Err(e) => CopilotResponse::err(format!("Rename error: {}", e)),
            }
        }
        "nexus_delete_folder" => {
            let folder_path = args
                .get("folder_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match crate::commands::files::delete_folder(folder_path.to_string()).await {
                Ok(()) => CopilotResponse::ok(
                    format!("Deleted folder: {}", folder_path),
                    Some(serde_json::json!({ "path": folder_path, "deleted": true })),
                ),
                Err(e) => CopilotResponse::err(format!("Delete folder error: {}", e)),
            }
        }
        "nexus_scan_folder" => {
            let folder_path = args
                .get("folder_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match crate::commands::files::scan_folder(folder_path.to_string()).await {
                Ok(entry) => CopilotResponse::ok(
                    format!("Scanned folder: {}", folder_path),
                    Some(serde_json::json!({ "entry": entry })),
                ),
                Err(e) => CopilotResponse::err(format!("Scan error: {}", e)),
            }
        }
        "nexus_entity_metadata" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            match crate::commands::graph::get_entity_metadata(id.to_string()).await {
                Ok(meta) => CopilotResponse::ok(
                    format!("Entity {} has {} metadata fields", id, meta.len()),
                    Some(serde_json::json!({ "metadata": meta })),
                ),
                Err(e) => CopilotResponse::err(format!("Metadata error: {}", e)),
            }
        }
        "nexus_savings_record" => {
            let baseline = args
                .get("baseline_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let context = args
                .get("context_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let entities = args
                .get("entities_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let memories = args
                .get("memories_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let rels = args
                .get("relationships_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let cand_entities = args
                .get("candidate_entities")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let cand_memories = args
                .get("candidate_memories")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let intent = args
                .get("intent_type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let latency_ms = args
                .get("latency_ms")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);
            let precision = args.get("precision").and_then(|v| v.as_f64());
            let used_fragments = args
                .get("used_fragments")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);
            let irrelevant_fragments = args
                .get("irrelevant_fragments")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);
            let manual_context = args
                .get("manual_context")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);
            match crate::commands::savings::record_savings_event(
                baseline,
                context,
                entities,
                memories,
                rels,
                cand_entities,
                cand_memories,
                query,
                intent,
                latency_ms,
                precision,
                used_fragments,
                irrelevant_fragments,
                manual_context,
            ) {
                Ok(()) => CopilotResponse::ok(
                    format!(
                        "Recorded savings event: {} baseline → {} context tokens",
                        baseline, context
                    ),
                    None,
                ),
                Err(e) => CopilotResponse::err(format!("Savings record error: {}", e)),
            }
        }
        "nexus_ai_models" => {
            let free_only = args
                .get("free_only")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            match crate::commands::ai::ai_list_models(Some(free_only)).await {
                Ok(models) => CopilotResponse::ok(
                    format!(
                        "Found {} models ({})",
                        models.len(),
                        if free_only { "free only" } else { "all" }
                    ),
                    Some(serde_json::json!({ "models": models })),
                ),
                Err(e) => CopilotResponse::err(format!("Models error: {}", e)),
            }
        }
        "nexus_db_stats" => match crate::commands::config::get_db_stats().await {
            Ok(stats) => CopilotResponse::ok(
                format!(
                    "DB: {} memories, {} entities, {} relationships, {} commits, {} snapshots",
                    stats.memory_count,
                    stats.entity_count,
                    stats.relationship_count,
                    stats.commit_count,
                    stats.snapshot_count
                ),
                Some(serde_json::json!({ "stats": stats })),
            ),
            Err(e) => CopilotResponse::err(format!("DB stats error: {}", e)),
        },
        "nexus_config_get" => {
            let key = args.get("key").and_then(|v| v.as_str());
            match key {
                Some(k) => match crate::commands::config::get_config(k.to_string()).await {
                    Ok(Some(value)) => CopilotResponse::ok(
                        format!("config.{} = {}", k, value),
                        Some(serde_json::json!({ "key": k, "value": value })),
                    ),
                    Ok(None) => CopilotResponse::ok(
                        format!("config.{} is not set", k),
                        Some(serde_json::json!({ "key": k, "value": null })),
                    ),
                    Err(e) => CopilotResponse::err(format!("Config get error: {}", e)),
                },
                None => match crate::commands::config::get_all_config().await {
                    Ok(entries) => CopilotResponse::ok(
                        format!("Found {} config entries", entries.len()),
                        Some(serde_json::json!({ "config": entries })),
                    ),
                    Err(e) => CopilotResponse::err(format!("Config list error: {}", e)),
                },
            }
        }
        "nexus_config_set" => {
            let key = args.get("key").and_then(|v| v.as_str()).unwrap_or("");
            let value = args.get("value").and_then(|v| v.as_str()).unwrap_or("");
            match crate::commands::config::set_config(key.to_string(), value.to_string()).await {
                Ok(()) => CopilotResponse::ok(
                    format!("Set config.{} = {}", key, value),
                    Some(serde_json::json!({ "key": key, "value": value })),
                ),
                Err(e) => CopilotResponse::err(format!("Config set error: {}", e)),
            }
        }
        // ── File Interpreter Tools ──
        "nexus_index_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let project_id = args.get("project_id").and_then(|v| v.as_str());
            match copilot::index_file(path, project_id).await {
                Ok(result) => CopilotResponse::ok(
                    format!(
                        "Indexed '{}': {} entities, {} sub-entities — {}",
                        result.file_name,
                        result.entities_created,
                        result.sub_entities_created,
                        result.summary
                    ),
                    Some(serde_json::json!({
                        "file_name": result.file_name,
                        "entities_created": result.entities_created,
                        "sub_entities_created": result.sub_entities_created,
                        "summary": result.summary,
                    })),
                ),
                Err(e) => CopilotResponse::err(format!("Index error: {}", e)),
            }
        }
        "nexus_index_folder" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let project_id = args.get("project_id").and_then(|v| v.as_str());
            match copilot::index_folder(path, project_id).await {
                Ok(result) => CopilotResponse::ok(
                    format!(
                        "Indexed folder '{}': {} files, {} entities, {} sub-entities",
                        result.folder_name,
                        result.total_files,
                        result.total_entities,
                        result.total_sub_entities
                    ),
                    Some(serde_json::json!({
                        "folder_name": result.folder_name,
                        "total_files": result.total_files,
                        "total_entities": result.total_entities,
                        "total_sub_entities": result.total_sub_entities,
                        "summaries": result.summaries,
                        "errors": result.errors,
                    })),
                ),
                Err(e) => CopilotResponse::err(format!("Folder index error: {}", e)),
            }
        }
        "nexus_read_file_content" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match copilot::read_file_content(path) {
                Ok(result) => CopilotResponse::ok(
                    format!(
                        "Read '{}': {} — {} sub-entities",
                        result.file_name,
                        result.summary,
                        result.sub_entities.len()
                    ),
                    Some(serde_json::json!({
                        "file_name": result.file_name,
                        "file_type": result.file_type,
                        "summary": result.summary,
                        "text_content": crate::core::text::truncate_with_suffix(
                            &result.text_content, 2000, "...(truncated)"
                        ),
                        "sub_entities": result.sub_entities.iter().map(|e| serde_json::json!({
                            "title": e.title,
                            "description": e.description,
                            "metadata": e.metadata,
                        })).collect::<Vec<_>>(),
                    })),
                ),
                Err(e) => CopilotResponse::err(format!("Read error: {}", e)),
            }
        }
        // ── File Operation Tools ──
        "nexus_create_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            match copilot::create_file(path, content) {
                Ok(()) => CopilotResponse::ok(
                    format!("Created file: {}", path),
                    Some(serde_json::json!({ "path": path, "created": true })),
                ),
                Err(e) => CopilotResponse::err(format!("Create error: {}", e)),
            }
        }
        "nexus_write_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            match copilot::write_file(path, content) {
                Ok(()) => CopilotResponse::ok(
                    format!("Written to: {}", path),
                    Some(serde_json::json!({ "path": path, "written": true })),
                ),
                Err(e) => CopilotResponse::err(format!("Write error: {}", e)),
            }
        }
        "nexus_create_folder" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match copilot::create_folder(path) {
                Ok(()) => CopilotResponse::ok(
                    format!("Created folder: {}", path),
                    Some(serde_json::json!({ "path": path, "created": true })),
                ),
                Err(e) => CopilotResponse::err(format!("Create folder error: {}", e)),
            }
        }
        "nexus_delete_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match copilot::delete_path(path) {
                Ok(()) => CopilotResponse::ok(
                    format!("Deleted: {}", path),
                    Some(serde_json::json!({ "path": path, "deleted": true })),
                ),
                Err(e) => CopilotResponse::err(format!("Delete error: {}", e)),
            }
        }
        "nexus_move_file" => {
            let source = args
                .get("source_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let new_path = args.get("new_path").and_then(|v| v.as_str());
            let dest_dir = args.get("dest_dir").and_then(|v| v.as_str());
            let new_name = args.get("new_name").and_then(|v| v.as_str());
            match copilot::move_file(source, new_path, dest_dir, new_name) {
                Ok(dest) => CopilotResponse::ok(
                    format!("Moved: {} → {}", source, dest),
                    Some(serde_json::json!({ "source": source, "destination": dest })),
                ),
                Err(e) => CopilotResponse::err(format!("Move error: {}", e)),
            }
        }
        "nexus_read_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match copilot::read_raw_file(path) {
                Ok(content) => {
                    let total = content.len();
                    let truncated = total > 4000;
                    let display_content = if truncated {
                        format!(
                            "{}...(truncated, {} total chars)",
                            crate::core::text::truncate_chars(&content, 4000),
                            total
                        )
                    } else {
                        content.clone()
                    };
                    CopilotResponse::ok(
                        format!("Read file: {}", path),
                        Some(serde_json::json!({
                            "path": path,
                            "content": display_content,
                            "truncated": truncated,
                            "total_chars": total,
                        })),
                    )
                }
                Err(e) => CopilotResponse::err(format!("Read error: {}", e)),
            }
        }
        "nexus_create_workspace_file" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let parent_path = args
                .get("parent_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            match copilot::create_workspace_file(project_id, parent_path, name, content).await {
                Ok(path) => CopilotResponse::ok(
                    format!("Created workspace file: {}", path),
                    Some(
                        serde_json::json!({ "project_id": project_id, "path": path, "name": name }),
                    ),
                ),
                Err(e) => CopilotResponse::err(format!("Workspace file error: {}", e)),
            }
        }
        // ── Savings / Token Tracking Tools ──
        "nexus_savings_stats" => match crate::commands::savings::get_savings_stats() {
            Ok(stats) => {
                let msg = format!(
                    "Savings: {} tokens saved (${:.2}) across {} interactions. Today: {} tokens (${:.2}). Week: {} tokens (${:.2}). Month: {} tokens (${:.2}).",
                    stats.total_tokens_saved,
                    stats.total_cost_saved_usd,
                    stats.total_interactions,
                    stats.tokens_saved_today,
                    stats.cost_saved_today,
                    stats.tokens_saved_week,
                    stats.cost_saved_week,
                    stats.tokens_saved_month,
                    stats.cost_saved_month,
                );
                CopilotResponse::ok(msg, Some(serde_json::to_value(&stats).unwrap_or_default()))
            }
            Err(e) => CopilotResponse::err(format!("Savings error: {}", e)),
        },
        "nexus_savings_report" => match crate::commands::savings::get_savings_report() {
            Ok(report) => {
                let msg = format!(
                    "Savings report: {} tokens saved (${:.2}) across {} interactions. Across 21 models the same tokens would cost from ${:.2} (DeepSeek V4 Flash) to ${:.2} (Claude Fable 5).",
                    report.stats.total_tokens_saved,
                    report.stats.total_cost_saved_usd,
                    report.stats.total_interactions,
                    report
                        .models
                        .iter()
                        .map(|m| m.cost_saved_usd)
                        .fold(f64::INFINITY, f64::min),
                    report
                        .models
                        .iter()
                        .map(|m| m.cost_saved_usd)
                        .fold(0.0, f64::max),
                );
                CopilotResponse::ok(msg, Some(serde_json::to_value(&report).unwrap_or_default()))
            }
            Err(e) => CopilotResponse::err(format!("Savings report error: {}", e)),
        },
        "nexus_savings_per_model" => {
            let model = args.get("model").and_then(|v| v.as_str()).unwrap_or("");
            if model.is_empty() {
                return CopilotResponse::err("Missing required parameter 'model'");
            }
            match crate::commands::savings::get_model_savings(model) {
                Ok(json) => {
                    let cost = json["cost_saved_usd"].as_f64().unwrap_or(0.0);
                    let tokens = json["total_tokens_saved"].as_u64().unwrap_or(0);
                    let msg = format!(
                        "Model '{}' ({}): ${:.2} saved on {} input tokens at ${:.2}/1M input.",
                        json["model"]["name"].as_str().unwrap_or(model),
                        json["model"]["company"].as_str().unwrap_or(""),
                        cost,
                        tokens,
                        json["model"]["input_per_m"].as_f64().unwrap_or(0.0),
                    );
                    CopilotResponse::ok(msg, Some(json))
                }
                Err(e) => CopilotResponse::err(e),
            }
        }
        // ── Memory Lifecycle Tools ──
        "nexus_memory_set_state" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let state = args.get("state").and_then(|v| v.as_str()).unwrap_or("");
            if id.is_empty() || state.is_empty() {
                return CopilotResponse::err("Missing required parameters 'id' and 'state'");
            }
            match crate::commands::lifecycle::memory_set_state(id.to_string(), state.to_string())
                .await
            {
                Ok(m) => {
                    let msg = format!("Memory {} is now {}. {}", id, m.memory_state, m.title);
                    CopilotResponse::ok(msg, Some(serde_json::to_value(&m).unwrap_or_default()))
                }
                Err(e) => CopilotResponse::err(format!("State error: {}", e)),
            }
        }
        "nexus_memory_confirm" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let by = args
                .get("by")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if id.is_empty() {
                return CopilotResponse::err("Missing required parameter 'id'");
            }
            match crate::commands::lifecycle::memory_confirm(id.to_string(), by).await {
                Ok(m) => {
                    let msg = format!(
                        "Memory {} confirmed by {}: {}",
                        id,
                        m.confirmed_by.as_deref().unwrap_or("user"),
                        m.title
                    );
                    CopilotResponse::ok(msg, Some(serde_json::to_value(&m).unwrap_or_default()))
                }
                Err(e) => CopilotResponse::err(format!("Confirm error: {}", e)),
            }
        }
        "nexus_memory_feedback" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let kind = args.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            if id.is_empty() || kind.is_empty() {
                return CopilotResponse::err("Missing required parameters 'id' and 'kind'");
            }
            let note = args
                .get("note")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            match crate::commands::lifecycle::memory_feedback(
                id.to_string(),
                kind.to_string(),
                note,
            )
            .await
            {
                Ok(m) => {
                    let msg = format!(
                        "Feedback '{}' recorded on memory {}. Useful: {}, irrelevant: {}, wrong: {}.{}",
                        kind,
                        id,
                        m.feedback.useful,
                        m.feedback.irrelevant,
                        m.feedback.wrong,
                        m.feedback
                            .note
                            .as_deref()
                            .map(|n| format!(" Note: {}", n))
                            .unwrap_or_default()
                    );
                    CopilotResponse::ok(msg, Some(serde_json::to_value(&m).unwrap_or_default()))
                }
                Err(e) => CopilotResponse::err(format!("Feedback error: {}", e)),
            }
        }
        "nexus_memory_supersede" => {
            let old_id = args.get("old_id").and_then(|v| v.as_str()).unwrap_or("");
            let new_title = args.get("new_title").and_then(|v| v.as_str()).unwrap_or("");
            let new_content = args
                .get("new_content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let author = args
                .get("author")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if old_id.is_empty() || new_title.is_empty() || new_content.is_empty() {
                return CopilotResponse::err(
                    "Missing required parameters 'old_id', 'new_title', 'new_content'",
                );
            }
            match crate::commands::lifecycle::memory_supersede(
                old_id.to_string(),
                new_title.to_string(),
                new_content.to_string(),
                author,
            )
            .await
            {
                Ok(m) => {
                    let msg = format!(
                        "Memory {} superseded by {} (state: {}).",
                        old_id, m.id, m.memory_state
                    );
                    CopilotResponse::ok(msg, Some(serde_json::to_value(&m).unwrap_or_default()))
                }
                Err(e) => CopilotResponse::err(format!("Supersede error: {}", e)),
            }
        }
        "nexus_lifecycle_overview" => {
            match crate::commands::lifecycle::get_lifecycle_overview().await {
                Ok(o) => {
                    let msg = format!(
                        "Memory lifecycle: {} current, {} user-confirmed, {} inferred, {} superseded, {} conflicted (total {}).",
                        o.current,
                        o.user_confirmed,
                        o.inferred,
                        o.superseded,
                        o.conflicted,
                        o.total
                    );
                    CopilotResponse::ok(msg, Some(serde_json::to_value(&o).unwrap_or_default()))
                }
                Err(e) => CopilotResponse::err(format!("Lifecycle error: {}", e)),
            }
        }
        // ── Entity Resolution Tools ──
        "nexus_find_duplicates" => {
            let min_score = args.get("min_score").and_then(|v| v.as_f64());
            match crate::commands::graph::find_duplicate_entities(min_score).await {
                Ok(groups) => {
                    let count: usize = groups
                        .iter()
                        .map(|g| g.entities.len().saturating_sub(1))
                        .sum();
                    let msg = format!(
                        "Found {} duplicate groups ({} entities that could be merged).",
                        groups.len(),
                        count
                    );
                    CopilotResponse::ok(
                        msg,
                        Some(serde_json::to_value(&groups).unwrap_or_default()),
                    )
                }
                Err(e) => CopilotResponse::err(format!("Duplicate scan error: {}", e)),
            }
        }
        "nexus_merge_entities" => {
            let primary = args.get("primary").and_then(|v| v.as_str()).unwrap_or("");
            let duplicates = args
                .get("duplicates")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if primary.is_empty() || duplicates.is_empty() {
                return CopilotResponse::err(
                    "Missing required parameters 'primary' and 'duplicates'",
                );
            }
            match crate::commands::graph::merge_entities(primary.to_string(), duplicates.clone())
                .await
            {
                Ok(node) => {
                    let msg = format!(
                        "Merged {} entities into '{}' (type: {}).",
                        duplicates.len() + 1,
                        node.title,
                        node.entity_type
                    );
                    CopilotResponse::ok(msg, Some(serde_json::to_value(&node).unwrap_or_default()))
                }
                Err(e) => CopilotResponse::err(format!("Merge error: {}", e)),
            }
        }
        // ── Product Metrics Tools ──
        "nexus_product_metrics" => match crate::commands::savings::get_product_metrics().await {
            Ok(m) => {
                let msg = format!(
                    "Product metrics: {} interactions. No-manual-context share: {:.0}%. Avg precision: {:.2}. Used fragments: {} ({}% of considered). Irrelevant fragments: {}. Tokens saved: {} (baseline {}). Avg latency: {:.0} ms. Stale memories: {}. Memory fixes: {}. Memories reused across sessions: {} of {}.",
                    m.total_interactions,
                    m.auto_context_share * 100.0,
                    m.avg_precision,
                    m.total_used_fragments,
                    m.used_fragment_share * 100.0,
                    m.total_irrelevant_fragments,
                    m.total_tokens_saved,
                    m.total_baseline_tokens,
                    m.avg_latency_ms,
                    m.stale_memories,
                    m.memory_fixes,
                    m.reused_memories,
                    m.total_memories_delivered,
                );
                CopilotResponse::ok(msg, Some(serde_json::to_value(&m).unwrap_or_default()))
            }
            Err(e) => CopilotResponse::err(format!("Product metrics error: {}", e)),
        },
        // ── Project Knowledge Base Tools (RAG / AGENTS.md / skills) ──
        "nexus_docs_import" => {
            let folder = args
                .get("folder_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if folder.is_empty() {
                return CopilotResponse::err("Missing required parameter 'folder_path'");
            }
            let repo = match crate::core::knowledge::documents::ProjectDocumentRepository::open() {
                Ok(r) => r,
                Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
            };
            let dir = std::path::PathBuf::from(folder);
            match crate::core::knowledge::documents::import_directory(&repo, &dir) {
                Ok(report) => {
                    let msg = format!(
                        "Docs import: scanned {}, imported/updated {}, unchanged {}, pruned {}, failed {}.",
                        report.scanned,
                        report.imported,
                        report.unchanged,
                        report.updated,
                        report.failed
                    );
                    CopilotResponse::ok(
                        msg,
                        Some(serde_json::to_value(&report).unwrap_or_default()),
                    )
                }
                Err(e) => CopilotResponse::err(format!("Import error: {}", e)),
            }
        }
        "nexus_docs_list" => {
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as u32;
            match crate::commands::knowledge::list_docs(Some(limit)).await {
                Ok(docs) => {
                    let msg = format!("{} project documents indexed.", docs.len());
                    CopilotResponse::ok(msg, Some(serde_json::to_value(&docs).unwrap_or_default()))
                }
                Err(e) => CopilotResponse::err(format!("List error: {}", e)),
            }
        }
        "nexus_docs_search" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as u32;
            if query.is_empty() {
                return CopilotResponse::err("Missing required parameter 'query'");
            }
            match crate::core::knowledge::documents::search_docs(query, limit) {
                Ok(hits) => {
                    let msg = format!("{} document(s) match '{}'.", hits.len(), query);
                    let json: serde_json::Value = hits
                        .iter()
                        .map(|h| {
                            serde_json::json!({
                                "path": h.document.path,
                                "title": h.document.title,
                                "score": h.score,
                                "doc_type": h.document.doc_type,
                                "updated_at": h.document.updated_at,
                                "content": h.document.content,
                            })
                        })
                        .collect();
                    CopilotResponse::ok(msg, Some(json))
                }
                Err(e) => CopilotResponse::err(format!("Search error: {}", e)),
            }
        }
        "nexus_agents_read" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("AGENTS.md");
            match crate::commands::knowledge::agents_read(Some(name.to_string())).await {
                Ok(Some(file)) => CopilotResponse::ok(
                    format!("Agents file '{}' ({} chars)", file.name, file.content.len()),
                    Some(serde_json::json!({
                        "name": file.name,
                        "content": file.content,
                        "path": file.path,
                        "updated_at": file.updated_at,
                    })),
                ),
                Ok(None) => CopilotResponse::err(format!(
                    "Agents file '{}' not found. Generate one with nexus_agents_generate.",
                    name
                )),
                Err(e) => CopilotResponse::err(format!("Read error: {}", e)),
            }
        }
        "nexus_agents_generate" => match crate::commands::knowledge::agents_generate().await {
            Ok(file) => CopilotResponse::ok(
                format!(
                    "AGENTS.md generated from live system data ({} chars) and stored as the active instruction file.",
                    file.content.len()
                ),
                Some(serde_json::json!({
                    "name": file.name,
                    "content": file.content,
                    "path": file.path,
                })),
            ),
            Err(e) => CopilotResponse::err(format!("Generate error: {}", e)),
        },
        "nexus_skills_list" => match crate::commands::knowledge::skills_list().await {
            Ok(skills) => {
                let msg = format!("{} skills registered.", skills.len());
                let json: serde_json::Value = skills
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "name": s.name,
                            "description": s.description,
                            "command": s.command,
                            "script_path": s.script_path,
                            "enabled": s.enabled,
                        })
                    })
                    .collect();
                CopilotResponse::ok(msg, Some(json))
            }
            Err(e) => CopilotResponse::err(format!("List error: {}", e)),
        },
        "nexus_skills_run" => {
            let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let argv = args
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if name.is_empty() {
                return CopilotResponse::err("Missing required parameter 'name'");
            }
            match crate::core::knowledge::skills::SkillRunner::run(name, &argv) {
                Ok(out) => {
                    let status = if out.success { "ok" } else { "failed" };
                    let msg = format!(
                        "Skill '{}' {} ({} ms, exit {:?}).\n\nstdout:\n{}\n\nstderr:\n{}",
                        name, status, out.duration_ms, out.exit_code, out.stdout, out.stderr
                    );
                    CopilotResponse::ok(msg, Some(serde_json::to_value(&out).unwrap_or_default()))
                }
                Err(e) => CopilotResponse::err(format!("Run error: {}", e)),
            }
        }
        // ── Code Graph Tools ──
        "nexus_code_import" => {
            let folder = args
                .get("folder_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if folder.is_empty() {
                return CopilotResponse::err("Missing required parameter 'folder_path'");
            }
            let repo = match crate::core::knowledge::code_graph::CodeGraphRepository::open() {
                Ok(r) => r,
                Err(e) => return CopilotResponse::err(format!("DB error: {}", e)),
            };
            let dir = std::path::PathBuf::from(folder);
            match crate::core::knowledge::code_graph::import_code_directory(&repo, &dir) {
                Ok(report) => {
                    let msg = format!(
                        "Code import: scanned {}, indexed/updated {}, unchanged {}, symbols {}, dependencies {}, pruned {}, failed {}.",
                        report.scanned,
                        report.indexed,
                        report.unchanged,
                        report.symbols,
                        report.dependencies,
                        report.pruned,
                        report.failed
                    );
                    CopilotResponse::ok(
                        msg,
                        Some(serde_json::to_value(&report).unwrap_or_default()),
                    )
                }
                Err(e) => CopilotResponse::err(format!("Import error: {}", e)),
            }
        }
        "nexus_code_list" => {
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as u32;
            match crate::commands::knowledge::code_list(Some(limit)).await {
                Ok(files) => {
                    let msg = format!("{} source files indexed.", files.len());
                    CopilotResponse::ok(msg, Some(serde_json::to_value(&files).unwrap_or_default()))
                }
                Err(e) => CopilotResponse::err(format!("List error: {}", e)),
            }
        }
        "nexus_code_search" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as u32;
            if query.is_empty() {
                return CopilotResponse::err("Missing required parameter 'query'");
            }
            match crate::commands::knowledge::code_search(query.to_string(), Some(limit)).await {
                Ok(hits) => {
                    let msg = format!("{} symbol(s) match '{}'.", hits.len(), query);
                    let json: serde_json::Value = hits
                        .iter()
                        .map(|h| {
                            serde_json::json!({
                                "name": h.symbol.name,
                                "kind": h.symbol.kind,
                                "signature": h.symbol.signature,
                                "file": h.file_path,
                                "language": h.file_language,
                            })
                        })
                        .collect();
                    CopilotResponse::ok(msg, Some(json))
                }
                Err(e) => CopilotResponse::err(format!("Search error: {}", e)),
            }
        }
        "nexus_code_deps" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if path.is_empty() {
                return CopilotResponse::err("Missing required parameter 'path'");
            }
            match crate::commands::knowledge::code_deps(path.to_string()).await {
                Ok(deps) => {
                    let msg = format!("{} dependencies for '{}'.", deps.len(), path);
                    let json: serde_json::Value = deps
                        .iter()
                        .map(|d| {
                            serde_json::json!({
                                "target": d.target,
                                "kind": d.kind,
                                "is_external": d.is_external,
                            })
                        })
                        .collect();
                    CopilotResponse::ok(msg, Some(json))
                }
                Err(e) => CopilotResponse::err(format!("Deps error: {}", e)),
            }
        }
        "nexus_code_dependents" => {
            let target = args.get("target").and_then(|v| v.as_str()).unwrap_or("");
            if target.is_empty() {
                return CopilotResponse::err("Missing required parameter 'target'");
            }
            match crate::commands::knowledge::code_dependents(target.to_string()).await {
                Ok(hits) => {
                    let msg = format!("{} file(s) depend on '{}'.", hits.len(), target);
                    let json: serde_json::Value = hits
                        .iter()
                        .map(|h| {
                            serde_json::json!({
                                "file": h.file_path,
                                "kind": h.kind,
                            })
                        })
                        .collect();
                    CopilotResponse::ok(msg, Some(json))
                }
                Err(e) => CopilotResponse::err(format!("Dependents error: {}", e)),
            }
        }
        "nexus_radar_snapshot" => {
            let mark_seen = args
                .get("markSeen")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let snapshot = if mark_seen {
                crate::commands::radar::radar_scan_and_seen().await
            } else {
                crate::commands::radar::get_radar_snapshot().await
            };
            match snapshot {
                Ok(s) => {
                    let items: Vec<serde_json::Value> = s
                        .items
                        .iter()
                        .map(|i| {
                            serde_json::json!({
                                "id": i.id,
                                "title": i.title,
                                "action": i.action,
                                "importance": i.importance,
                                "confidence": i.confidence,
                                "memory_state": i.memory_state,
                                "reason": i.reason,
                                "expires_at": i.expires_at,
                            })
                        })
                        .collect();
                    let msg = format!(
                        "Radar snapshot: {} item(s) need attention (attention score {}). Conflicts: {}, expiring: {}, inferred: {}, new since last scan: {}.",
                        s.items.len(),
                        s.attention_score,
                        s.counts.conflicted,
                        s.counts.expiring,
                        s.counts.inferred,
                        s.counts.new_since_last_scan,
                    );
                    CopilotResponse::ok(
                        msg,
                        Some(serde_json::json!({
                            "generated_at": s.generated_at,
                            "since": s.since,
                            "attention_score": s.attention_score,
                            "counts": s.counts,
                            "items": items,
                        })),
                    )
                }
                Err(e) => CopilotResponse::err(format!("Radar error: {}", e)),
            }
        }
        "nexus_team_add_member" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let role = args
                .get("role")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if name.trim().is_empty() {
                return CopilotResponse::err("Team add member error: name is required");
            }
            match crate::commands::team::team_add_member(name, role).await {
                Ok(m) => CopilotResponse::ok(
                    format!("Team member '{}' added with role {}.", m.name, m.role),
                    Some(
                        serde_json::json!({ "id": m.id, "name": m.name, "role": m.role, "active": m.active }),
                    ),
                ),
                Err(e) => CopilotResponse::err(format!("Team add member error: {}", e)),
            }
        }
        "nexus_team_list_members" => match crate::commands::team::team_list_members().await {
            Ok(members) => {
                let items: Vec<serde_json::Value> = members
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "id": m.id,
                            "name": m.name,
                            "role": m.role,
                            "active": m.active,
                        })
                    })
                    .collect();
                CopilotResponse::ok(
                    format!("Team roster: {} member(s).", members.len()),
                    Some(serde_json::json!({ "members": items })),
                )
            }
            Err(e) => CopilotResponse::err(format!("Team list members error: {}", e)),
        },
        "nexus_team_update_member" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let role = args
                .get("role")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let active = args.get("active").and_then(|v| v.as_bool());
            match crate::commands::team::team_update_member(id, role, active).await {
                Ok(m) => CopilotResponse::ok(
                    format!(
                        "Team member '{}' updated: role {}, active {}.",
                        m.name, m.role, m.active
                    ),
                    Some(
                        serde_json::json!({ "id": m.id, "name": m.name, "role": m.role, "active": m.active }),
                    ),
                ),
                Err(e) => CopilotResponse::err(format!("Team update member error: {}", e)),
            }
        }
        "nexus_team_remove_member" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            match crate::commands::team::team_remove_member(id).await {
                Ok(()) => CopilotResponse::ok("Team member removed.".to_string(), None),
                Err(e) => CopilotResponse::err(format!("Team remove member error: {}", e)),
            }
        }
        "nexus_team_overview" => match crate::commands::team::get_team_overview().await {
            Ok(o) => {
                let confirmed: Vec<serde_json::Value> = o
                    .confirmed_decisions
                    .iter()
                    .map(|d| {
                        serde_json::json!({
                            "memory_id": d.memory_id,
                            "title": d.title,
                            "by": d.by,
                            "at": d.at,
                        })
                    })
                    .collect();
                let superseded: Vec<serde_json::Value> = o
                    .superseded_decisions
                    .iter()
                    .map(|d| {
                        serde_json::json!({
                            "memory_id": d.memory_id,
                            "title": d.title,
                            "by": d.by,
                            "detail": d.detail,
                        })
                    })
                    .collect();
                let conflicted: Vec<serde_json::Value> = o
                    .conflicted
                    .iter()
                    .map(|d| {
                        serde_json::json!({
                            "memory_id": d.memory_id,
                            "title": d.title,
                            "by": d.by,
                        })
                    })
                    .collect();
                CopilotResponse::ok(
                    format!(
                        "Team overview: {} member(s), {} confirmed decision(s), {} superseded, {} in conflict.",
                        o.totals.members,
                        o.totals.confirmed,
                        o.totals.superseded,
                        o.totals.conflicted,
                    ),
                    Some(serde_json::json!({
                        "totals": o.totals,
                        "members": o.members,
                        "confirmed_decisions": confirmed,
                        "superseded_decisions": superseded,
                        "conflicted": conflicted,
                    })),
                )
            }
            Err(e) => CopilotResponse::err(format!("Team overview error: {}", e)),
        },
        "nexus_audit_trail" => {
            let memory_id = args
                .get("memoryId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if memory_id.trim().is_empty() {
                return CopilotResponse::err("Audit trail error: memoryId is required");
            }
            match crate::commands::audit::get_audit_trail(memory_id).await {
                Ok(t) => {
                    let summary = format!(
                        "Audit trail for '{}' ({}): {} alternative(s) considered, {} event(s), {} version(s). Confirmed by {}{}.",
                        t.title,
                        t.state,
                        t.alternatives.len(),
                        t.events.len(),
                        t.versions.len(),
                        t.confirmed_by
                            .clone()
                            .unwrap_or_else(|| "nobody".to_string()),
                        match &t.superseded_by {
                            Some(s) => format!("; superseded by {}", s),
                            None => String::new(),
                        },
                    );
                    CopilotResponse::ok(summary, Some(serde_json::to_value(&t).unwrap_or_default()))
                }
                Err(e) => CopilotResponse::err(format!("Audit trail error: {}", e)),
            }
        }
        "nexus_audit_add_event" => {
            let memory_id = args
                .get("memoryId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let event_type = args
                .get("eventType")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let actor = args
                .get("actor")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let detail = args
                .get("detail")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let related_memory_id = args
                .get("relatedMemoryId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if memory_id.trim().is_empty() || event_type.trim().is_empty() {
                return CopilotResponse::err(
                    "Audit add event error: memoryId and eventType are required",
                );
            }
            match crate::commands::audit::audit_add_event(
                memory_id,
                event_type,
                actor,
                detail,
                related_memory_id,
            )
            .await
            {
                Ok(e) => CopilotResponse::ok(
                    format!(
                        "{} event recorded for memory {}.",
                        e.event_type, e.memory_id
                    ),
                    Some(
                        serde_json::json!({ "id": e.id, "event_type": e.event_type, "created_at": e.created_at }),
                    ),
                ),
                Err(e) => CopilotResponse::err(format!("Audit add event error: {}", e)),
            }
        }
        "nexus_audit_alternative" => {
            let memory_id = args
                .get("memoryId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let reason = args
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let actor = args
                .get("actor")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if memory_id.trim().is_empty() || title.trim().is_empty() {
                return CopilotResponse::err(
                    "Audit alternative error: memoryId and title are required",
                );
            }
            match crate::commands::audit::audit_alternative(memory_id, title, reason, actor).await {
                Ok(e) => CopilotResponse::ok(
                    format!(
                        "Alternative considered and recorded for memory {}.",
                        e.memory_id
                    ),
                    Some(
                        serde_json::json!({ "id": e.id, "event_type": e.event_type, "created_at": e.created_at }),
                    ),
                ),
                Err(e) => CopilotResponse::err(format!("Audit alternative error: {}", e)),
            }
        }
        other => CopilotResponse::err(format!("Unknown tool: {}", other)),
    }
}

// ═══════════════════════════════════════════════════════════════
//  MCP server (stdio JSON-RPC)
// ═══════════════════════════════════════════════════════════════

fn ok_response(id: Option<serde_json::Value>, result: serde_json::Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id,
        result: Some(result),
        error: None,
    }
}

fn err_response(id: Option<serde_json::Value>, code: i64, msg: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id,
        result: None,
        error: Some(JsonRpcError { code, message: msg }),
    }
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
            ok_response(
                req.id,
                serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": capabilities,
                    "serverInfo": info,
                }),
            )
        }
        "notifications/initialized" => {
            // Per JSON-RPC 2.0 spec: notifications must NOT receive a response.
            return None;
        }
        "tools/list" => {
            let tools: Vec<serde_json::Value> = tool_definitions()
                .into_iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": t.input_schema,
                    })
                })
                .collect();
            ok_response(req.id, serde_json::json!({ "tools": tools }))
        }
        "tools/call" => {
            let tool_name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let arguments = req
                .params
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::json!({}));
            let resp = dispatch_tool(tool_name, &arguments).await;

            let content = if resp.success {
                let mut text = resp.message.clone();
                if let Some(data) = &resp.data {
                    text.push_str("\n\n");
                    text.push_str(&serde_json::to_string_pretty(data).unwrap_or_default());
                }
                vec![serde_json::json!({ "type": "text", "text": text })]
            } else {
                vec![
                    serde_json::json!({ "type": "text", "text": format!("Error: {}", resp.message) }),
                ]
            };

            ok_response(
                req.id,
                serde_json::json!({
                    "content": content,
                    "isError": !resp.success,
                }),
            )
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
                serde_json::json!({
                    "uri": "nexus://savings",
                    "name": "Token Savings Stats",
                    "description": "Cumulative token and cost savings (real data from the database)",
                    "mimeType": "application/json",
                }),
                serde_json::json!({
                    "uri": "nexus://savings-report",
                    "name": "Token Savings Report",
                    "description": "Savings stats plus per-model cost breakdown for all supported LLMs",
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
                        vec![
                            serde_json::json!({ "type": "text", "text": format!("Error: {}", resp.message) }),
                        ]
                    };
                    ok_response(
                        req.id,
                        serde_json::json!({ "contents": [{ "uri": uri, "mimeType": "application/json", "text": content[0]["text"] }] }),
                    )
                }
                "nexus://health" => {
                    let resp = dispatch_tool("nexus_health", &serde_json::json!({})).await;
                    let text = resp.message.clone();
                    ok_response(
                        req.id,
                        serde_json::json!({ "contents": [{ "uri": uri, "mimeType": "application/json", "text": text }] }),
                    )
                }
                "nexus://settings" => {
                    let resp = dispatch_tool("nexus_settings", &serde_json::json!({})).await;
                    let mut text = resp.message.clone();
                    if let Some(data) = &resp.data {
                        text.push_str("\n\n");
                        text.push_str(&serde_json::to_string_pretty(data).unwrap_or_default());
                    }
                    ok_response(
                        req.id,
                        serde_json::json!({ "contents": [{ "uri": uri, "mimeType": "application/json", "text": text }] }),
                    )
                }
                "nexus://savings" => {
                    let resp = dispatch_tool("nexus_savings_stats", &serde_json::json!({})).await;
                    let mut text = resp.message.clone();
                    if let Some(data) = &resp.data {
                        text.push_str("\n\n");
                        text.push_str(&serde_json::to_string_pretty(data).unwrap_or_default());
                    }
                    ok_response(
                        req.id,
                        serde_json::json!({ "contents": [{ "uri": uri, "mimeType": "application/json", "text": text }] }),
                    )
                }
                "nexus://savings-report" => {
                    let resp = dispatch_tool("nexus_savings_report", &serde_json::json!({})).await;
                    let mut text = resp.message.clone();
                    if let Some(data) = &resp.data {
                        text.push_str("\n\n");
                        text.push_str(&serde_json::to_string_pretty(data).unwrap_or_default());
                    }
                    ok_response(
                        req.id,
                        serde_json::json!({ "contents": [{ "uri": uri, "mimeType": "application/json", "text": text }] }),
                    )
                }
                _ => err_response(req.id, -32602, format!("Resource not found: {}", uri)),
            }
        }
        "ping" => ok_response(req.id, serde_json::json!({})),
        _ => err_response(req.id, -32601, format!("Method not found: {}", req.method)),
    };
    Some(response)
}

/// Run the MCP server on stdio (blocking). Reads JSON-RPC messages line-by-line.
pub async fn run_stdio() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let reader = stdin.lock();
    let line_iter = reader.lines();

    eprintln!("[nexus-mcp] Server started on stdio");

    for line_result in line_iter {
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
        assert_eq!(tools.len(), 95);
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
    async fn dispatch_savings_stats_tool() {
        let resp = dispatch_tool("nexus_savings_stats", &serde_json::json!({})).await;
        // DB may be empty or unavailable, but must not be an unknown-tool error
        assert!(
            resp.success || resp.message.contains("DB") || resp.message.contains("error"),
            "Expected success or DB error, got: {}",
            resp.message
        );
    }

    #[tokio::test]
    async fn dispatch_savings_report_tool() {
        let resp = dispatch_tool("nexus_savings_report", &serde_json::json!({})).await;
        assert!(
            resp.success || resp.message.contains("DB") || resp.message.contains("error"),
            "Expected success or DB error, got: {}",
            resp.message
        );
    }

    #[tokio::test]
    async fn dispatch_savings_per_model_missing_model() {
        let resp = dispatch_tool("nexus_savings_per_model", &serde_json::json!({})).await;
        assert!(!resp.success);
        assert!(
            resp.message.contains("model"),
            "Expected model-required error, got: {}",
            resp.message
        );
    }

    #[tokio::test]
    async fn dispatch_savings_per_model_unknown_model() {
        let resp = dispatch_tool(
            "nexus_savings_per_model",
            &serde_json::json!({
                "model": "not-a-real-model"
            }),
        )
        .await;
        assert!(!resp.success);
        assert!(
            resp.message.contains("Unknown model"),
            "Expected unknown-model error, got: {}",
            resp.message
        );
    }

    #[tokio::test]
    async fn dispatch_list_memories_tool() {
        let resp = dispatch_tool("nexus_list_memories", &serde_json::json!({})).await;
        // DB may or may not have memories, but command should succeed or report error
        assert!(
            resp.success
                || resp.message.contains("error")
                || resp.message.contains("DB")
                || resp.message.contains("not found"),
            "Expected success or DB-related error, got: {}",
            resp.message
        );
    }

    #[tokio::test]
    async fn dispatch_stats_tool() {
        let resp = dispatch_tool("nexus_stats", &serde_json::json!({})).await;
        // DB may not be available in test environment
        assert!(
            resp.success || resp.message.contains("error") || resp.message.contains("DB"),
            "Expected success or DB-related error, got: {}",
            resp.message
        );
        assert!(
            resp.message.contains("memories") || resp.message.contains("DB"),
            "Expected 'memories' or 'DB' in message, got: {}",
            resp.message
        );
    }

    #[tokio::test]
    async fn dispatch_copilot_command_tool() {
        let resp = dispatch_tool(
            "nexus_copilot_command",
            &serde_json::json!({
                "command": "/health"
            }),
        )
        .await;
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
        assert!(
            resp.is_none(),
            "Notifications must not receive responses per JSON-RPC 2.0"
        );
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
        assert_eq!(tools.len(), 95);
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
