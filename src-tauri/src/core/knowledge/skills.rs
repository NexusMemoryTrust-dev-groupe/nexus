//! Skills — runnable commands agents can invoke.
//!
//! The team's feedback noted that MCP tools bloat context, and modern agents
//! prefer "skills": named, on-demand capabilities the agent calls only when
//! needed. A skill is a stored command (a JS script, a shell one-liner, a
//! binary) with a description the agent can read before deciding to run it.
//!
//! Skills are persisted in the `skills` table and executed through
//! [`SkillRunner`], which runs the command with an argument list and a
//! timeout, capturing stdout/stderr so the result can be returned to the AI.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::core::entity_id::EntityId;
use crate::core::result::{AppError, Result};

/// A runnable skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: EntityId,
    pub name: String,
    pub description: String,
    pub command: String,
    pub script_path: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Output of a skill execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub timed_out: bool,
}

/// Default timeout for skill execution (protects the MCP server from a
/// runaway script).
const SKILL_TIMEOUT: Duration = Duration::from_secs(30);

/// SQLite-backed repository for skills.
pub struct SkillRepository {
    conn: Connection,
}

impl SkillRepository {
    pub fn new(conn: Connection) -> Result<Self> {
        crate::storage::sqlite::schema::apply_migrations(&conn)?;
        Ok(Self { conn })
    }

    pub fn open() -> Result<Self> {
        let conn = crate::db::open_connection().map_err(AppError::Database)?;
        Self::new(conn)
    }

