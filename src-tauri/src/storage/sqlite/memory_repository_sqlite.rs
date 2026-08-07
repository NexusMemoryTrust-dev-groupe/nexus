use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use std::sync::Mutex;

use crate::core::entity_id::EntityId;
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::memory::types::{
    MemoryCaptureMode, MemoryFeedback, MemoryLayer, MemorySource, MemoryState, MemoryStatus,
    MemoryVisibility,
};
use crate::core::result::{AppError, Result};
use crate::storage::sqlite::schema;

/// SQLite-backed implementation of MemoryRepository.
/// Uses a Mutex<Connection> for thread-safe synchronous rusqlite calls inside async methods.
/// WAL mode is enabled for concurrent read performance.
pub struct SqliteMemoryRepository {
    conn: Mutex<Connection>,
}

impl SqliteMemoryRepository {
    /// Create a new repository from an existing connection.
    /// Applies migrations and enables WAL mode.
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

// ── Helpers ──

fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<MemoryRecord> {
    let id_str: String = row.get(0)?;
    let title: String = row.get(1)?;
    let summary: String = row.get(2)?;
    let content: String = row.get(3)?;
    let created_at: String = row.get(4)?;
    let updated_at: String = row.get(5)?;
    let author: String = row.get(6)?;
    let source_str: String = row.get(7)?;
    let confidence_score: f64 = row.get(8)?;
    let importance_score: f64 = row.get(9)?;
    let visibility_str: String = row.get(10)?;
    let capture_mode_str: String = row.get(11)?;
    let project_space_id: Option<String> = row.get(12)?;
    let linked_json: String = row.get(13)?;
    let latest_version_id: Option<String> = row.get(14)?;
    let status_str: String = row.get(15)?;
    let layer_str: String = row.get(16)?;
    let attached_files_json: String = row
        .get::<_, Option<String>>(17)?
        .unwrap_or_else(|| "[]".to_string());
    let derived_from_json: String = row
        .get::<_, Option<String>>(18)?
        .unwrap_or_else(|| "[]".to_string());
    let reason: Option<String> = row.get(19)?;
    let version: u32 = row.get::<_, i32>(20)? as u32;
    let updated_by: Option<String> = row.get(21)?;
    let memory_state: String = row
        .get::<_, Option<String>>(22)?
        .unwrap_or_else(|| "Current".to_string());
    let supersedes_id: Option<String> = row.get(23)?;
    let superseded_by_id: Option<String> = row.get(24)?;
    let confirmed_at: Option<String> = row.get(25)?;
    let confirmed_by: Option<String> = row.get(26)?;
    let expires_at: Option<String> = row.get(27)?;
    let feedback_json: String = row
        .get::<_, Option<String>>(28)?
        .unwrap_or_else(|| "{\"useful\":0,\"irrelevant\":0,\"wrong\":0}".to_string());

    let linked_entity_ids: Vec<EntityId> = serde_json::from_str(&linked_json).unwrap_or_default();
    let attached_files: Vec<crate::core::memory::memory_record::AttachedFile> =
        serde_json::from_str(&attached_files_json).unwrap_or_default();
    let derived_from: Vec<String> = serde_json::from_str(&derived_from_json).unwrap_or_default();

    Ok(MemoryRecord {
        id: EntityId::parse(&id_str)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        title,
        summary,
        content,
        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        author,
        source: parse_source(&source_str),
        confidence_score,
        importance_score,
        visibility: parse_visibility(&visibility_str),
        capture_mode: parse_capture_mode(&capture_mode_str),
        project_space_id: project_space_id
            .map(|s| EntityId::parse(&s))
            .transpose()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        linked_entity_ids,
        latest_version_id,
        status: parse_status(&status_str),
        layer: parse_layer(&layer_str),
        attached_files,
        derived_from,
        reason,
        version,
        updated_by,
        memory_state: MemoryState::parse(&memory_state),
        supersedes_id,
        superseded_by_id,
        confirmed_at: confirmed_at
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc)),
        confirmed_by,
        expires_at: expires_at
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc)),
        feedback: serde_json::from_str(&feedback_json).unwrap_or_default(),
    })
}

fn source_to_string(s: &MemorySource) -> String {
    match s {
        MemorySource::Manual => "Manual",
        MemorySource::Git => "Git",
        MemorySource::Telegram => "Telegram",
        MemorySource::Email => "Email",
        MemorySource::Meeting => "Meeting",
        MemorySource::Document => "Document",
        MemorySource::AiGenerated => "AiGenerated",
        MemorySource::Compressed => "Compressed",
    }
    .to_string()
}

