use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use std::sync::Mutex;

use crate::core::entity_id::EntityId;
use crate::core::result::{AppError, Result};
use crate::core::versioning::automatic_commit::{AutomaticCommit, ChangeType};
use crate::core::versioning::causality_record::CausalityRecord;
use crate::core::versioning::commit_service::{CommitService, CreateCommitParams};
use crate::core::versioning::version_edge::{VersionEdge, VersionEdgeType};

pub struct SqliteVersioningRepository {
    conn: Mutex<Connection>,
}

impl SqliteVersioningRepository {
    pub fn new(conn: Connection) -> Result<Self> {
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

fn row_to_commit(row: &rusqlite::Row) -> rusqlite::Result<AutomaticCommit> {
    let id: String = row.get(0)?;
    let hash: String = row.get(1)?;
    let version_number: u32 = row.get(2)?;
    let entity_type: String = row.get(3)?;
    let entity_id_str: String = row.get(4)?;
    let change_type_str: String = row.get(5)?;
    let diff: Option<String> = row.get(6)?;
    let baseline_snapshot_id: Option<String> = row.get(7)?;
    let is_baseline: bool = row.get::<_, i32>(8)? != 0;
    let created_at: String = row.get(9)?;
    let created_by: String = row.get(10)?;
    let triggering_event_type: String = row.get(11)?;
    let triggering_event_id: String = row.get(12)?;
    let change_reason: Option<String> = row.get(13)?;
    let linked_entity_ids_json: String = row.get(14)?;
    let linked_decision_ids_json: String = row.get(15)?;
    let is_indexed: bool = row.get::<_, i32>(16)? != 0;
    let is_archived: bool = row.get::<_, i32>(17)? != 0;
    let size_bytes: u64 = row.get::<_, i64>(18)? as u64;

    let linked_entity_ids: Vec<String> =
        serde_json::from_str(&linked_entity_ids_json).unwrap_or_default();
    let linked_decision_ids: Vec<String> =
        serde_json::from_str(&linked_decision_ids_json).unwrap_or_default();

    Ok(AutomaticCommit {
        id,
        hash,
        version_number,
        entity_type,
        entity_id: EntityId::parse(&entity_id_str)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        change_type: match change_type_str.as_str() {
            "Created" => ChangeType::Created,
            "Modified" => ChangeType::Modified,
            "Deleted" => ChangeType::Deleted,
            _ => ChangeType::Modified,
        },
        diff,
        baseline_snapshot_id,
        is_baseline,
        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        created_by,
        triggering_event_type,
        triggering_event_id,
        change_reason,
        linked_entity_ids,
        linked_decision_ids,
        is_indexed,
        is_archived,
        size_bytes,
    })
}

fn change_type_to_string(ct: &ChangeType) -> String {
    match ct {
        ChangeType::Created => "Created",
        ChangeType::Modified => "Modified",
        ChangeType::Deleted => "Deleted",
    }
    .to_string()
}

#[async_trait]
impl CommitService for SqliteVersioningRepository {
    async fn create_automatic_commit(&self, params: CreateCommitParams) -> Result<AutomaticCommit> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        // Get next version number
        let version_number: u32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version_number), 0) + 1 FROM automatic_commits WHERE entity_id = ?1",
                params![params.entity_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let id = EntityId::new().as_str().to_string();
        let data_str =
            serde_json::to_string(&params.data).map_err(|e| AppError::Internal(e.to_string()))?;
        let linked_json =
            serde_json::to_string(&params.linked_entities.clone().unwrap_or_default())
                .map_err(|e| AppError::Internal(e.to_string()))?;

        // Simple deterministic hash for commit integrity
        let hash_input = format!("{}:{}", id, version_number);
        let hash = commit_hash(&hash_input);

        let commit = AutomaticCommit {
            id: id.clone(),
            hash: hash.clone(),
            version_number,
            entity_type: params.entity_type.clone(),
            entity_id: params.entity_id.clone(),
            change_type: params.change_type.clone(),
            diff: params.diff.clone(),
            baseline_snapshot_id: None,
            is_baseline: version_number.is_multiple_of(20),
            created_at: chrono::Utc::now(),
            created_by: "system".to_string(),
            triggering_event_type: params.triggering_event_type.clone(),
            triggering_event_id: params.triggering_event_id.clone(),
            change_reason: params.change_reason.clone(),
            linked_entity_ids: params.linked_entities.clone().unwrap_or_default(),
            linked_decision_ids: vec![],
            is_indexed: false,
            is_archived: false,
            size_bytes: data_str.len() as u64,
        };

        conn.execute(
            "INSERT INTO automatic_commits (
                id, hash, version_number, entity_type, entity_id, change_type,
                diff_json, baseline_snapshot_id, is_baseline, created_at, created_by,
                triggering_event_type, triggering_event_id, change_reason,
                linked_entity_ids_json, linked_decision_ids_json, is_indexed, is_archived, size_bytes
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            params![
                commit.id,
                commit.hash,
                commit.version_number,
                commit.entity_type,
                commit.entity_id.as_str(),
                change_type_to_string(&commit.change_type),
                commit.diff,
                commit.baseline_snapshot_id,
                commit.is_baseline as i32,
                commit.created_at.to_rfc3339(),
                commit.created_by,
                commit.triggering_event_type,
                commit.triggering_event_id,
                commit.change_reason,
                linked_json,
                "[]",
                commit.is_indexed as i32,
                commit.is_archived as i32,
                commit.size_bytes as i64,
            ],
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(commit)
    }

    async fn get_commit(&self, commit_id: &str) -> Result<Option<AutomaticCommit>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, hash, version_number, entity_type, entity_id, change_type,
                    diff_json, baseline_snapshot_id, is_baseline, created_at, created_by,
                    triggering_event_type, triggering_event_id, change_reason,
                    linked_entity_ids_json, linked_decision_ids_json, is_indexed, is_archived, size_bytes
                 FROM automatic_commits WHERE id = ?1",
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;
        stmt.query_row(params![commit_id], row_to_commit)
            .optional()
            .map_err(|e| AppError::Internal(e.to_string()))
    }