    /// Register (or update) a skill by name.
    pub fn upsert(
        &self,
        name: &str,
        description: &str,
        command: &str,
        script_path: &str,
    ) -> Result<Skill> {
        let now = chrono::Utc::now().to_rfc3339();
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM skills WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))?;

        let id = match existing {
            Some(id) => {
                self.conn
                    .execute(
                        "UPDATE skills
                         SET description = ?2, command = ?3, script_path = ?4, updated_at = ?5
                         WHERE id = ?1",
                        params![id, description, command, script_path, now],
                    )
                    .map_err(|e| AppError::Database(e.to_string()))?;
                EntityId::parse(&id).unwrap_or_else(|_| EntityId::new())
            }
            None => {
                let id = EntityId::new();
                self.conn
                    .execute(
                        "INSERT INTO skills
                         (id, name, description, command, script_path, enabled, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)",
                        params![id.as_str(), name, description, command, script_path, now],
                    )
                    .map_err(|e| AppError::Database(e.to_string()))?;
                id
            }
        };

        self.get(&id).map(|o| o.expect("just upserted"))
    }

    pub fn get(&self, id: &EntityId) -> Result<Option<Skill>> {
        self.conn
            .query_row(
                "SELECT id, name, description, command, script_path, enabled, created_at, updated_at
                 FROM skills WHERE id = ?1",
                params![id.as_str()],
                Self::row_to_skill,
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))
    }

    pub fn get_by_name(&self, name: &str) -> Result<Option<Skill>> {
        self.conn
            .query_row(
                "SELECT id, name, description, command, script_path, enabled, created_at, updated_at
                 FROM skills WHERE name = ?1",
                params![name],
                Self::row_to_skill,
            )
            .optional()
            .map_err(|e| AppError::Database(e.to_string()))
    }

    pub fn list(&self) -> Result<Vec<Skill>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, description, command, script_path, enabled, created_at, updated_at
                 FROM skills ORDER BY name",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], Self::row_to_skill)
            .map_err(|e| AppError::Database(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| AppError::Database(e.to_string()))?);
        }
        Ok(out)
    }

    pub fn delete(&self, id: &EntityId) -> Result<()> {
        self.conn
            .execute("DELETE FROM skills WHERE id = ?1", params![id.as_str()])
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    fn row_to_skill(row: &rusqlite::Row<'_>) -> rusqlite::Result<Skill> {
        let id_str: String = row.get(0)?;
        Ok(Skill {
            id: EntityId::parse(&id_str).unwrap_or_else(|_| EntityId::new()),
            name: row.get(1)?,
            description: row.get(2)?,
            command: row.get(3)?,
            script_path: row.get(4)?,
            enabled: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    }
}

/// Executes skills with a timeout, capturing output.
pub struct SkillRunner;

/// Default skills shipped with the app, registered on first startup so agents
/// can use them through both MCP (`nexus_skills_list`/`nexus_skills_run`) and
/// Copilot (`/skills`/`/skill-run`).
///
/// Each entry is `(name, description, command, script_file)`. The scripts live
/// in `<repo>/scripts/skills/` and read the Nexus database read-only through
/// `node:sqlite`, so they work even when the MCP server is not reachable.
pub const DEFAULT_SKILLS: &[(&str, &str, &str, &str)] = &[
    (
        "audit-trail",
        "Reconstruct the full decision chain of one memory: why it exists, which alternatives were rejected, who confirmed it and when, what it superseded and what replaced it, plus its version history. Pass a memory id. The compliance answer to 'why did we decide this?'.",
        "node",
        "audit-trail.js",
    ),
    (
        "team-roster",
        "List the trusted decision layer: every team member with role and per-member activity (memories authored / confirmed / superseded). The answer to 'who is on the team and what do they agree on'.",
        "node",
        "team-roster.js",
    ),
    (
        "radar-scan",
        "Latest memory activity: the most recently updated memories with state, layer, importance and id. The answer to 'what is the team working on right now'. Optional limit argument (default 12).",
        "node",
        "radar-scan.js",
    ),
    (
        "memory-search",
        "Full-text search over memories by title, summary and content (FTS5). Returns ranked hits with state, layer and id. Arguments: <query> [limit]. The answer to 'have we ever decided X?'.",
        "node",
        "memory-search.js",
    ),
    (
        "version-history",
        "Full version history of one memory: every automatic commit with change type, author, timestamp, size and change reason. The audit trail of the content itself. Pass a memory id.",
        "node",
        "version-history.js",
    ),
    (
        "code-search",
        "Search the indexed code graph (code_symbols joined with code_files) for functions, structs, types and modules. Arguments: <query> [limit]. The answer to 'where is X implemented?'.",
        "node",
        "code-search.js",
    ),
];

/// Register the default skills (upsert — safe to call on every startup; it
/// refreshes descriptions and paths without duplicating rows). Resolves the
/// scripts directory relative to the crate manifest so it works both in dev
/// (`npm run tauri dev`) and after `cargo build`.
pub fn seed_default_skills() -> std::result::Result<usize, String> {
    let repo = SkillRepository::open().map_err(|e| e.to_string())?;
    let scripts_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("scripts")
        .join("skills");
    let mut seeded = 0usize;
    for (name, description, command, script_file) in DEFAULT_SKILLS {
        let script_path = scripts_dir.join(script_file).to_string_lossy().to_string();
        repo.upsert(name, description, command, &script_path)
            .map_err(|e| e.to_string())?;
        seeded += 1;
    }
    Ok(seeded)
}

impl SkillRunner {
    /// Run a skill by name with the given arguments.
    pub fn run(name: &str, args: &[String]) -> Result<SkillOutput> {
        let repo = SkillRepository::open()?;
        let skill = repo
            .get_by_name(name)?
            .ok_or_else(|| AppError::NotFound(format!("Skill '{}' not found", name)))?;
        if !skill.enabled {
            return Ok(SkillOutput {
                success: false,
                stdout: String::new(),
                stderr: format!("Skill '{}' is disabled", name),
                exit_code: Some(1),
                duration_ms: 0,
                timed_out: false,
            });
        }
        Self::execute(&skill, args)
    }

    /// Execute a skill's command with arguments.
    fn execute(skill: &Skill, args: &[String]) -> Result<SkillOutput> {
        let start = Instant::now();
        let mut cmd = Self::build_command(skill, args);

        // Capture output; spawn with pipes so we can read stdout/stderr.
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::Internal(format!("Failed to run '{}': {}", skill.name, e)))?;

        // Read output with a timeout using threads.
        let stdout_handle = std::thread::spawn({
            let mut stdout = child.stdout.take().expect("stdout piped");
            move || {
                let mut buf = String::new();
                use std::io::Read;
                let _ = stdout.read_to_string(&mut buf);
                buf
            }
        });
        let stderr_handle = std::thread::spawn({
            let mut stderr = child.stderr.take().expect("stderr piped");
            move || {
                let mut buf = String::new();
                use std::io::Read;
                let _ = stderr.read_to_string(&mut buf);
                buf
            }
        });

        // Wait with timeout.
        let (status, timed_out) = match wait_with_timeout(&mut child, SKILL_TIMEOUT) {
            Ok(status) => (status, false),
            Err(_) => {
                let _ = child.kill();
                (None, true)
            }
        };

        let stdout = stdout_handle.join().unwrap_or_default();
        let stderr = stderr_handle.join().unwrap_or_default();

        let exit_code = status.and_then(|s| s.code());
        Ok(SkillOutput {
            success: !timed_out && exit_code == Some(0),
            stdout: cap_output(&stdout),
            stderr: cap_output(&stderr),
            exit_code,
            duration_ms: start.elapsed().as_millis() as u64,
            timed_out,
        })
    }

    /// Build the OS command for a skill.
    ///
    /// Skills are stored as a command template; arguments are appended as
    /// separate argv entries (never shell-interpolated, so no injection).
    fn build_command(skill: &Skill, args: &[String]) -> Command {
        // When a JS script path is present and node exists, run `node <script>`.
        if !skill.script_path.is_empty() && skill.script_path.ends_with(".js") {
            let mut cmd = Command::new("node");
            cmd.arg(&skill.script_path);
            cmd.args(args);
            return cmd;
        }

        // Otherwise split the command on whitespace: program + fixed args.
        let mut parts = skill.command.split_whitespace();
        let program = parts.next().unwrap_or("true");
        let mut cmd = Command::new(program);
        cmd.args(parts);
        cmd.args(args);
        cmd
    }
}

/// Wait for a child with a timeout. Returns the status, or `Err` on timeout.
fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> std::result::Result<Option<std::process::ExitStatus>, ()> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(Some(status)),
            Ok(None) => {
                if Instant::now() >= deadline {
                    return Err(());
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => {
                tracing::warn!("wait_with_timeout: {e}");
                return Ok(None);
            }
        }
    }
}

