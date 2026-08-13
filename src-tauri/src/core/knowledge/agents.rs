//! AGENTS.md-style instruction files.
//!
//! An agents file is a markdown document (conventionally `AGENTS.md`) that
//! describes how an AI should behave inside a project: architecture rules,
//! conventions, forbidden operations. The content is stored in
//! `agents_documents` and injected into every context package so the AI
//! follows the project's rules automatically.
//!
//! The generator (`generate_agents_file`) is the "documentation skill": it
//! builds a fresh AGENTS.md from the system's own data (modules, commands,
//! tools, DB stats) instead of requiring the developer to write one by hand.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::core::entity_id::EntityId;
use crate::core::knowledge::content_checksum;
use crate::core::result::{AppError, Result};

/// An AGENTS.md-style instruction file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsFile {
    pub id: EntityId,
    pub name: String,
    pub content: String,
    pub path: String,
    pub checksum: String,
    pub created_at: String,
    pub updated_at: String,
}

/// SQLite-backed repository for agents files. One row per name (`AGENTS.md`,
/// `AGENTS-API.md`, …).
pub struct AgentsRepository {
    conn: Connection,
}

impl AgentsRepository {
    pub fn new(conn: Connection) -> Result<Self> {
        crate::storage::sqlite::schema::apply_migrations(&conn)?;
        Ok(Self { conn })
    }

    pub fn open() -> Result<Self> {
        let conn = crate::db::open_connection().map_err(AppError::Database)?;
        Self::new(conn)
    }