    async fn get_entity_history(
        &self,
        entity_type: &str,
        entity_id: &EntityId,
    ) -> Result<Vec<AutomaticCommit>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, hash, version_number, entity_type, entity_id, change_type,
                    diff_json, baseline_snapshot_id, is_baseline, created_at, created_by,
                    triggering_event_type, triggering_event_id, change_reason,
                    linked_entity_ids_json, linked_decision_ids_json, is_indexed, is_archived, size_bytes
                 FROM automatic_commits
                 WHERE entity_type = ?1 AND entity_id = ?2
                 ORDER BY version_number ASC",
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let rows = stmt
            .query_map(params![entity_type, entity_id.as_str()], row_to_commit)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let mut commits = Vec::new();
        for row in rows {
            commits.push(row.map_err(|e| AppError::Internal(e.to_string()))?);
        }
        Ok(commits)
    }

    async fn get_baseline(
        &self,
        entity_type: &str,
        entity_id: &EntityId,
    ) -> Result<Option<AutomaticCommit>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, hash, version_number, entity_type, entity_id, change_type,
                    diff_json, baseline_snapshot_id, is_baseline, created_at, created_by,
                    triggering_event_type, triggering_event_id, change_reason,
                    linked_entity_ids_json, linked_decision_ids_json, is_indexed, is_archived, size_bytes
                 FROM automatic_commits
                 WHERE entity_type = ?1 AND entity_id = ?2 AND is_baseline = 1
                 ORDER BY version_number DESC LIMIT 1",
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;
        stmt.query_row(params![entity_type, entity_id.as_str()], row_to_commit)
            .optional()
            .map_err(|e| AppError::Internal(e.to_string()))
    }
}

// ── SnapshotService ─────────────────────────────────────────────────

