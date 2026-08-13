use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use std::sync::Mutex;

use crate::core::entity_id::EntityId;
use crate::core::memory::conflict::{
    ConflictGroup, ConflictRepository, ConflictResolution, ConflictStatus,
};
use crate::core::result::Result;
use crate::storage::sqlite::schema;

/// SQLite-backed implementation of the conflict group repository.
/// Same Mutex<Connection> pattern as the other repositories.
pub struct SqliteConflictRepository {
    conn: Mutex<Connection>,
}

impl SqliteConflictRepository {
    /// Used by the conflict service / Tauri commands (Phase 2.3+).
    #[allow(dead_code)] // Constructed once the conflict service lands
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

fn row_to_group(row: &rusqlite::Row) -> rusqlite::Result<ConflictGroup> {
    let id_str: String = row.get(0)?;
    let topic: String = row.get(1)?;
    let member_json: String = row.get(2)?;
    let detected_at: String = row.get(3)?;
    let resolved_at: Option<String> = row.get(4)?;
    let resolution_json: Option<String> = row.get(5)?;
    let status_str: String = row.get(6)?;
    Ok(ConflictGroup {
        id: EntityId::parse(&id_str)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        topic,
        member_ids: ConflictGroup::member_ids_from_json(&member_json),
        detected_at: chrono::DateTime::parse_from_rfc3339(&detected_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        status: ConflictStatus::parse(&status_str),
        resolved_at: resolved_at
            .map(|s| {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))
            })
            .transpose()?,
        resolution: resolution_json.and_then(|j| ConflictGroup::resolution_from_json(&j)),
    })
}