fn parse_source(s: &str) -> MemorySource {
    match s {
        "Manual" => MemorySource::Manual,
        "Git" => MemorySource::Git,
        "Telegram" => MemorySource::Telegram,
        "Email" => MemorySource::Email,
        "Meeting" => MemorySource::Meeting,
        "Document" => MemorySource::Document,
        "AiGenerated" => MemorySource::AiGenerated,
        // Was missing, so `source_to_string` wrote "Compressed" but reading it
        // back silently downgraded the record to `Manual`.
        "Compressed" => MemorySource::Compressed,
        _ => MemorySource::Manual,
    }
}

fn visibility_to_string(v: &MemoryVisibility) -> String {
    match v {
        MemoryVisibility::Public => "Public",
        MemoryVisibility::Private => "Private",
        MemoryVisibility::Restricted => "Restricted",
    }
    .to_string()
}

fn parse_visibility(s: &str) -> MemoryVisibility {
    match s {
        "Public" => MemoryVisibility::Public,
        "Private" => MemoryVisibility::Private,
        "Restricted" => MemoryVisibility::Restricted,
        _ => MemoryVisibility::Private,
    }
}

fn capture_mode_to_string(m: &MemoryCaptureMode) -> String {
    match m {
        MemoryCaptureMode::Passive => "Passive",
        MemoryCaptureMode::Assisted => "Assisted",
        MemoryCaptureMode::Automatic => "Automatic",
    }
    .to_string()
}

fn parse_capture_mode(s: &str) -> MemoryCaptureMode {
    match s {
        "Passive" => MemoryCaptureMode::Passive,
        "Assisted" => MemoryCaptureMode::Assisted,
        "Automatic" => MemoryCaptureMode::Automatic,
        _ => MemoryCaptureMode::Passive,
    }
}

fn status_to_string(s: &MemoryStatus) -> String {
    match s {
        MemoryStatus::Active => "Active",
        MemoryStatus::Archived => "Archived",
        MemoryStatus::Merged => "Merged",
    }
    .to_string()
}

fn parse_status(s: &str) -> MemoryStatus {
    match s {
        "Active" => MemoryStatus::Active,
        "Archived" => MemoryStatus::Archived,
        "Merged" => MemoryStatus::Merged,
        _ => MemoryStatus::Active,
    }
}

fn layer_to_string(l: &MemoryLayer) -> String {
    match l {
        MemoryLayer::Raw => "Raw",
        MemoryLayer::Knowledge => "Knowledge",
        MemoryLayer::Decision => "Decision",
        MemoryLayer::Wisdom => "Wisdom",
    }
    .to_string()
}

fn parse_layer(s: &str) -> MemoryLayer {
    match s {
        "Raw" => MemoryLayer::Raw,
        "Knowledge" => MemoryLayer::Knowledge,
        "Decision" => MemoryLayer::Decision,
        "Wisdom" => MemoryLayer::Wisdom,
        _ => MemoryLayer::Raw,
    }
}