#[async_trait]
impl crate::core::versioning::snapshot_service::SnapshotService for SqliteVersioningRepository {
    async fn capture(
        &self,
        entity_type: &str,
        entity_id: &EntityId,
    ) -> crate::core::Result<Vec<u8>> {
        // Build a snapshot from the entity's latest state by serializing its commit history
        let history = self.get_entity_history(entity_type, entity_id).await?;
        let snapshot_data = serde_json::to_vec(&history)
            .map_err(|e| crate::core::AppError::Serialization(e.to_string()))?;
        Ok(snapshot_data)
    }

    async fn store(
        &self,
        snapshot: &[u8],
        entity_type: &str,
        entity_id: &EntityId,
    ) -> crate::core::Result<String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        let id = EntityId::new().as_str().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let size = snapshot.len() as i64;

        conn.execute(
            "INSERT INTO entity_snapshots (id, entity_type, entity_id, snapshot_data, size_bytes, is_baseline, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
            params![id, entity_type, entity_id.as_str(), snapshot, size, now],
        ).map_err(|e| crate::core::AppError::Internal(e.to_string()))?;

        Ok(id)
    }

    async fn get(&self, snapshot_id: &str) -> crate::core::Result<Option<Vec<u8>>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT snapshot_data FROM entity_snapshots WHERE id = ?1")
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        let result = stmt
            .query_row(params![snapshot_id], |row| row.get::<_, Vec<u8>>(0))
            .optional()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        Ok(result)
    }

    async fn get_baseline(
        &self,
        entity_type: &str,
        entity_id: &EntityId,
    ) -> crate::core::Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id FROM entity_snapshots
                 WHERE entity_type = ?1 AND entity_id = ?2 AND is_baseline = 1
                 ORDER BY created_at DESC LIMIT 1",
            )
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        let result = stmt
            .query_row(params![entity_type, entity_id.as_str()], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        Ok(result)
    }
}

// ── CausalityChain ─────────────────────────────────────────────────

#[async_trait]
impl crate::core::versioning::causality_chain::CausalityChain for SqliteVersioningRepository {
    async fn trace_causes(
        &self,
        entity_id: &EntityId,
        version_id: &str,
    ) -> crate::core::Result<Vec<crate::core::versioning::causality_record::CausalityRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, entity_id, version_id, reason, affected_entities_json, created_at
                 FROM causality_records
                 WHERE entity_id = ?1 AND version_id = ?2
                 ORDER BY created_at ASC",
            )
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;

        let rows = stmt
            .query_map(params![entity_id.as_str(), version_id], |row| {
                let id: String = row.get(0)?;
                let eid: String = row.get(1)?;
                let vid: String = row.get(2)?;
                let reason: String = row.get(3)?;
                let affected_json: String = row.get(4)?;
                let created_at: String = row.get(5)?;
                Ok((id, eid, vid, reason, affected_json, created_at))
            })
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;

        let mut records = Vec::new();
        for row in rows {
            let (id, eid_str, vid, reason, affected_json, created_at_str) =
                row.map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
            let affected: Vec<String> = serde_json::from_str(&affected_json).unwrap_or_default();
            let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            records.push(crate::core::versioning::causality_record::CausalityRecord {
                id,
                entity_id: EntityId::parse(&eid_str)
                    .map_err(|e| crate::core::AppError::Internal(e.to_string()))?,
                version_id: vid,
                reason,
                affected_entities: affected,
                created_at,
            });
        }
        Ok(records)
    }

    async fn find_effects(
        &self,
        cause_id: &str,
    ) -> crate::core::Result<Vec<crate::core::versioning::causality_record::CausalityRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        // Find all causality records where the cause_id appears in affected_entities
        let mut stmt = conn
            .prepare(
                "SELECT id, entity_id, version_id, reason, affected_entities_json, created_at
                 FROM causality_records
                 WHERE affected_entities_json LIKE ?1
                 ORDER BY created_at ASC",
            )
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;

        let pattern = format!("%\"{}\"%", cause_id);
        let rows = stmt
            .query_map(params![pattern], |row| {
                let id: String = row.get(0)?;
                let eid: String = row.get(1)?;
                let vid: String = row.get(2)?;
                let reason: String = row.get(3)?;
                let affected_json: String = row.get(4)?;
                let created_at: String = row.get(5)?;
                Ok((id, eid, vid, reason, affected_json, created_at))
            })
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;

        let mut records = Vec::new();
        for row in rows {
            let (id, eid_str, vid, reason, affected_json, created_at_str) =
                row.map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
            let affected: Vec<String> = serde_json::from_str(&affected_json).unwrap_or_default();
            // Verify the cause_id is actually in the affected list (not a substring match)
            if !affected.iter().any(|a| a == cause_id) {
                continue;
            }
            let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            records.push(crate::core::versioning::causality_record::CausalityRecord {
                id,
                entity_id: EntityId::parse(&eid_str)
                    .map_err(|e| crate::core::AppError::Internal(e.to_string()))?,
                version_id: vid,
                reason,
                affected_entities: affected,
                created_at,
            });
        }
        Ok(records)
    }

    async fn record_causality(
        &self,
        entity_id: &EntityId,
        version_id: &str,
        reason: &str,
        affected: &[EntityId],
    ) -> crate::core::Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        let id = EntityId::new().as_str().to_string();
        let affected_json = serde_json::to_vec(affected)
            .map_err(|e| crate::core::AppError::Serialization(e.to_string()))?;
        let affected_str = String::from_utf8(affected_json)
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO causality_records (id, entity_id, version_id, reason, affected_entities_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, entity_id.as_str(), version_id, reason, affected_str, now],
        )
        .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        Ok(())
    }
}

