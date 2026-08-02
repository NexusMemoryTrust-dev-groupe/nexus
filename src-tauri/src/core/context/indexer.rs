//! Background semantic indexing.
//!
//! Why this exists
//! ---------------
//! Embeddings were only ever written when something explicitly called the
//! `nexus_store_fingerprint` MCP tool. Nothing in the application did, so in a
//! normal install `memory_semantic_fingerprints` stayed empty and
//! `nexus_search_semantic` returned nothing at all — the feature was present in
//! the tool list but dead in practice.
//!
//! This module closes that gap from both ends:
//!
//! * [`backfill`] indexes everything that has no fingerprint yet, so existing
//!   databases become searchable without the user doing anything.
//! * [`index_memory`] / [`forget_memory`] keep the index in step as memories are
//!   created, edited and deleted.
//!
//! Design constraints that shaped it:
//!
//! * **Never block a write.** Indexing runs on its own thread; a failure to
//!   embed must never make "save my note" fail. Callers use the fire-and-forget
//!   [`spawn_index_memory`].
//! * **One indexer at a time.** Embedding is CPU-heavy, so a guard flag stops a
//!   second backfill from piling on top of a running one.
//! * **Bounded batches.** A large vault is processed in chunks with the
//!   connection released between them, so the UI keeps its own DB access.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rusqlite::Connection;

use crate::core::context::semantic_search::SemanticSearch;
use crate::core::entity_id::EntityId;
use crate::core::result::{AppError, Result};

/// Memories embedded per batch before the connection is released.
const BATCH_SIZE: u32 = 32;

/// Text handed to the embedder per memory: title, then summary, then content.
/// Truncated because embedding models have a fixed window and the leading text
/// carries the topic; `SemanticSearch::validate_text` truncates again at its own
/// limit, this just avoids reading megabytes into memory first.
const MAX_INDEX_TEXT: usize = 8192;

/// Set while a backfill is running, so concurrent triggers collapse into one.
fn backfill_running() -> &'static AtomicBool {
    static FLAG: AtomicBool = AtomicBool::new(false);
    &FLAG
}

/// Outcome of a backfill pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BackfillReport {
    /// Memories that had no fingerprint when the pass started.
    pub pending: u64,
    /// Memories successfully embedded.
    pub indexed: u64,
    /// Memories skipped because they had no usable text.
    pub skipped: u64,
    /// Memories that failed to embed.
    pub failed: u64,
}

impl BackfillReport {
    pub fn is_complete(&self) -> bool {
        self.indexed + self.skipped + self.failed >= self.pending
    }
}

/// Build the text we embed for a memory.
///
/// Title first so that the most identifying words survive truncation, then the
/// human summary, then the body.
pub fn index_text(title: &str, summary: &str, content: &str) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(3);
    for part in [title, summary, content] {
        let trimmed = part.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed);
        }
    }
    let joined = parts.join("\n\n");
    crate::core::text::truncate_chars(&joined, MAX_INDEX_TEXT).to_string()
}

/// Memory ids that have no fingerprint yet, oldest first.
///
/// `LEFT JOIN ... WHERE f.memory_id IS NULL` is the whole point: it asks the
/// database which rows are missing instead of loading every memory and every
/// fingerprint into memory to diff them.
fn unindexed_batch(conn: &Connection, limit: u32) -> Result<Vec<(String, String, String, String)>> {
    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.title, m.summary, m.content
             FROM memory_records m
             LEFT JOIN memory_semantic_fingerprints f ON f.memory_id = m.id
             WHERE f.memory_id IS NULL
             ORDER BY m.created_at ASC
             LIMIT ?1",
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

    let rows = stmt
        .query_map([limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1).unwrap_or_default(),
                row.get::<_, String>(2).unwrap_or_default(),
                row.get::<_, String>(3).unwrap_or_default(),
            ))
        })
        .map_err(|e| AppError::Database(e.to_string()))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| AppError::Database(e.to_string()))?);
    }
    Ok(out)
}

/// How many memories still lack a fingerprint.
pub fn pending_count(conn: &Connection) -> Result<u64> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM memory_records m
             LEFT JOIN memory_semantic_fingerprints f ON f.memory_id = m.id
             WHERE f.memory_id IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(count.max(0) as u64)
}

