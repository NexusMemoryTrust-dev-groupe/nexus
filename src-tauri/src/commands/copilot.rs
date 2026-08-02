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
        // System commands
        serde_json::json!({ "command": "/stats", "description": "Show database statistics" }),
        serde_json::json!({ "command": "/health", "description": "Check system health" }),
        serde_json::json!({ "command": "/settings", "description": "Show application settings" }),
        serde_json::json!({ "command": "/timeline", "description": "Show all entities by creation date" }),
        serde_json::json!({ "command": "/help", "description": "Show available commands" }),
        serde_json::json!({ "command": "/projects", "description": "List all project entities" }),
    ]
}