// ── VersionGraph ───────────────────────────────────────────────────

#[async_trait]
impl crate::core::versioning::version_graph::VersionGraph for SqliteVersioningRepository {
    async fn get_lineage(&self, entity_id: &EntityId) -> crate::core::Result<Vec<AutomaticCommit>> {
        // Lineage = full commit history for this entity across ALL entity types
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, hash, version_number, entity_type, entity_id, change_type,
                    diff_json, baseline_snapshot_id, is_baseline, created_at, created_by,
                    triggering_event_type, triggering_event_id, change_reason,
                    linked_entity_ids_json, linked_decision_ids_json, is_indexed, is_archived, size_bytes
                 FROM automatic_commits
                 WHERE entity_id = ?1
                 ORDER BY version_number ASC",
            )
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        let rows = stmt
            .query_map(params![entity_id.as_str()], row_to_commit)
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        let mut commits = Vec::new();
        for row in rows {
            commits.push(row.map_err(|e| crate::core::AppError::Internal(e.to_string()))?);
        }
        Ok(commits)
    }

    async fn get_dependents(&self, version_id: &str) -> crate::core::Result<Vec<AutomaticCommit>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        // Find all commits that have an edge FROM version_id
        let mut stmt = conn
            .prepare(
                "SELECT ac.id, ac.hash, ac.version_number, ac.entity_type, ac.entity_id,
                        ac.change_type, ac.diff_json, ac.baseline_snapshot_id, ac.is_baseline,
                        ac.created_at, ac.created_by, ac.triggering_event_type,
                        ac.triggering_event_id, ac.change_reason,
                        ac.linked_entity_ids_json, ac.linked_decision_ids_json,
                        ac.is_indexed, ac.is_archived, ac.size_bytes
                 FROM automatic_commits ac
                 JOIN version_edges ve ON ve.to_version_id = ac.id
                 WHERE ve.from_version_id = ?1
                 ORDER BY ac.version_number ASC",
            )
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;

        let rows = stmt
            .query_map(params![version_id], row_to_commit)
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;

        let mut commits = Vec::new();
        for row in rows {
            commits.push(row.map_err(|e| crate::core::AppError::Internal(e.to_string()))?);
        }
        Ok(commits)
    }

    async fn add_edge(
        &self,
        from: &str,
        to: &str,
        edge_type: crate::core::versioning::version_edge::VersionEdgeType,
    ) -> crate::core::Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        let id = EntityId::new().as_str().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let edge_type_str = match edge_type {
            crate::core::versioning::version_edge::VersionEdgeType::EvolvedTo => "EvolvedTo",
            crate::core::versioning::version_edge::VersionEdgeType::BranchedTo => "BranchedTo",
            crate::core::versioning::version_edge::VersionEdgeType::MergedWith => "MergedWith",
        };

        conn.execute(
            "INSERT INTO version_edges (id, from_version_id, to_version_id, relationship_type, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, from, to, edge_type_str, now],
        )
        .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        Ok(())
    }
}

