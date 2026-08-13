use rusqlite::{Connection, OptionalExtension, params};
use std::sync::Mutex;

use crate::core::memory::canonical_consolidation::Cluster;
use crate::core::result::{AppError, Result};
use crate::storage::sqlite::schema;

/// A consolidated canonical memory persisted by the Rehearsal sleep cycle.
#[derive(Debug, Clone)]
pub struct CanonicalMemory {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub content: String,
    pub author: String,
    /// Ids of the source memory records this was derived from (provenance).
    pub member_ids: Vec<String>,
    /// Average pairwise similarity inside the cluster (0.0–1.0).
    pub cohesion: f64,
    pub importance_score: f64,
    pub confidence_score: f64,
    pub layer: String,
    pub created_at: String,
    /// Id of the canonical MemoryRecord in the main memory table.
    pub source_memory_id: Option<String>,
}

impl CanonicalMemory {
    /// Build a persisted record from a cluster and the synthesized record.
    pub fn from_parts(
        canonical_record: &crate::core::memory::memory_record::MemoryRecord,
        cluster: &Cluster,
        cohesion: f64,
    ) -> Self {
        Self {
            id: canonical_record.id.as_str().to_string(),
            title: canonical_record.title.clone(),
            summary: canonical_record.summary.clone(),
            content: canonical_record.content.clone(),
            author: canonical_record.author.clone(),
            member_ids: cluster.member_ids.clone(),
            cohesion,
            importance_score: canonical_record.importance_score,
            confidence_score: canonical_record.confidence_score,
            layer: canonical_record.layer.as_str().to_string(),
            created_at: canonical_record.created_at.to_rfc3339(),
            source_memory_id: Some(canonical_record.id.as_str().to_string()),
        }
    }
}

/// SQLite-backed storage for canonical memories (Rehearsal consolidation).
pub struct SqliteCanonicalRepository {
    conn: Mutex<Connection>,
}

impl SqliteCanonicalRepository {
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

    /// Save a canonical memory (insert or update by id).
    pub fn save(&self, cm: &CanonicalMemory) -> Result<()> {
        let member_json = serde_json::to_string(&cm.member_ids)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO canonical_memories
                (id, title, summary, content, author, member_ids, member_count, cohesion,
                 importance_score, confidence_score, layer, created_at, source_memory_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(id) DO UPDATE SET
                title=excluded.title,
                summary=excluded.summary,
                content=excluded.content,
                member_ids=excluded.member_ids,
                member_count=excluded.member_count,
                cohesion=excluded.cohesion,
                importance_score=excluded.importance_score,
                confidence_score=excluded.confidence_score,
                layer=excluded.layer,
                source_memory_id=excluded.source_memory_id",
            params![
                cm.id,
                cm.title,
                cm.summary,
                cm.content,
                cm.author,
                member_json,
                cm.member_ids.len() as i64,
                cm.cohesion,
                cm.importance_score,
                cm.confidence_score,
                cm.layer,
                cm.created_at,
                cm.source_memory_id,
            ],
        )?;
        Ok(())
    }

    /// Check whether a cluster was already consolidated (idempotency guard).
    pub fn exists_cluster(&self, member_ids: &[String]) -> Result<bool> {
        if member_ids.is_empty() {
            return Ok(false);
        }
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT 1 FROM canonical_memories WHERE member_ids = ?1 LIMIT 1")?;
        // Compare JSON arrays canonically: sort for stability.
        let mut sorted = member_ids.to_vec();
        sorted.sort();
        let json = serde_json::to_string(&sorted)?;
        let found = stmt
            .query_row(params![json], |_| Ok(()))
            .optional()?
            .is_some();
        Ok(found)
    }

    /// All canonical memories, newest first.
    pub fn list(&self, limit: u32) -> Result<Vec<CanonicalMemory>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, summary, content, author, member_ids, cohesion,
                    importance_score, confidence_score, layer, created_at, source_memory_id
             FROM canonical_memories
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            let member_json: String = row.get(5)?;
            Ok(CanonicalMemory {
                id: row.get(0)?,
                title: row.get(1)?,
                summary: row.get(2)?,
                content: row.get(3)?,
                author: row.get(4)?,
                member_ids: serde_json::from_str(&member_json).unwrap_or_default(),
                cohesion: row.get(6)?,
                importance_score: row.get(7)?,
                confidence_score: row.get(8)?,
                layer: row.get(9)?,
                created_at: row.get(10)?,
                source_memory_id: row.get(11)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// How many canonical memories were consolidated so far.
    pub fn count(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM canonical_memories", [], |row| {
            row.get(0)
        })?;
        Ok(n as u64)
    }

    /// Delete a canonical memory by id.
    #[allow(dead_code)] // Public API — used by tests; exposed to commands on demand
    pub fn delete(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute("DELETE FROM canonical_memories WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(AppError::NotFound(format!(
                "Canonical memory '{}' not found",
                id
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::memory_record::MemoryRecord;
    use crate::core::memory::types::MemorySource;

    fn sample_cm(author: &str) -> CanonicalMemory {
        let mut rec = MemoryRecord::new(
            "Auth".to_string(),
            "Authentication uses JWT access tokens".to_string(),
            author.to_string(),
            MemorySource::Compressed,
        )
        .unwrap();
        rec.summary =
            "Authentication uses JWT access tokens and rotating refresh tokens".to_string();
        CanonicalMemory::from_parts(
            &rec,
            &Cluster {
                member_ids: vec!["mem-1".to_string(), "mem-2".to_string()],
                member_titles: vec!["Auth".to_string(), "Auth details".to_string()],
                cohesion: 0.85,
            },
            0.85,
        )
    }

    #[test]
    fn save_and_list_roundtrip() {
        let repo = SqliteCanonicalRepository::new_in_memory().unwrap();
        let cm = sample_cm("nexus");
        repo.save(&cm).unwrap();
        let all = repo.list(10).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].title, "Auth");
        assert_eq!(all[0].member_ids.len(), 2);
        assert!(all[0].source_memory_id.is_some());
        assert_eq!(repo.count().unwrap(), 1);
    }

    #[test]
    fn exists_cluster_is_order_insensitive() {
        let repo = SqliteCanonicalRepository::new_in_memory().unwrap();
        let cm = sample_cm("nexus");
        repo.save(&cm).unwrap();
        // Same members, different order — still detected as consolidated.
        assert!(
            repo.exists_cluster(&["mem-2".to_string(), "mem-1".to_string()])
                .unwrap()
        );
        assert!(!repo.exists_cluster(&["mem-9".to_string()]).unwrap());
    }

    #[test]
    fn delete_removes_and_missing_errors() {
        let repo = SqliteCanonicalRepository::new_in_memory().unwrap();
        let cm = sample_cm("nexus");
        repo.save(&cm).unwrap();
        repo.delete(&cm.id).unwrap();
        assert_eq!(repo.count().unwrap(), 0);
        assert!(repo.delete(&cm.id).is_err());
    }
}