/// Index every memory that has no fingerprint.
///
/// Synchronous and bounded: intended to be called from [`spawn_backfill`] on a
/// worker thread, or directly from a test.
pub fn backfill(search: &SemanticSearch) -> Result<BackfillReport> {
    let mut report = BackfillReport::default();

    {
        let conn = crate::db::open_connection().map_err(AppError::Database)?;
        report.pending = pending_count(&conn)?;
    }

    if report.pending == 0 {
        return Ok(report);
    }

    tracing::info!(
        "Semantic index: {} memories pending, exact_model={}",
        report.pending,
        search.is_model_loaded()
    );

    loop {
        // Fresh connection per batch, dropped before embedding, so the indexer
        // never holds a DB handle while doing CPU work.
        let batch = {
            let conn = crate::db::open_connection().map_err(AppError::Database)?;
            unindexed_batch(&conn, BATCH_SIZE)?
        };

        if batch.is_empty() {
            break;
        }

        for (id, title, summary, content) in batch {
            let Ok(entity_id) = EntityId::parse(&id) else {
                // An unparseable id cannot be embedded and would otherwise be
                // re-selected forever, so count it as failed and move on.
                report.failed += 1;
                tracing::warn!("Semantic index: skipping unparseable memory id {id}");
                continue;
            };

            let text = index_text(&title, &summary, &content);
            if text.is_empty() {
                // Nothing to embed. Store an empty fingerprint would be wrong,
                // so record the skip; the row stays pending but we stop looping
                // on it via the guard below.
                report.skipped += 1;
                continue;
            }

            match search.store_fingerprint(&entity_id, &text) {
                Ok(()) => report.indexed += 1,
                Err(e) => {
                    report.failed += 1;
                    tracing::warn!("Semantic index: failed for {id}: {e}");
                }
            }
        }

        // Guard against a non-advancing loop: if a whole batch produced neither
        // an index nor a failure (all skipped), the same rows would be selected
        // again forever.
        if report.indexed + report.failed == 0 {
            break;
        }
        if report.indexed + report.skipped + report.failed >= report.pending {
            break;
        }
    }

    tracing::info!(
        "Semantic index: indexed={} skipped={} failed={}",
        report.indexed,
        report.skipped,
        report.failed
    );
    Ok(report)
}

/// Open a semantic search instance against the application database.
fn open_search() -> Result<SemanticSearch> {
    let conn = crate::db::open_connection().map_err(AppError::Database)?;
    SemanticSearch::new(conn)
}

/// Run [`backfill`] on a worker thread. Returns immediately.
///
/// Collapses to a no-op when a backfill is already in flight, so calling it on
/// every startup and after every bulk import stays safe.
pub fn spawn_backfill() {
    if backfill_running()
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        tracing::debug!("Semantic index: backfill already running, skipping");
        return;
    }

    std::thread::Builder::new()
        .name("nexus-semantic-backfill".into())
        .spawn(|| {
            let result = open_search().and_then(|s| backfill(&s));
            if let Err(e) = result {
                tracing::warn!("Semantic index: backfill aborted: {e}");
            }
            backfill_running().store(false, Ordering::SeqCst);
        })
        .map(|_| ())
        .unwrap_or_else(|e| {
            // Thread creation failed: clear the flag so a later attempt can run.
            backfill_running().store(false, Ordering::SeqCst);
            tracing::warn!("Semantic index: cannot spawn worker: {e}");
        });
}

/// Index (or re-index) a single memory. Synchronous.
pub fn index_memory(memory_id: &EntityId, title: &str, summary: &str, content: &str) -> Result<()> {
    let text = index_text(title, summary, content);
    if text.is_empty() {
        return Ok(());
    }
    open_search()?.store_fingerprint(memory_id, &text)
}

/// Index a single memory on a worker thread, ignoring failures.
///
/// Used on the write path: a memory must save even if embedding is unavailable.
pub fn spawn_index_memory(memory_id: &EntityId, title: &str, summary: &str, content: &str) {
    let id = memory_id.clone();
    let text = index_text(title, summary, content);
    if text.is_empty() {
        return;
    }

    let spawned = std::thread::Builder::new()
        .name("nexus-semantic-index".into())
        .spawn(move || match open_search() {
            Ok(search) => {
                if let Err(e) = search.store_fingerprint(&id, &text) {
                    tracing::warn!("Semantic index: store failed for {}: {e}", id.as_str());
                }
            }
            Err(e) => tracing::warn!("Semantic index: unavailable ({e})"),
        });

    if let Err(e) = spawned {
        tracing::warn!("Semantic index: cannot spawn indexer: {e}");
    }
}

/// Drop a memory's fingerprint. Synchronous; safe when none exists.
pub fn forget_memory(memory_id: &EntityId) -> Result<()> {
    open_search()?.delete_fingerprint(memory_id)
}

