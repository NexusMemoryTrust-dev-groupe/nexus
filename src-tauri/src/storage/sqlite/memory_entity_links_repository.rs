use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use std::sync::Mutex;

use crate::core::entity_id::EntityId;
use crate::core::result::{AppError, Result};
use crate::storage::sqlite::schema;

/// A link between a memory and an entity.
/// Fields `id`, `created_at`, `created_by` are stored in DB for audit trail but not read back.
#[derive(Debug, Clone)]
#[allow(dead_code)] // DB-mapped struct: fields written by DB layer, not read by code
pub struct MemoryEntityLink {
    pub id: EntityId,
    pub memory_id: EntityId,
    pub entity_id: EntityId,
    pub relationship: String,
    pub weight: f64,
    pub created_at: String,
    pub created_by: String,
}

/// Trait for memory-entity link operations.
#[async_trait]
pub trait MemoryEntityLinkRepository: Send + Sync {
    /// Create a new link between a memory and an entity.
    async fn create_link(
        &self,
        memory_id: &EntityId,
        entity_id: &EntityId,
        relationship: &str,
        weight: f64,
    ) -> Result<EntityId>;

    /// Get all links for a memory.
    async fn get_links_for_memory(&self, memory_id: &EntityId) -> Result<Vec<MemoryEntityLink>>;

    /// Get all links for an entity.
    async fn get_links_for_entity(&self, entity_id: &EntityId) -> Result<Vec<MemoryEntityLink>>;

    /// Delete a link.
    async fn delete_link(
        &self,
        memory_id: &EntityId,
        entity_id: &EntityId,
        relationship: &str,
    ) -> Result<()>;

    /// Delete all links for a memory.
    async fn delete_links_for_memory(&self, memory_id: &EntityId) -> Result<()>;

    /// Delete all links for an entity.
    async fn delete_links_for_entity(&self, entity_id: &EntityId) -> Result<()>;
}

/// SQLite-backed implementation of MemoryEntityLinkRepository.
pub struct SqliteMemoryEntityLinkRepository {
    conn: Mutex<Connection>,
}

impl SqliteMemoryEntityLinkRepository {
    /// Create a new repository from an existing connection.
    pub fn new(conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        schema::apply_migrations(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create a new in-memory SQLite repository (for testing).
    #[cfg(test)]
    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::new(conn)
    }
}

fn row_to_link(row: &rusqlite::Row) -> rusqlite::Result<MemoryEntityLink> {
    let id_str: String = row.get(0)?;
    let memory_id_str: String = row.get(1)?;
    let entity_id_str: String = row.get(2)?;
    let relationship: String = row.get(3)?;
    let weight: f64 = row.get(4)?;
    let created_at: String = row.get(5)?;
    let created_by: String = row.get(6)?;

    Ok(MemoryEntityLink {
        id: EntityId::parse(&id_str)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        memory_id: EntityId::parse(&memory_id_str)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        entity_id: EntityId::parse(&entity_id_str)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        relationship,
        weight,
        created_at,
        created_by,
    })
}

#[async_trait]
impl MemoryEntityLinkRepository for SqliteMemoryEntityLinkRepository {
    async fn create_link(
        &self,
        memory_id: &EntityId,
        entity_id: &EntityId,
        relationship: &str,
        weight: f64,
    ) -> Result<EntityId> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let id = EntityId::new();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT OR IGNORE INTO memory_entity_links (
                id, memory_id, entity_id, relationship, weight, created_at, created_by
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id.as_str(),
                memory_id.as_str(),
                entity_id.as_str(),
                relationship,
                weight,
                now,
                "system",
            ],
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(id)
    }