#[async_trait]
impl ConflictRepository for SqliteConflictRepository {
    async fn save_group(&self, group: &ConflictGroup) -> Result<EntityId> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO conflict_groups (id, topic, member_ids, detected_at, resolved_at, resolution, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                group.id.as_str(),
                group.topic,
                group.member_ids_to_json(),
                group.detected_at.to_rfc3339(),
                group.resolved_at.map(|dt| dt.to_rfc3339()),
                group.resolution.as_ref().map(ConflictGroup::resolution_to_json),
                group.status.as_str(),
            ],
        )?;
        Ok(group.id.clone())
    }

    async fn get_group(&self, id: &EntityId) -> Result<Option<ConflictGroup>> {
        let conn = self.conn.lock().unwrap();
        let group = conn
            .query_row(
                "SELECT id, topic, member_ids, detected_at, resolved_at, resolution, status
                 FROM conflict_groups WHERE id = ?1",
                params![id.as_str()],
                row_to_group,
            )
            .optional()?;
        Ok(group)
    }

    async fn list_groups(&self, status: Option<ConflictStatus>) -> Result<Vec<ConflictGroup>> {
        let conn = self.conn.lock().unwrap();
        let (sql, param): (&str, Option<String>) = match status {
            Some(s) => (
                "SELECT id, topic, member_ids, detected_at, resolved_at, resolution, status
                 FROM conflict_groups WHERE status = ?1 ORDER BY detected_at DESC",
                Some(s.as_str().to_string()),
            ),
            None => (
                "SELECT id, topic, member_ids, detected_at, resolved_at, resolution, status
                 FROM conflict_groups ORDER BY detected_at DESC",
                None,
            ),
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = match param {
            Some(p) => stmt.query_map(params![p], row_to_group)?,
            None => stmt.query_map([], row_to_group)?,
        };
        let mut groups = Vec::new();
        for row in rows {
            groups.push(row?);
        }
        Ok(groups)
    }

    async fn update_group(&self, group: &ConflictGroup) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE conflict_groups
             SET topic = ?2, member_ids = ?3, detected_at = ?4, resolved_at = ?5,
                 resolution = ?6, status = ?7
             WHERE id = ?1",
            params![
                group.id.as_str(),
                group.topic,
                group.member_ids_to_json(),
                group.detected_at.to_rfc3339(),
                group.resolved_at.map(|dt| dt.to_rfc3339()),
                group
                    .resolution
                    .as_ref()
                    .map(ConflictGroup::resolution_to_json),
                group.status.as_str(),
            ],
        )?;
        Ok(())
    }

    async fn open_groups_containing(&self, memory_id: &EntityId) -> Result<Vec<ConflictGroup>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, topic, member_ids, detected_at, resolved_at, resolution, status
             FROM conflict_groups WHERE status = 'open' ORDER BY detected_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_group)?;
        let mut groups = Vec::new();
        for row in rows {
            let group = row?;
            if group.contains(memory_id) {
                groups.push(group);
            }
        }
        Ok(groups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn save_and_get_group() {
        let repo = SqliteConflictRepository::new_in_memory().unwrap();
        let m1 = EntityId::new();
        let m2 = EntityId::new();
        let group = ConflictGroup::new("database".to_string(), vec![m1, m2]);
        repo.save_group(&group).await.unwrap();

        let loaded = repo.get_group(&group.id).await.unwrap().unwrap();
        assert_eq!(loaded.id, group.id);
        assert_eq!(loaded.topic, "database");
        assert_eq!(loaded.member_ids.len(), 2);
        assert_eq!(loaded.status, ConflictStatus::Open);
    }

    #[tokio::test]
    async fn list_groups_filters_by_status() {
        let repo = SqliteConflictRepository::new_in_memory().unwrap();
        let open = ConflictGroup::new("a".to_string(), vec![EntityId::new()]);
        repo.save_group(&open).await.unwrap();

        let mut resolved = ConflictGroup::new("b".to_string(), vec![EntityId::new()]);
        resolved.status = ConflictStatus::Resolved;
        resolved.resolution = Some(ConflictResolution {
            winner_id: EntityId::new(),
            confidence: 0.9,
            reasons: vec!["+ test".to_string()],
            by: "user".to_string(),
            at: chrono::Utc::now(),
        });
        resolved.resolved_at = Some(chrono::Utc::now());
        repo.save_group(&resolved).await.unwrap();

        let all = repo.list_groups(None).await.unwrap();
        let only_open = repo.list_groups(Some(ConflictStatus::Open)).await.unwrap();
        let only_resolved = repo
            .list_groups(Some(ConflictStatus::Resolved))
            .await
            .unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(only_open.len(), 1);
        assert_eq!(only_open[0].id, open.id);
        assert_eq!(only_resolved.len(), 1);
        assert_eq!(only_resolved[0].resolution.as_ref().unwrap().by, "user");
    }

    #[tokio::test]
    async fn update_group_persists_resolution() {
        let repo = SqliteConflictRepository::new_in_memory().unwrap();
        let winner = EntityId::new();
        let mut group =
            ConflictGroup::new("port".to_string(), vec![EntityId::new(), EntityId::new()]);
        repo.save_group(&group).await.unwrap();

        group.status = ConflictStatus::Resolved;
        group.resolved_at = Some(chrono::Utc::now());
        group.resolution = Some(ConflictResolution {
            winner_id: winner.clone(),
            confidence: 0.94,
            reasons: vec!["+ recent source".to_string()],
            by: "engine".to_string(),
            at: chrono::Utc::now(),
        });
        repo.update_group(&group).await.unwrap();

        let loaded = repo.get_group(&group.id).await.unwrap().unwrap();
        assert_eq!(loaded.status, ConflictStatus::Resolved);
        let res = loaded.resolution.unwrap();
        assert_eq!(res.winner_id, winner);
        assert_eq!(res.confidence, 0.94);
        assert_eq!(res.by, "engine");
    }

    #[tokio::test]
    async fn open_groups_containing_filters_by_membership() {
        let repo = SqliteConflictRepository::new_in_memory().unwrap();
        let target = EntityId::new();
        let other = EntityId::new();

        let g1 = ConflictGroup::new("t1".to_string(), vec![target.clone(), EntityId::new()]);
        repo.save_group(&g1).await.unwrap();
        let g2 = ConflictGroup::new("t2".to_string(), vec![EntityId::new(), other.clone()]);
        repo.save_group(&g2).await.unwrap();

        let found = repo.open_groups_containing(&target).await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, g1.id);

        // Resolved groups never appear as open.
        let mut resolved = g2.clone();
        resolved.status = ConflictStatus::Resolved;
        repo.update_group(&resolved).await.unwrap();
        let found_other = repo.open_groups_containing(&other).await.unwrap();
        assert!(found_other.is_empty());
    }
}