#[async_trait]
impl MemoryRepository for SqliteMemoryRepository {
    async fn save(&self, record: &MemoryRecord) -> Result<EntityId> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let id = record.id.as_str().to_string();
        let linked_json = serde_json::to_string(&record.linked_entity_ids)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let attached_json = serde_json::to_string(&record.attached_files)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let derived_from_json = serde_json::to_string(&record.derived_from)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let feedback_json = serde_json::to_string(&record.feedback)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        conn.execute(
            "INSERT INTO memory_records (
                id, title, summary, content, created_at, updated_at,
                author, source, confidence_score, importance_score,
                visibility, capture_mode, project_space_id,
                linked_entity_ids_json, latest_version_id, status, layer,
                attached_files_json, derived_from_json, reason, version, updated_by,
                memory_state, supersedes_id, superseded_by_id,
                confirmed_at, confirmed_by, expires_at, feedback_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29)",
            params![
                id,
                record.title,
                record.summary,
                record.content,
                record.created_at.to_rfc3339(),
                record.updated_at.to_rfc3339(),
                record.author,
                source_to_string(&record.source),
                record.confidence_score,
                record.importance_score,
                visibility_to_string(&record.visibility),
                capture_mode_to_string(&record.capture_mode),
                record.project_space_id.as_ref().map(|e| e.as_str()),
                linked_json,
                record.latest_version_id,
                status_to_string(&record.status),
                layer_to_string(&record.layer),
                attached_json,
                derived_from_json,
                record.reason,
                record.version as i32,
                record.updated_by,
                record.memory_state.as_str(),
                record.supersedes_id,
                record.superseded_by_id,
                record.confirmed_at.map(|dt| dt.to_rfc3339()),
                record.confirmed_by,
                record.expires_at.map(|dt| dt.to_rfc3339()),
                feedback_json,
            ],
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(record.id.clone())
    }

    async fn get_by_id(&self, id: &EntityId) -> Result<Option<MemoryRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, title, summary, content, created_at, updated_at,
                    author, source, confidence_score, importance_score,
                    visibility, capture_mode, project_space_id,
                    linked_entity_ids_json, latest_version_id, status, layer,
                    attached_files_json, derived_from_json, reason, version, updated_by,
                    memory_state, supersedes_id, superseded_by_id,
                    confirmed_at, confirmed_by, expires_at, feedback_json
                 FROM memory_records WHERE id = ?1",
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let result = stmt
            .query_row(params![id.as_str()], row_to_record)
            .optional()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(result)
    }

    async fn get_by_project(&self, project_id: &EntityId) -> Result<Vec<MemoryRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, title, summary, content, created_at, updated_at,
                    author, source, confidence_score, importance_score,
                    visibility, capture_mode, project_space_id,
                    linked_entity_ids_json, latest_version_id, status, layer,
                    attached_files_json, derived_from_json, reason, version, updated_by,
                    memory_state, supersedes_id, superseded_by_id,
                    confirmed_at, confirmed_by, expires_at, feedback_json
                 FROM memory_records WHERE project_space_id = ?1",
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let rows = stmt
            .query_map(params![project_id.as_str()], row_to_record)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row.map_err(|e| AppError::Internal(e.to_string()))?);
        }
        Ok(records)
    }

    async fn search(&self, query: &str) -> Result<Vec<MemoryRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        // Use FTS5 for full-text search with OR logic
        // Split query into words and join with OR for better matching
        let words: Vec<&str> = query.split_whitespace().collect();
        let fts_query = if words.is_empty() {
            "\"\"".to_string()
        } else if words.len() == 1 {
            // Single word - use prefix matching
            format!("\"{}\"*", words[0].replace('"', "\"\""))
        } else {
            // Multiple words - use OR to match any word
            words
                .iter()
                .map(|w| format!("\"{}\"", w.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(" OR ")
        };

        let mut stmt = conn
            .prepare(
                "SELECT mr.id, mr.title, mr.summary, mr.content, mr.created_at, mr.updated_at,
                    mr.author, mr.source, mr.confidence_score, mr.importance_score,
                    mr.visibility, mr.capture_mode, mr.project_space_id,
                    mr.linked_entity_ids_json, mr.latest_version_id, mr.status, mr.layer,
                    mr.attached_files_json, mr.derived_from_json, mr.reason, mr.version, mr.updated_by,
                    mr.memory_state, mr.supersedes_id, mr.superseded_by_id,
                    mr.confirmed_at, mr.confirmed_by, mr.expires_at, mr.feedback_json
                 FROM memory_fts fts
                 JOIN memory_records mr ON fts.rowid = mr.rowid
                 WHERE memory_fts MATCH ?1
                 ORDER BY rank",
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let rows = stmt
            .query_map(params![fts_query], row_to_record)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row.map_err(|e| AppError::Internal(e.to_string()))?);
        }
        Ok(records)
    }

    async fn update(&self, record: &MemoryRecord) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let linked_json = serde_json::to_string(&record.linked_entity_ids)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let attached_json = serde_json::to_string(&record.attached_files)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let derived_from_json = serde_json::to_string(&record.derived_from)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let feedback_json = serde_json::to_string(&record.feedback)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        // Persists the versioning columns too — without them `touch()` bumps
        // `version` in memory but the DB keeps the stale value forever.
        let rows = conn
            .execute(
                "UPDATE memory_records SET
                    title = ?2, summary = ?3, content = ?4, created_at = ?5, updated_at = ?6,
                    author = ?7, source = ?8, confidence_score = ?9, importance_score = ?10,
                    visibility = ?11, capture_mode = ?12, project_space_id = ?13,
                    linked_entity_ids_json = ?14, latest_version_id = ?15, status = ?16, layer = ?17,
                    attached_files_json = ?18, derived_from_json = ?19, reason = ?20,
                    version = ?21, updated_by = ?22,
                    memory_state = ?23, supersedes_id = ?24, superseded_by_id = ?25,
                    confirmed_at = ?26, confirmed_by = ?27, expires_at = ?28, feedback_json = ?29
                 WHERE id = ?1",
                params![
                    record.id.as_str(),
                    record.title,
                    record.summary,
                    record.content,
                    record.created_at.to_rfc3339(),
                    record.updated_at.to_rfc3339(),
                    record.author,
                    source_to_string(&record.source),
                    record.confidence_score,
                    record.importance_score,
                    visibility_to_string(&record.visibility),
                    capture_mode_to_string(&record.capture_mode),
                    record.project_space_id.as_ref().map(|e| e.as_str()),
                    linked_json,
                    record.latest_version_id,
                    status_to_string(&record.status),
                    layer_to_string(&record.layer),
                    attached_json,
                    derived_from_json,
                    record.reason,
                    record.version as i32,
                    record.updated_by,
                    record.memory_state.as_str(),
                    record.supersedes_id,
                    record.superseded_by_id,
                    record.confirmed_at.map(|dt| dt.to_rfc3339()),
                    record.confirmed_by,
                    record.expires_at.map(|dt| dt.to_rfc3339()),
                    feedback_json,
                ],
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;

        if rows == 0 {
            return Err(AppError::NotFound(format!(
                "Memory record {} not found",
                record.id
            )));
        }
        Ok(())
    }

    async fn delete(&self, id: &EntityId) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let rows = conn
            .execute(
                "DELETE FROM memory_records WHERE id = ?1",
                params![id.as_str()],
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;

        if rows == 0 {
            return Err(AppError::NotFound(format!(
                "Memory record {} not found",
                id
            )));
        }
        Ok(())
    }

    async fn list(&self, limit: u32, offset: u32) -> Result<Vec<MemoryRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, title, summary, content, created_at, updated_at,
                    author, source, confidence_score, importance_score,
                    visibility, capture_mode, project_space_id,
                    linked_entity_ids_json, latest_version_id, status, layer,
                    attached_files_json, derived_from_json, reason, version, updated_by,
                    memory_state, supersedes_id, superseded_by_id,
                    confirmed_at, confirmed_by, expires_at, feedback_json
                 FROM memory_records ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let rows = stmt
            .query_map(params![limit, offset], row_to_record)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row.map_err(|e| AppError::Internal(e.to_string()))?);
        }
        Ok(records)
    }

    async fn count(&self) -> Result<u64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM memory_records", [], |row| row.get(0))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(count as u64)
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::types::MemorySource;

    fn sample_record() -> MemoryRecord {
        MemoryRecord::new(
            "Test Title".to_string(),
            "Test content body".to_string(),
            "author-1".to_string(),
            MemorySource::Manual,
        )
        .unwrap()
    }

    fn repo() -> SqliteMemoryRepository {
        SqliteMemoryRepository::new_in_memory().unwrap()
    }

    #[tokio::test]
    async fn save_and_get() {
        let r = repo();
        let record = sample_record();
        let id = r.save(&record).await.unwrap();
        let fetched = r.get_by_id(&id).await.unwrap().unwrap();
        assert_eq!(fetched.title, "Test Title");
        assert_eq!(fetched.author, "author-1");
    }

    #[tokio::test]
    async fn get_nonexistent() {
        let r = repo();
        let result = r.get_by_id(&EntityId::new()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn update_record() {
        let r = repo();
        let mut record = sample_record();
        r.save(&record).await.unwrap();
        record.title = "Updated Title".to_string();
        r.update(&record).await.unwrap();
        let fetched = r.get_by_id(&record.id).await.unwrap().unwrap();
        assert_eq!(fetched.title, "Updated Title");
    }

    #[tokio::test]
    async fn update_nonexistent_fails() {
        let r = repo();
        let record = sample_record();
        let result = r.update(&record).await;
        assert!(result.is_err());
    }

    /// Regression: `update()` used to omit the versioning columns, so `touch()`
    /// bumped `version` in memory while the DB kept the stale value.
    #[tokio::test]
    async fn update_persists_versioning_columns() {
        let r = repo();
        let mut record = sample_record();
        r.save(&record).await.unwrap();
        assert_eq!(record.version, 1);

        record.touch();
        record.reason = Some("edited by user".to_string());
        record.updated_by = Some("tester".to_string());
        record.derived_from = vec!["origin-1".to_string()];
        r.update(&record).await.unwrap();

        let fetched = r.get_by_id(&record.id).await.unwrap().unwrap();
        assert_eq!(fetched.version, 2, "version must survive a round-trip");
        assert_eq!(fetched.reason.as_deref(), Some("edited by user"));
        assert_eq!(fetched.updated_by.as_deref(), Some("tester"));
        assert_eq!(fetched.derived_from, vec!["origin-1".to_string()]);
    }

    /// Regression: `Compressed` was written by `source_to_string` but missing
    /// from `parse_source`, so reading it back silently downgraded to `Manual`.
    #[tokio::test]
    async fn every_source_survives_a_round_trip() {
        let r = repo();
        let sources = [
            MemorySource::Manual,
            MemorySource::Git,
            MemorySource::Telegram,
            MemorySource::Email,
            MemorySource::Meeting,
            MemorySource::Document,
            MemorySource::AiGenerated,
            MemorySource::Compressed,
        ];

        for source in sources {
            let mut record = sample_record();
            record.source = source.clone();
            let id = r.save(&record).await.unwrap();
            let fetched = r.get_by_id(&id).await.unwrap().unwrap();
            assert_eq!(
                source_to_string(&fetched.source),
                source_to_string(&source),
                "source {:?} was mangled on read",
                source
            );
        }
    }

    /// Cyrillic content must round-trip byte-for-byte through SQLite.
    #[tokio::test]
    async fn cyrillic_content_round_trips() {
        let r = repo();
        let record = MemoryRecord::new(
            "Заголовок памяти".to_string(),
            "Содержимое с эмодзи 🚀 и юникодом".to_string(),
            "автор".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        let id = r.save(&record).await.unwrap();
        let fetched = r.get_by_id(&id).await.unwrap().unwrap();
        assert_eq!(fetched.title, "Заголовок памяти");
        assert_eq!(fetched.content, "Содержимое с эмодзи 🚀 и юникодом");
        assert_eq!(fetched.author, "автор");
    }

    #[tokio::test]
    async fn delete_record() {
        let r = repo();
        let record = sample_record();
        let id = r.save(&record).await.unwrap();
        r.delete(&id).await.unwrap();
        assert!(r.get_by_id(&id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_fails() {
        let r = repo();
        let result = r.delete(&EntityId::new()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn list_with_pagination() {
        let r = repo();
        for i in 0..10 {
            let mut rec = sample_record();
            rec.title = format!("Record {}", i);
            r.save(&rec).await.unwrap();
        }
        let page1 = r.list(3, 0).await.unwrap();
        assert_eq!(page1.len(), 3);
        let page2 = r.list(3, 7).await.unwrap();
        assert_eq!(page2.len(), 3);
    }

    #[tokio::test]
    async fn count_records() {
        let r = repo();
        assert_eq!(r.count().await.unwrap(), 0);
        r.save(&sample_record()).await.unwrap();
        r.save(&sample_record()).await.unwrap();
        assert_eq!(r.count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn get_by_project() {
        let r = repo();
        let mut record = sample_record();
        let project_id = EntityId::new();
        record.project_space_id = Some(project_id.clone());
        r.save(&record).await.unwrap();

        let other = sample_record();
        r.save(&other).await.unwrap();

        let results = r.get_by_project(&project_id).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn search_fts() {
        let r = repo();
        let mut rec = sample_record();
        rec.title = "Rust programming language".to_string();
        rec.content = "Rust is a systems programming language".to_string();
        r.save(&rec).await.unwrap();

        let other = sample_record();
        r.save(&other).await.unwrap();

        let results = r.search("Rust").await.unwrap();
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.title.contains("Rust")));
    }

    #[tokio::test]
    async fn roundtrip_all_fields() {
        let r = repo();
        let mut record = sample_record();
        record.summary = "Summary text".to_string();
        record.confidence_score = 0.8;
        record.importance_score = 0.9;
        record.visibility = MemoryVisibility::Public;
        record.capture_mode = MemoryCaptureMode::Assisted;
        record.status = MemoryStatus::Active;
        record.layer = MemoryLayer::Knowledge;
        r.save(&record).await.unwrap();

        let fetched = r.get_by_id(&record.id).await.unwrap().unwrap();
        assert_eq!(fetched.summary, "Summary text");
        assert!((fetched.confidence_score - 0.8).abs() < f64::EPSILON);
        assert!((fetched.importance_score - 0.9).abs() < f64::EPSILON);
        assert_eq!(fetched.visibility, MemoryVisibility::Public);
        assert_eq!(fetched.capture_mode, MemoryCaptureMode::Assisted);
        assert_eq!(fetched.status, MemoryStatus::Active);
        assert_eq!(fetched.layer, MemoryLayer::Knowledge);
    }
}
