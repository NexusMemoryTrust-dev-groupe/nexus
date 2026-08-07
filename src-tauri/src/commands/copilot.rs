use crate::ai::copilot::{self, CopilotResponse};

/// Execute a copilot slash command via Tauri IPC.
#[tauri::command]
pub async fn copilot_execute(command: String) -> Result<CopilotResponse, String> {
    let parsed = copilot::parse_command(&command)
        .ok_or_else(|| format!("Invalid command: must start with '/', got: {}", command))?;
    Ok(copilot::execute_command(&parsed).await)
}

/// List all available copilot commands.
#[tauri::command]
pub fn copilot_list_commands() -> Vec<serde_json::Value> {
    vec![
        // Memory commands
        serde_json::json!({ "command": "/memories", "description": "List all memories" }),
        serde_json::json!({ "command": "/memory <id>", "description": "Get memory details" }),
        serde_json::json!({ "command": "/create-memory <title> [content]", "description": "Create new memory" }),
        serde_json::json!({ "command": "/update-memory <id> <content>", "description": "Update memory content" }),
        serde_json::json!({ "command": "/delete-memory <id>", "description": "Delete memory" }),
        serde_json::json!({ "command": "/search <query>", "description": "Search memories" }),
        // Graph commands
        serde_json::json!({ "command": "/graph", "description": "Get knowledge graph stats" }),
        serde_json::json!({ "command": "/entity <id>", "description": "Get entity details" }),
        serde_json::json!({ "command": "/create-entity <type> <title>", "description": "Create entity (Person, Project, Task, Technology, etc.)" }),
        serde_json::json!({ "command": "/update-entity <id> <title>", "description": "Update entity title" }),
        serde_json::json!({ "command": "/delete-entity <id>", "description": "Delete entity" }),
        serde_json::json!({ "command": "/link <source> <target> [type] [weight]", "description": "Link two entities" }),
        serde_json::json!({ "command": "/unlink <id>", "description": "Remove relationship" }),
        // Context commands
        serde_json::json!({ "command": "/context <query>", "description": "Build context package for query" }),
        // Memory lifecycle commands
        serde_json::json!({ "command": "/lifecycle", "description": "Show memory trust lifecycle overview" }),
        serde_json::json!({ "command": "/memory-set-state <id> <state>", "description": "Set memory state (Current/Inferred/Superseded/Conflicted)" }),
        serde_json::json!({ "command": "/memory-confirm <id> [by]", "description": "Mark a memory as confirmed by a human" }),
        serde_json::json!({ "command": "/memory-feedback <id> <kind>", "description": "Record feedback: useful/irrelevant/wrong" }),
        serde_json::json!({ "command": "/memory-supersede <old_id> <title> <content>", "description": "Replace an outdated memory with a new one" }),
        // Entity resolution commands
        serde_json::json!({ "command": "/find-duplicates [min_score]", "description": "Scan graph for duplicate entities" }),
        serde_json::json!({ "command": "/merge-entities <primary> <dup>...", "description": "Merge duplicate entities into one" }),
        // Product metrics commands
        serde_json::json!({ "command": "/product-metrics", "description": "Show product metrics proving Nexus' value" }),
        // System commands
        serde_json::json!({ "command": "/stats", "description": "Show database statistics" }),
        serde_json::json!({ "command": "/health", "description": "Check system health" }),
        serde_json::json!({ "command": "/settings", "description": "Show application settings" }),
        serde_json::json!({ "command": "/timeline", "description": "Show all entities by creation date" }),
        serde_json::json!({ "command": "/help", "description": "Show available commands" }),
        serde_json::json!({ "command": "/projects", "description": "List all project entities" }),
        // Project knowledge base commands (RAG / AGENTS.md / skills)
        serde_json::json!({ "command": "/docs-import <folder>", "description": "Import project .md/.txt docs into the RAG corpus" }),
        serde_json::json!({ "command": "/docs-search <query>", "description": "Search imported project docs" }),
        serde_json::json!({ "command": "/agents [name]", "description": "Read the AGENTS.md instruction file" }),
        serde_json::json!({ "command": "/agents-generate", "description": "Generate AGENTS.md from live system data" }),
        serde_json::json!({ "command": "/skills", "description": "List registered skills" }),
        serde_json::json!({ "command": "/skill-run <name> [args...]", "description": "Run a skill" }),
        // Code graph commands
        serde_json::json!({ "command": "/code-import <folder>", "description": "Index source files into the code graph" }),
        serde_json::json!({ "command": "/code-search <symbol>", "description": "Search code symbols by name" }),
        serde_json::json!({ "command": "/code-deps <path>", "description": "Dependencies of a source file" }),
        serde_json::json!({ "command": "/code-dependents <target>", "description": "Files that depend on a target" }),
        serde_json::json!({ "command": "/code-stats", "description": "Code graph statistics" }),
    ]
}
