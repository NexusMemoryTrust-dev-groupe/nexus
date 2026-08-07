use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use std::sync::Mutex;

use crate::core::entity_id::EntityId;
use crate::core::result::Result;
use crate::core::team::team_repository::TeamRepository;
use crate::core::team::{TeamMember, TeamRole};
use crate::storage::sqlite::schema;

/// SQLite-backed implementation of TeamRepository.
/// Follows the same Mutex<Connection> pattern as SqliteMemoryRepository.
pub struct SqliteTeamRepository {
    conn: Mutex<Connection>,
}

impl SqliteTeamRepository {
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

fn row_to_member(row: &rusqlite::Row) -> rusqlite::Result<TeamMember> {
    let id_str: String = row.get(0)?;
    let name: String = row.get(1)?;
    let role_str: String = row.get(2)?;
    let active: i64 = row.get(3)?;
    let created_at: String = row.get(4)?;
    let updated_at: String = row.get(5)?;
    Ok(TeamMember {
        id: EntityId::parse(&id_str)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        name,
        role: TeamRole::parse(&role_str),
        active: active != 0,
        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
    })
}

#[async_trait]
impl TeamRepository for SqliteTeamRepository {
    async fn add_member(&self, member: &TeamMember) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO team_members (id, name, role, active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                member.id.as_str(),
                member.name,
                member.role.as_str(),
                if member.active { 1 } else { 0 },
                member.created_at.to_rfc3339(),
                member.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    async fn get_member(&self, id: &EntityId) -> Result<Option<TeamMember>> {
        let conn = self.conn.lock().unwrap();
        let row = conn
            .query_row(
                "SELECT id, name, role, active, created_at, updated_at
                 FROM team_members WHERE id = ?1",
                params![id.as_str()],
                row_to_member,
            )
            .optional()?;
        Ok(row)
    }

    async fn get_member_by_name(&self, name: &str) -> Result<Option<TeamMember>> {
        let conn = self.conn.lock().unwrap();
        let row = conn
            .query_row(
                "SELECT id, name, role, active, created_at, updated_at
                 FROM team_members WHERE name = ?1",
                params![name],
                row_to_member,
            )
            .optional()?;
        Ok(row)
    }

    async fn list_members(&self) -> Result<Vec<TeamMember>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, role, active, created_at, updated_at FROM team_members ORDER BY name",
        )?;
        let rows = stmt.query_map([], row_to_member)?;
        let mut members = Vec::new();
        for row in rows {
            members.push(row?);
        }
        Ok(members)
    }

    async fn update_member(&self, member: &TeamMember) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE team_members SET name = ?1, role = ?2, active = ?3, updated_at = ?4 WHERE id = ?5",
            params![
                member.name,
                member.role.as_str(),
                if member.active { 1 } else { 0 },
                member.updated_at.to_rfc3339(),
                member.id.as_str(),
            ],
        )?;
        Ok(())
    }

    async fn remove_member(&self, id: &EntityId) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM team_members WHERE id = ?1",
            params![id.as_str()],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn add_and_list_members() {
        let repo = SqliteTeamRepository::new_in_memory().unwrap();
        let alice = TeamMember::new("Alice".to_string(), TeamRole::Admin);
        let bob = TeamMember::new("Bob".to_string(), TeamRole::Viewer);

        repo.add_member(&alice).await.unwrap();
        repo.add_member(&bob).await.unwrap();
        let members = repo.list_members().await.unwrap();
        assert_eq!(members.len(), 2);
    }

    #[tokio::test]
    async fn duplicate_name_rejected() {
        let repo = SqliteTeamRepository::new_in_memory().unwrap();
        let a = TeamMember::new("Alice".to_string(), TeamRole::Admin);
        let b = TeamMember::new("Alice".to_string(), TeamRole::Member);
        repo.add_member(&a).await.unwrap();
        assert!(repo.add_member(&b).await.is_err());
    }

    #[tokio::test]
    async fn get_by_name_and_id() {
        let repo = SqliteTeamRepository::new_in_memory().unwrap();
        let alice = TeamMember::new("Alice".to_string(), TeamRole::Admin);
        repo.add_member(&alice).await.unwrap();
        let by_name = repo.get_member_by_name("Alice").await.unwrap().unwrap();
        assert_eq!(by_name.id, alice.id);
        let by_id = repo.get_member(&alice.id).await.unwrap().unwrap();
        assert_eq!(by_id.name, "Alice");
        assert!(repo.get_member_by_name("Nobody").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn update_and_remove() {
        let repo = SqliteTeamRepository::new_in_memory().unwrap();
        let mut alice = TeamMember::new("Alice".to_string(), TeamRole::Member);
        repo.add_member(&alice).await.unwrap();
        alice.role = TeamRole::Admin;
        alice.active = false;
        repo.update_member(&alice).await.unwrap();
        let fetched = repo.get_member(&alice.id).await.unwrap().unwrap();
        assert_eq!(fetched.role, TeamRole::Admin);
        assert!(!fetched.active);
        repo.remove_member(&alice.id).await.unwrap();
        assert!(repo.get_member(&alice.id).await.unwrap().is_none());
    }
}
