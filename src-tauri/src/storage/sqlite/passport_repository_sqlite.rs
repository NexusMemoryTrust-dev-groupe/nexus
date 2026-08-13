//! SQLite-хранилище паспортов агентов (Система 6).
//! Тот же паттерн, что у team_repository_sqlite: Mutex<Connection>, WAL.

use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use std::sync::Mutex;

use crate::core::knowledge::agent_passport::{
    AgentPassport, AgentRole, MemoryScope, PassportRepository,
};
use crate::core::result::Result;
use crate::storage::sqlite::schema;

/// SQLite-backed implementation of PassportRepository.
pub struct SqlitePassportRepository {
    conn: Mutex<Connection>,
}

impl SqlitePassportRepository {
    pub fn new(conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        schema::apply_migrations(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    #[cfg(test)]
    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::new(conn)
    }
}

fn row_to_passport(row: &rusqlite::Row) -> rusqlite::Result<AgentPassport> {
    let name: String = row.get(1)?;
    let display_name: String = row.get(2)?;
    let role_str: String = row.get(3)?;
    let description: String = row.get(4)?;
    let skills_json: String = row.get(5)?;
    let tools_json: String = row.get(6)?;
    let constraints_json: String = row.get(7)?;
    let trust_level: i64 = row.get(8)?;
    let memory_scope_str: String = row.get(9)?;
    let is_active: i64 = row.get(10)?;
    let created_at: String = row.get(11)?;
    let updated_at: String = row.get(12)?;
    let parse_list =
        |s: String| -> Vec<String> { serde_json::from_str::<Vec<String>>(&s).unwrap_or_default() };
    Ok(AgentPassport {
        id: row.get(0)?,
        name,
        display_name,
        role: AgentRole::parse(&role_str),
        description,
        skills: parse_list(skills_json),
        tools: parse_list(tools_json),
        constraints: parse_list(constraints_json),
        trust_level: trust_level.clamp(1, 10) as u8,
        memory_scope: MemoryScope::parse(&memory_scope_str),
        is_active: is_active != 0,
        created_at,
        updated_at,
    })
}

fn list_to_json(list: &[String]) -> String {
    serde_json::to_string(list).unwrap_or_else(|_| "[]".to_string())
}

#[async_trait]
impl PassportRepository for SqlitePassportRepository {
    async fn upsert(&self, passport: &AgentPassport) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        let updated = conn.execute(
            "UPDATE agent_passports
             SET display_name = ?2, role = ?3, description = ?4,
                 skills_json = ?5, tools_json = ?6, constraints_json = ?7,
                 trust_level = ?8, memory_scope = ?9, is_active = ?10,
                 updated_at = ?11
             WHERE name = ?1",
            params![
                passport.name,
                passport.display_name,
                passport.role.as_str(),
                passport.description,
                list_to_json(&passport.skills),
                list_to_json(&passport.tools),
                list_to_json(&passport.constraints),
                passport.trust_level as i64,
                passport.memory_scope.as_str(),
                passport.is_active as i64,
                now,
            ],
        )?;
        if updated == 0 {
            conn.execute(
                "INSERT INTO agent_passports
                 (id, name, display_name, role, description,
                  skills_json, tools_json, constraints_json,
                  trust_level, memory_scope, is_active, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    passport.id,
                    passport.name,
                    passport.display_name,
                    passport.role.as_str(),
                    passport.description,
                    list_to_json(&passport.skills),
                    list_to_json(&passport.tools),
                    list_to_json(&passport.constraints),
                    passport.trust_level as i64,
                    passport.memory_scope.as_str(),
                    passport.is_active as i64,
                    passport.created_at,
                    passport.updated_at,
                ],
            )?;
        }
        Ok(())
    }

    async fn get_by_name(&self, name: &str) -> Result<Option<AgentPassport>> {
        let conn = self.conn.lock().unwrap();
        let passport = conn
            .query_row(
                "SELECT id, name, display_name, role, description,
                        skills_json, tools_json, constraints_json,
                        trust_level, memory_scope, is_active, created_at, updated_at
                 FROM agent_passports WHERE name = ?1",
                params![name],
                row_to_passport,
            )
            .optional()?;
        Ok(passport)
    }