/// Keep output bounded so a chatty script cannot blow up a context window.
const MAX_OUTPUT_CHARS: usize = 16 * 1024;

fn cap_output(s: &str) -> String {
    if s.len() <= MAX_OUTPUT_CHARS {
        s.to_string()
    } else {
        let mut end = MAX_OUTPUT_CHARS;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        let mut out = s[..end].to_string();
        out.push_str("\n…[output truncated]…");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_repo() -> SkillRepository {
        let conn = Connection::open_in_memory().unwrap();
        SkillRepository::new(conn).unwrap()
    }

    #[test]
    fn upsert_and_get() {
        let repo = test_repo();
        let skill = repo.upsert("hello", "Say hello", "echo hello", "").unwrap();
        assert_eq!(skill.name, "hello");
        assert_eq!(
            repo.get_by_name("hello").unwrap().unwrap().description,
            "Say hello"
        );
        assert_eq!(repo.list().unwrap().len(), 1);
    }

    #[test]
    fn upsert_updates_same_name() {
        let repo = test_repo();
        repo.upsert("s", "v1", "echo one", "").unwrap();
        repo.upsert("s", "v2", "echo two", "").unwrap();
        assert_eq!(repo.list().unwrap().len(), 1);
        assert_eq!(repo.get_by_name("s").unwrap().unwrap().description, "v2");
    }

    #[test]
    fn delete_removes() {
        let repo = test_repo();
        let skill = repo.upsert("s", "d", "true", "").unwrap();
        repo.delete(&skill.id).unwrap();
        assert!(repo.get_by_name("s").unwrap().is_none());
    }

    #[test]
    fn cap_output_truncates() {
        let big = "x".repeat(20000);
        let capped = cap_output(&big);
        assert!(capped.len() <= MAX_OUTPUT_CHARS + 32);
        assert!(capped.contains("truncated"));
    }

    #[test]
    fn cap_output_keeps_small() {
        assert_eq!(cap_output("hello"), "hello");
    }
}