/// Drop a memory's fingerprint on a worker thread, ignoring failures.
pub fn spawn_forget_memory(memory_id: &EntityId) {
    let id = memory_id.clone();
    let spawned = std::thread::Builder::new()
        .name("nexus-semantic-forget".into())
        .spawn(move || {
            if let Err(e) = forget_memory(&id) {
                tracing::warn!("Semantic index: delete failed for {}: {e}", id.as_str());
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("Semantic index: cannot spawn remover: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── index_text ──

    #[test]
    fn index_text_joins_all_three_fields() {
        let t = index_text("Title", "Summary", "Content");
        assert!(t.contains("Title"));
        assert!(t.contains("Summary"));
        assert!(t.contains("Content"));
    }

    #[test]
    fn index_text_puts_title_first() {
        // Truncation must never cost us the title, so it has to lead.
        let t = index_text("Unique-Title", "s", "c");
        assert!(t.starts_with("Unique-Title"));
    }

    #[test]
    fn index_text_skips_blank_fields() {
        let t = index_text("Title", "   ", "");
        assert_eq!(t, "Title");
    }

    #[test]
    fn index_text_is_empty_when_nothing_usable() {
        assert!(index_text("", "", "").is_empty());
        assert!(index_text("  ", "\n", "\t").is_empty());
    }

    #[test]
    fn index_text_truncates_without_splitting_utf8() {
        // Cyrillic is two bytes per char: a byte-indexed cut would panic.
        let long = "Пользователь ".repeat(2000);
        let t = index_text("Заголовок", "", &long);
        assert!(t.len() <= MAX_INDEX_TEXT);
        assert!(std::str::from_utf8(t.as_bytes()).is_ok());
    }

    // ── pending / unindexed queries ──

    /// In-memory database with `count` memories and no fingerprints.
    ///
    /// Rows are inserted with plain SQL rather than through the repository so
    /// the test exercises only the indexer's own queries and stays synchronous.
    fn seeded_db(count: usize) -> (Connection, Vec<EntityId>) {
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::sqlite::schema::apply_migrations(&conn).unwrap();

        let now = "2026-01-01T00:00:00+00:00";
        let ids: Vec<EntityId> = (0..count)
            .map(|i| {
                let id = EntityId::new();
                conn.execute(
                    "INSERT INTO memory_records
                       (id, title, summary, content, created_at, updated_at, author, source)
                     VALUES (?1, ?2, '', ?3, ?4, ?4, 'test', 'Manual')",
                    rusqlite::params![
                        id.as_str(),
                        format!("Memory {i}"),
                        format!("Body of memory number {i}"),
                        now,
                    ],
                )
                .unwrap();
                id
            })
            .collect();

        (conn, ids)
    }

    #[test]
    fn pending_count_sees_every_unindexed_memory() {
        let (conn, _ids) = seeded_db(5);
        assert_eq!(pending_count(&conn).unwrap(), 5);
    }

    #[test]
    fn pending_count_is_zero_on_empty_database() {
        let (conn, _ids) = seeded_db(0);
        assert_eq!(pending_count(&conn).unwrap(), 0);
    }

    #[test]
    fn unindexed_batch_respects_the_limit() {
        let (conn, _ids) = seeded_db(10);
        let batch = unindexed_batch(&conn, 4).unwrap();
        assert_eq!(batch.len(), 4);
    }

    #[test]
    fn unindexed_batch_excludes_already_indexed_rows() {
        let (conn, ids) = seeded_db(3);

        // Mark one as indexed.
        conn.execute(
            "INSERT INTO memory_semantic_fingerprints (memory_id, keywords_json, created_at)
             VALUES (?1, '[]', '2026-01-01T00:00:00Z')",
            [ids[0].as_str()],
        )
        .unwrap();

        let batch = unindexed_batch(&conn, 10).unwrap();
        assert_eq!(batch.len(), 2);
        assert!(
            !batch.iter().any(|(id, ..)| id == ids[0].as_str()),
            "indexed memory must not be selected again"
        );
        assert_eq!(pending_count(&conn).unwrap(), 2);
    }

    #[test]
    fn unindexed_batch_returns_the_text_fields() {
        let (conn, _ids) = seeded_db(1);
        let batch = unindexed_batch(&conn, 1).unwrap();
        let (_, title, _summary, content) = &batch[0];
        assert_eq!(title, "Memory 0");
        assert!(content.contains("Body of memory"));
    }

    // ── report ──

    #[test]
    fn report_completes_when_all_rows_accounted_for() {
        let r = BackfillReport { pending: 3, indexed: 2, skipped: 1, failed: 0 };
        assert!(r.is_complete());
    }

    #[test]
    fn report_incomplete_while_rows_remain() {
        let r = BackfillReport { pending: 3, indexed: 1, skipped: 0, failed: 0 };
        assert!(!r.is_complete());
    }

    #[test]
    fn empty_report_is_complete() {
        assert!(BackfillReport::default().is_complete());
    }
}