/// Simple deterministic hash for commit integrity.
fn commit_hash(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    let h = hasher.finish();
    format!("{:016x}", h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::versioning::automatic_commit::ChangeType;

    fn repo() -> SqliteVersioningRepository {
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::sqlite::schema::apply_migrations(&conn).unwrap();
        SqliteVersioningRepository::new(conn).unwrap()
    }

    fn sample_params() -> CreateCommitParams {
        CreateCommitParams {
            entity_type: "MemoryRecord".to_string(),
            entity_id: EntityId::new(),
            change_type: ChangeType::Created,
            data: serde_json::json!({"title": "test"}),
            triggering_event_type: "EntityCreated".to_string(),
            triggering_event_id: "evt-1".to_string(),
            diff: None,
            linked_entities: None,
            change_reason: Some("test".to_string()),
        }
    }

    #[tokio::test]
    async fn create_and_get_commit() {
        let r = repo();
        let params = sample_params();
        let _eid = params.entity_id.clone();
        let commit = r.create_automatic_commit(params).await.unwrap();
        assert!(!commit.id.is_empty());
        assert_eq!(commit.version_number, 1);
        assert_eq!(commit.entity_type, "MemoryRecord");

        let fetched = r.get_commit(&commit.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, commit.id);
    }

    #[tokio::test]
    async fn version_number_increments() {
        let r = repo();
        let params = sample_params();
        let eid = params.entity_id.clone();
        let c1 = r.create_automatic_commit(params).await.unwrap();
        assert_eq!(c1.version_number, 1);

        let mut p2 = sample_params();
        p2.entity_id = eid.clone();
        let c2 = r.create_automatic_commit(p2).await.unwrap();
        assert_eq!(c2.version_number, 2);
    }

    #[tokio::test]
    async fn entity_history() {
        let r = repo();
        let params = sample_params();
        let eid = params.entity_id.clone();
        r.create_automatic_commit(params).await.unwrap();

        let mut p2 = sample_params();
        p2.entity_id = eid.clone();
        p2.change_type = ChangeType::Modified;
        r.create_automatic_commit(p2).await.unwrap();

        let history = r.get_entity_history("MemoryRecord", &eid).await.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].version_number, 1);
        assert_eq!(history[1].version_number, 2);
    }

    #[tokio::test]
    async fn get_nonexistent_commit() {
        let r = repo();
        let result = r.get_commit("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    // ── SnapshotService tests ──

    #[tokio::test]
    async fn snapshot_store_and_get() {
        use crate::core::versioning::snapshot_service::SnapshotService;

        let r = repo();
        let eid = EntityId::new();
        let data = b"snapshot binary data";
        let id = r.store(data, "MemoryRecord", &eid).await.unwrap();
        assert!(!id.is_empty());

        let fetched = r.get(&id).await.unwrap().unwrap();
        assert_eq!(fetched, data);
    }

    #[tokio::test]
    async fn snapshot_capture() {
        use crate::core::versioning::snapshot_service::SnapshotService;

        let r = repo();
        let eid = EntityId::new();
        // Capture with no history returns serialized empty vec: b"[]"
        let data = r.capture("MemoryRecord", &eid).await.unwrap();
        assert_eq!(data, b"[]");
    }

    #[tokio::test]
    async fn snapshot_capture_with_history() {
        use crate::core::versioning::snapshot_service::SnapshotService;

        let r = repo();
        let params = sample_params();
        let eid = params.entity_id.clone();
        r.create_automatic_commit(params).await.unwrap();

        let data = r.capture("MemoryRecord", &eid).await.unwrap();
        assert!(!data.is_empty());
        // Should be valid JSON (serialized commit history)
        let parsed: Vec<AutomaticCommit> = serde_json::from_slice(&data).unwrap();
        assert_eq!(parsed.len(), 1);
    }

    #[tokio::test]
    async fn snapshot_get_baseline_none() {
        use crate::core::versioning::snapshot_service::SnapshotService;

        let r = repo();
        let eid = EntityId::new();
        use crate::core::versioning::snapshot_service::SnapshotService as SS;
        assert!(
            SS::get_baseline(&r, "MemoryRecord", &eid)
                .await
                .unwrap()
                .is_none()
        );
    }

    // ── CausalityChain tests ──

    #[tokio::test]
    async fn causality_record_and_trace() {
        use crate::core::versioning::causality_chain::CausalityChain;

        let r = repo();
        let eid = EntityId::new();
        let affected = vec![EntityId::new(), EntityId::new()];

        r.record_causality(&eid, "v1", "user decided", &affected)
            .await
            .unwrap();

        let causes = r.trace_causes(&eid, "v1").await.unwrap();
        assert_eq!(causes.len(), 1);
        assert_eq!(causes[0].reason, "user decided");
        assert_eq!(causes[0].affected_entities.len(), 2);
    }

    #[tokio::test]
    async fn causality_find_effects() {
        use crate::core::versioning::causality_chain::CausalityChain;

        let r = repo();
        let eid = EntityId::new();
        let target = EntityId::new();

        r.record_causality(&eid, "v1", "triggered", std::slice::from_ref(&target))
            .await
            .unwrap();

        let effects = r.find_effects(target.as_str()).await.unwrap();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].version_id, "v1");
    }

    #[tokio::test]
    async fn causality_no_causes() {
        use crate::core::versioning::causality_chain::CausalityChain;

        let r = repo();
        let eid = EntityId::new();
        let causes = r.trace_causes(&eid, "nonexistent").await.unwrap();
        assert!(causes.is_empty());
    }

    // ── VersionGraph tests ──

    #[tokio::test]
    async fn version_graph_add_edge_and_dependents() {
        use crate::core::versioning::version_edge::VersionEdgeType;
        use crate::core::versioning::version_graph::VersionGraph;

        let r = repo();
        // Create two commits
        let params = sample_params();
        let eid = params.entity_id.clone();
        let c1 = r.create_automatic_commit(params).await.unwrap();

        let mut p2 = sample_params();
        p2.entity_id = eid.clone();
        let c2 = r.create_automatic_commit(p2).await.unwrap();

        // Add edge c1 -> c2
        r.add_edge(&c1.id, &c2.id, VersionEdgeType::EvolvedTo)
            .await
            .unwrap();

        // c2 should be a dependent of c1
        let dependents = r.get_dependents(&c1.id).await.unwrap();
        assert_eq!(dependents.len(), 1);
        assert_eq!(dependents[0].id, c2.id);
    }

    #[tokio::test]
    async fn version_graph_lineage() {
        use crate::core::versioning::version_graph::VersionGraph;

        let r = repo();
        let params = sample_params();
        let eid = params.entity_id.clone();
        r.create_automatic_commit(params).await.unwrap();

        let mut p2 = sample_params();
        p2.entity_id = eid.clone();
        r.create_automatic_commit(p2).await.unwrap();

        let lineage = r.get_lineage(&eid).await.unwrap();
        assert_eq!(lineage.len(), 2);
    }

    #[tokio::test]
    async fn version_graph_no_dependents() {
        use crate::core::versioning::version_graph::VersionGraph;

        let r = repo();
        let dependents = r.get_dependents("nonexistent").await.unwrap();
        assert!(dependents.is_empty());
    }
}