    /// Upsert by name. Returns `true` when content changed, `false` when no-op.
    pub fn upsert(&self, name: &str, content: &str, path: &str) -> Result<bool> {
        let checksum = content_checksum(content);
        let now = chrono::Utc::now().to_rfc3339();

        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT checksum FROM agents_documents WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))?;

        match existing {
            Some(old) if old == checksum => Ok(false),
            Some(_) => {
                self.conn
                    .execute(
                        "UPDATE agents_documents
                         SET content = ?2, path = ?3, checksum = ?4, updated_at = ?5
                         WHERE name = ?1",
                        params![name, content, path, checksum, now],
                    )
                    .map_err(|e| AppError::Database(e.to_string()))?;
                Ok(true)
            }
            None => {
                let id = EntityId::new();
                self.conn
                    .execute(
                        "INSERT INTO agents_documents
                         (id, name, content, path, checksum, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                        params![id.as_str(), name, content, path, checksum, now],
                    )
                    .map_err(|e| AppError::Database(e.to_string()))?;
                Ok(true)
            }
        }
    }

    pub fn get(&self, name: &str) -> Result<Option<AgentsFile>> {
        self.conn
            .query_row(
                "SELECT id, name, content, path, checksum, created_at, updated_at
                 FROM agents_documents WHERE name = ?1",
                params![name],
                Self::row_to_file,
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))
    }

    pub fn list(&self) -> Result<Vec<AgentsFile>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, content, path, checksum, created_at, updated_at
                 FROM agents_documents ORDER BY name",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], Self::row_to_file)
            .map_err(|e| AppError::Database(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| AppError::Database(e.to_string()))?);
        }
        Ok(out)
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM agents_documents WHERE name = ?1",
                params![name],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    fn row_to_file(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentsFile> {
        let id_str: String = row.get(0)?;
        Ok(AgentsFile {
            id: EntityId::parse(&id_str).unwrap_or_else(|_| EntityId::new()),
            name: row.get(1)?,
            content: row.get(2)?,
            path: row.get(3)?,
            checksum: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    }
}

/// The default instruction file name.
pub const DEFAULT_AGENTS_NAME: &str = "AGENTS.md";

/// Load the active agents file (defaulting to `AGENTS.md`), or `None` when the
/// project has not defined one yet.
pub fn active_agents_content() -> Option<String> {
    AgentsRepository::open()
        .ok()
        .and_then(|repo| repo.get(DEFAULT_AGENTS_NAME).ok().flatten())
        .map(|f| f.content)
}

/// Build an AGENTS.md from the system's own data — the "documentation skill".
///
/// The generated file reflects what Nexus actually knows right now: module
/// inventory, copilot commands, MCP tools, storage stats. It is regenerated on
/// demand (`/agents-generate`, `nexus_agents_generate`) so it never goes stale
/// the way a hand-written doc does.
pub fn generate_agents_file() -> String {
    let mut out = String::with_capacity(4096);
    out.push_str("# AGENTS.md — Nexus project instructions\n\n");
    out.push_str("> Generated by Nexus from live system data. Regenerate with ");
    out.push_str("`/agents-generate` or the `nexus_agents_generate` MCP tool.\n\n");

    // ── Project identity ──
    out.push_str("## Project\n\n");
    out.push_str("Nexus Memory Trust — AI Memory Operating System.\n");
    out.push_str("This file instructs AI agents working inside the project.\n\n");

    // ── Modules ──
    out.push_str("## Architecture\n\n");
    out.push_str("The codebase is organized as modules (see `core/mod.rs`):\n\n");
    let modules = [
        (
            "memory",
            "memory records, lifecycle (Current/Inferred/Superseded/Conflicted), recall",
        ),
        (
            "graph",
            "knowledge graph: entities, relationships, projects",
        ),
        (
            "context",
            "context engine: intent detection, seeding, expansion, memory injection, ranking, compression",
        ),
        ("knowledge", "RAG documents, AGENTS.md instructions, skills"),
        ("ai", "copilot slash commands and the MCP stdio server"),
        ("storage", "SQLite persistence and schema migrations"),
        ("security", "request context and access control"),
    ];
    for (name, desc) in modules {
        out.push_str(&format!("- `{name}` — {desc}\n"));
    }
    out.push('\n');

    // ── Copilot commands ──
    out.push_str("## Available commands\n\n");
    out.push_str(
        "Slash commands usable via `/copilot` or the `nexus_copilot_command` MCP tool:\n\n",
    );
    for (cmd, desc) in crate::commands::knowledge::copilot_command_help() {
        out.push_str(&format!("- `{cmd}` — {desc}\n"));
    }
    out.push('\n');

    // ── Knowledge base ──
    out.push_str("## Knowledge base\n\n");
    out.push_str("- Import project docs: `/docs-import <folder>` or `nexus_docs_import`\n");
    out.push_str("- Search project docs: `/docs-search <query>` or `nexus_docs_search`\n");
    out.push_str(
        "- List skills: `/skills` or `nexus_skills_list`; run: `/skill-run <name> [args...]`\n",
    );
    out.push_str("- Index source code: `/code-import <folder>` or `nexus_code_import`\n");
    out.push_str("- Search code symbols: `/code-search <symbol>` or `nexus_code_search`\n");
    out.push_str(
        "- File dependencies: `/code-deps <path>`; dependents: `/code-dependents <target>`\n\n",
    );

    // ── Agent passports ──
    out.push_str("## Agents\n\n");
    out.push_str("Agent identity passports (who you are, what you may do):\n\n");
    if let Ok(conn) = crate::db::open_connection() {
        let mut passports_found = false;
        if let Ok(mut stmt) = conn.prepare(
            "SELECT name, display_name, role, description,
                    skills_json, tools_json, constraints_json,
                    trust_level, memory_scope, is_active
             FROM agent_passports WHERE is_active = 1 ORDER BY name",
        ) && let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, String>(8)?,
            ))
        }) {
            for row in rows.flatten() {
                passports_found = true;
                let (name, display, role, desc, skills, tools, cons, trust, scope) = row;
                out.push_str(&format!(
                    "- **{name}** ({display}) — role `{role}`, scope `{scope}`, trust {trust}/10",
                ));
                if !desc.is_empty() {
                    out.push_str(&format!(" — {desc}"));
                }
                out.push('\n');
                let parse_list = |s: String| -> Vec<String> {
                    serde_json::from_str::<Vec<String>>(&s).unwrap_or_default()
                };
                let skills = parse_list(skills);
                let tools = parse_list(tools);
                let cons = parse_list(cons);
                if !skills.is_empty() {
                    out.push_str(&format!("  - skills: {}\n", skills.join(", ")));
                }
                if !tools.is_empty() {
                    out.push_str(&format!("  - tools: {}\n", tools.join(", ")));
                }
                if !cons.is_empty() {
                    out.push_str(&format!("  - constraints: {}\n", cons.join("; ")));
                }
            }
        }
        if !passports_found {
            let primary = crate::core::knowledge::default_primary_passport();
            out.push_str(&format!(
                "- **{}** ({}) — role `{}`, scope `{}`, trust {}/10 — {}\n",
                primary.name,
                primary.display_name,
                primary.role.as_str(),
                primary.memory_scope.as_str(),
                primary.trust_level,
                primary.description
            ));
            out.push_str(&format!("  - skills: {}\n", primary.skills.join(", ")));
        }
    } else {
        out.push_str(
            "- database unavailable — passports not loaded; assume generalist role, project scope\n",
        );
    }
    out.push('\n');

    // ── Storage stats ──
    if let Ok(conn) = crate::db::open_connection() {
        let count = |table: &str| -> u64 {
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap_or(0)
            .max(0) as u64
        };
        out.push_str("## Current state\n\n");
        out.push_str(&format!(
            "- memories: {} | entities: {} | project docs: {} | skills: {} | agents files: {} | code files: {} | code symbols: {}\n",
            count("memory_records"),
            count("graph_entities"),
            count("project_documents"),
            count("skills"),
            count("agents_documents"),
            count("code_files"),
            count("code_symbols"),
        ));
    }

    out.push_str("\n## Rules\n\n");
    out.push_str("- Always ground answers in Nexus memory and project docs when available.\n");
    out.push_str("- Use slash commands and MCP tools rather than raw SQL.\n");
    out.push_str("- Never invent data; state explicitly when information is missing.\n");

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_repo() -> AgentsRepository {
        let conn = Connection::open_in_memory().unwrap();
        AgentsRepository::new(conn).unwrap()
    }

    #[test]
    fn upsert_roundtrip() {
        let repo = test_repo();
        let created = repo.upsert("AGENTS.md", "# Rules\n- be nice", "").unwrap();
        assert!(created);
        let noop = repo.upsert("AGENTS.md", "# Rules\n- be nice", "").unwrap();
        assert!(!noop);

        let file = repo.get("AGENTS.md").unwrap().unwrap();
        assert_eq!(file.name, "AGENTS.md");
        assert!(file.content.contains("be nice"));
        assert_eq!(repo.list().unwrap().len(), 1);
    }

    #[test]
    fn delete_removes() {
        let repo = test_repo();
        repo.upsert("AGENTS.md", "x", "").unwrap();
        repo.delete("AGENTS.md").unwrap();
        assert!(repo.get("AGENTS.md").unwrap().is_none());
    }

    #[test]
    fn generator_contains_system_data() {
        let text = generate_agents_file();
        assert!(text.contains("# AGENTS.md"));
        assert!(text.contains("memory"));
        assert!(text.contains("nexus_copilot_command"));
        assert!(text.contains("## Agents"));
        assert!(
            text.contains("trust "),
            "passport section renders trust level"
        );
    }
}
