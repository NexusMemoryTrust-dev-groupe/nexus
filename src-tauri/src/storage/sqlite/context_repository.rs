use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use std::sync::Mutex;

use crate::core::context::context_package::ContextPackage;
use crate::core::context::context_snapshot::ContextSnapshot;
use crate::core::context::context_store::ContextStore;
use crate::core::entity_id::EntityId;
use crate::core::result::{AppError, Result};

/// SQLite-backed context snapshot store.
pub struct SqliteContextRepository {
    conn: Mutex<Connection>,
}

impl SqliteContextRepository {
    /// Create a new repository from an existing connection.
    #[allow(dead_code)] // Used in tests and available for future command layer
    pub fn new(conn: Connection) -> Result<Self> {
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

fn row_to_snapshot(row: &rusqlite::Row) -> rusqlite::Result<ContextSnapshot> {
    let id: String = row.get(0)?;
    let entity_id_str: String = row.get(1)?;
    let package_json: String = row.get(2)?;
    let created_at: String = row.get(3)?;
    let label: Option<String> = row.get(4)?;

    let package: ContextPackage = serde_json::from_str(&package_json)
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

    Ok(ContextSnapshot {
        id,
        entity_id: EntityId::parse(&entity_id_str)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        package,
        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        label,
    })
}

#[async_trait]
impl ContextStore for SqliteContextRepository {
    async fn save_snapshot(&self, snapshot: &ContextSnapshot) -> Result<String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let package_json = serde_json::to_string(&snapshot.package)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        conn.execute(
            "INSERT INTO context_snapshots (id, entity_id, package_json, created_at, label)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                snapshot.id,
                snapshot.entity_id.as_str(),
                package_json,
                snapshot.created_at.to_rfc3339(),
                snapshot.label,
            ],
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(snapshot.id.clone())
    }

    async fn get_snapshot(&self, snapshot_id: &str) -> Result<Option<ContextSnapshot>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, entity_id, package_json, created_at, label
                 FROM context_snapshots WHERE id = ?1",
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;
        stmt.query_row(params![snapshot_id], row_to_snapshot)
            .optional()
            .map_err(|e| AppError::Internal(e.to_string()))
    }

    async fn list_snapshots(&self, entity_id: &EntityId) -> Result<Vec<ContextSnapshot>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, entity_id, package_json, created_at, label
                 FROM context_snapshots WHERE entity_id = ?1
                 ORDER BY created_at DESC",
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let rows = stmt
            .query_map(params![entity_id.as_str()], row_to_snapshot)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        rows.map(|r| r.map_err(|e| AppError::Internal(e.to_string())))
            .collect()
    }

    async fn list_all_snapshots(&self) -> Result<Vec<ContextSnapshot>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, entity_id, package_json, created_at, label
                 FROM context_snapshots ORDER BY created_at DESC",
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let rows = stmt
            .query_map([], row_to_snapshot)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        rows.map(|r| r.map_err(|e| AppError::Internal(e.to_string())))
            .collect()
    }

    async fn restore_snapshot(&self, snapshot_id: &str) -> Result<ContextPackage> {
        let snapshot = self
            .get_snapshot(snapshot_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Snapshot {} not found", snapshot_id)))?;
        Ok(snapshot.package)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::context_package::{IntentType, UserIntent};

    fn repo() -> SqliteContextRepository {
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::sqlite::schema::apply_migrations(&conn).unwrap();
        SqliteContextRepository::new(conn).unwrap()
    }

    fn sample_snapshot() -> ContextSnapshot {
        let pkg = ContextPackage::new(UserIntent {
            query: "test".to_string(),
            intent_type: IntentType::Search,
            confidence: 0.8,
            keywords: vec!["test".to_string()],
            temporal: None,
        });
        ContextSnapshot::new(EntityId::new(), pkg, Some("test".to_string()))
    }

    #[tokio::test]
    async fn save_and_get_snapshot() {
        let r = repo();
        let snap = sample_snapshot();
        let id = r.save_snapshot(&snap).await.unwrap();
        let fetched = r.get_snapshot(&id).await.unwrap().unwrap();
        assert_eq!(fetched.id, snap.id);
        assert_eq!(fetched.label, Some("test".to_string()));
    }

    #[tokio::test]
    async fn get_nonexistent_snapshot() {
        let r = repo();
        assert!(r.get_snapshot("nonexistent").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_snapshots() {
        let r = repo();
        let eid = EntityId::new();
        let pkg = ContextPackage::new(UserIntent {
            query: "q".to_string(),
            intent_type: IntentType::Search,
            confidence: 0.8,
            keywords: vec!["q".to_string()],
            temporal: None,
        });
        let snap1 = ContextSnapshot::new(eid.clone(), pkg.clone(), Some("s1".to_string()));
        let snap2 = ContextSnapshot::new(eid.clone(), pkg, Some("s2".to_string()));
        r.save_snapshot(&snap1).await.unwrap();
        r.save_snapshot(&snap2).await.unwrap();

        let list = r.list_snapshots(&eid).await.unwrap();
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn restore_snapshot() {
        let r = repo();
        let snap = sample_snapshot();
        let pkg = snap.package.clone();
        let id = r.save_snapshot(&snap).await.unwrap();
        let restored = r.restore_snapshot(&id).await.unwrap();
        assert_eq!(restored.id, pkg.id);
    }

    #[tokio::test]
    async fn restore_nonexistent_fails() {
        let r = repo();
        assert!(r.restore_snapshot("nope").await.is_err());
    }
}