    async fn list(&self, active_only: bool) -> Result<Vec<AgentPassport>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = if active_only {
            conn.prepare(
                "SELECT id, name, display_name, role, description,
                        skills_json, tools_json, constraints_json,
                        trust_level, memory_scope, is_active, created_at, updated_at
                 FROM agent_passports WHERE is_active = 1 ORDER BY name",
            )?
        } else {
            conn.prepare(
                "SELECT id, name, display_name, role, description,
                        skills_json, tools_json, constraints_json,
                        trust_level, memory_scope, is_active, created_at, updated_at
                 FROM agent_passports ORDER BY name",
            )?
        };
        let rows = stmt.query_map([], row_to_passport)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    async fn set_active(&self, name: &str, active: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE agent_passports SET is_active = ?2, updated_at = ?3 WHERE name = ?1",
            params![name, active as i64, chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    async fn delete(&self, name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM agent_passports WHERE name = ?1", params![name])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::knowledge::agent_passport::{
        AgentRole, MemoryScope, default_primary_passport,
    };

    fn sample_passport() -> AgentPassport {
        AgentPassport::new(
            "coder-alpha",
            "Coder Alpha",
            AgentRole::Coder,
            "Writes and refactors Rust code.",
            vec!["rust".to_string()],
            vec!["nexus_memory_search".to_string()],
            vec!["Never delete memories".to_string()],
            8,
            MemoryScope::Project,
        )
    }

    #[tokio::test]
    async fn upsert_inserts_and_updates() {
        let repo = SqlitePassportRepository::new_in_memory().unwrap();

        repo.upsert(&sample_passport()).await.unwrap();
        let fetched = repo.get_by_name("coder-alpha").await.unwrap().unwrap();
        assert_eq!(fetched.role, AgentRole::Coder);
        assert_eq!(fetched.trust_level, 8);
        assert!(fetched.skills.contains(&"rust".to_string()));

        // Обновление по имени — не создаёт вторую запись.
        let mut updated = sample_passport();
        updated.role = AgentRole::Reviewer;
        updated.trust_level = 9;
        repo.upsert(&updated).await.unwrap();
        let fetched = repo.get_by_name("coder-alpha").await.unwrap().unwrap();
        assert_eq!(fetched.role, AgentRole::Reviewer);
        assert_eq!(fetched.trust_level, 9);
        assert_eq!(repo.list(false).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn get_missing_returns_none() {
        let repo = SqlitePassportRepository::new_in_memory().unwrap();
        assert!(repo.get_by_name("nobody").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_filters_active() {
        let repo = SqlitePassportRepository::new_in_memory().unwrap();
        repo.upsert(&sample_passport()).await.unwrap();
        repo.upsert(&default_primary_passport()).await.unwrap();

        assert_eq!(repo.list(false).await.unwrap().len(), 2);

        repo.set_active("coder-alpha", false).await.unwrap();
        let active = repo.list(true).await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "opencode-primary");
    }

    #[tokio::test]
    async fn delete_removes_passport() {
        let repo = SqlitePassportRepository::new_in_memory().unwrap();
        repo.upsert(&sample_passport()).await.unwrap();
        repo.delete("coder-alpha").await.unwrap();
        assert!(repo.get_by_name("coder-alpha").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn json_lists_roundtrip() {
        let repo = SqlitePassportRepository::new_in_memory().unwrap();
        let mut p = sample_passport();
        p.tools = vec![
            "nexus_memory_search".to_string(),
            "nexus_memory_store".to_string(),
        ];
        p.constraints = vec!["Constraint A".to_string()];
        repo.upsert(&p).await.unwrap();
        let fetched = repo.get_by_name("coder-alpha").await.unwrap().unwrap();
        assert_eq!(fetched.tools.len(), 2);
        assert_eq!(fetched.constraints, vec!["Constraint A".to_string()]);
    }
}