    async fn get_links_for_memory(&self, memory_id: &EntityId) -> Result<Vec<MemoryEntityLink>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, memory_id, entity_id, relationship, weight, created_at, created_by
                 FROM memory_entity_links WHERE memory_id = ?1",
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let rows = stmt
            .query_map(params![memory_id.as_str()], row_to_link)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut links = Vec::new();
        for row in rows {
            links.push(row.map_err(|e| AppError::Internal(e.to_string()))?);
        }
        Ok(links)
    }

    async fn get_links_for_entity(&self, entity_id: &EntityId) -> Result<Vec<MemoryEntityLink>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, memory_id, entity_id, relationship, weight, created_at, created_by
                 FROM memory_entity_links WHERE entity_id = ?1",
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let rows = stmt
            .query_map(params![entity_id.as_str()], row_to_link)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut links = Vec::new();
        for row in rows {
            links.push(row.map_err(|e| AppError::Internal(e.to_string()))?);
        }
        Ok(links)
    }

    async fn delete_link(
        &self,
        memory_id: &EntityId,
        entity_id: &EntityId,
        relationship: &str,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let rows = conn
            .execute(
                "DELETE FROM memory_entity_links 
                 WHERE memory_id = ?1 AND entity_id = ?2 AND relationship = ?3",
                params![memory_id.as_str(), entity_id.as_str(), relationship],
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;

        if rows == 0 {
            return Err(AppError::NotFound(format!(
                "Link not found for memory {} -> entity {} ({})",
                memory_id, entity_id, relationship
            )));
        }
        Ok(())
    }

    async fn delete_links_for_memory(&self, memory_id: &EntityId) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        conn.execute(
            "DELETE FROM memory_entity_links WHERE memory_id = ?1",
            params![memory_id.as_str()],
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn delete_links_for_entity(&self, entity_id: &EntityId) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        conn.execute(
            "DELETE FROM memory_entity_links WHERE entity_id = ?1",
            params![entity_id.as_str()],
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> SqliteMemoryEntityLinkRepository {
        SqliteMemoryEntityLinkRepository::new_in_memory().unwrap()
    }

    #[tokio::test]
    async fn create_and_get_link() {
        let r = repo();
        let memory_id = EntityId::new();
        let entity_id = EntityId::new();
        let id = r
            .create_link(&memory_id, &entity_id, "Related", 1.0)
            .await
            .unwrap();
        assert!(!id.as_str().is_empty());

        let links = r.get_links_for_memory(&memory_id).await.unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].entity_id, entity_id);
    }

    #[tokio::test]
    async fn get_links_for_entity() {
        let r = repo();
        let memory_id = EntityId::new();
        let entity_id = EntityId::new();
        r.create_link(&memory_id, &entity_id, "Related", 1.0)
            .await
            .unwrap();

        let links = r.get_links_for_entity(&entity_id).await.unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].memory_id, memory_id);
    }

    #[tokio::test]
    async fn delete_link() {
        let r = repo();
        let memory_id = EntityId::new();
        let entity_id = EntityId::new();
        r.create_link(&memory_id, &entity_id, "Related", 1.0)
            .await
            .unwrap();

        r.delete_link(&memory_id, &entity_id, "Related")
            .await
            .unwrap();
        let links = r.get_links_for_memory(&memory_id).await.unwrap();
        assert!(links.is_empty());
    }

    #[tokio::test]
    async fn delete_links_for_memory() {
        let r = repo();
        let memory_id = EntityId::new();
        let entity1 = EntityId::new();
        let entity2 = EntityId::new();
        r.create_link(&memory_id, &entity1, "Related", 1.0)
            .await
            .unwrap();
        r.create_link(&memory_id, &entity2, "Related", 1.0)
            .await
            .unwrap();

        r.delete_links_for_memory(&memory_id).await.unwrap();
        let links = r.get_links_for_memory(&memory_id).await.unwrap();
        assert!(links.is_empty());
    }

    #[tokio::test]
    async fn count_links() {
        let r = repo();
        let memory_id = EntityId::new();
        let entity1 = EntityId::new();
        let entity2 = EntityId::new();
        r.create_link(&memory_id, &entity1, "Related", 1.0)
            .await
            .unwrap();
        r.create_link(&memory_id, &entity2, "Related", 1.0)
            .await
            .unwrap();

        let mem_links = r.get_links_for_memory(&memory_id).await.unwrap();
        assert_eq!(mem_links.len(), 2);
        let ent_links = r.get_links_for_entity(&entity1).await.unwrap();
        assert_eq!(ent_links.len(), 1);
    }

    #[tokio::test]
    async fn duplicate_link_ignored() {
        let r = repo();
        let memory_id = EntityId::new();
        let entity_id = EntityId::new();
        r.create_link(&memory_id, &entity_id, "Related", 1.0)
            .await
            .unwrap();
        r.create_link(&memory_id, &entity_id, "Related", 0.5)
            .await
            .unwrap();

        let links = r.get_links_for_memory(&memory_id).await.unwrap();
        assert_eq!(links.len(), 1);
    }

    #[tokio::test]
    async fn different_relationships_allowed() {
        let r = repo();
        let memory_id = EntityId::new();
        let entity_id = EntityId::new();
        r.create_link(&memory_id, &entity_id, "Related", 1.0)
            .await
            .unwrap();
        r.create_link(&memory_id, &entity_id, "CausedBy", 0.8)
            .await
            .unwrap();

        let links = r.get_links_for_memory(&memory_id).await.unwrap();
        assert_eq!(links.len(), 2);
    }

    #[tokio::test]
    async fn delete_nonexistent_link_fails() {
        let r = repo();
        let memory_id = EntityId::new();
        let entity_id = EntityId::new();
        let result = r.delete_link(&memory_id, &entity_id, "Related").await;
        assert!(result.is_err());
    }
}
