use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::core::flight::context_chain::ContextChain;
use crate::core::flight::flight_recorder::{
    FlightCategory, FlightOutcome, FlightRecord, FlightRepository, FlightSession,
    FlightSessionStatus, FlightStats,
};
use crate::core::result::Result;
use crate::storage::sqlite::schema;

/// SQLite-backed implementation of the Flight Recorder repository
/// (sessions + records, System 5). Same Mutex<Connection> pattern as the
/// other repositories.
pub struct SqliteFlightRepository {
    conn: Mutex<Connection>,
}

impl SqliteFlightRepository {
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

fn row_to_session(row: &rusqlite::Row) -> rusqlite::Result<FlightSession> {
    let id: String = row.get(0)?;
    let title: String = row.get(1)?;
    let purpose: String = row.get(2)?;
    let actor: String = row.get(3)?;
    let source: String = row.get(4)?;
    let status: String = row.get(5)?;
    let started_at: String = row.get(6)?;
    let ended_at: Option<String> = row.get(7)?;
    Ok(FlightSession {
        id,
        title,
        purpose,
        actor,
        source,
        status: FlightSessionStatus::parse(&status),
        started_at: chrono::DateTime::parse_from_rfc3339(&started_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now()),
        ended_at: ended_at.and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|dt| dt.with_timezone(&chrono::Utc))
        }),
    })
}

fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<FlightRecord> {
    let id: String = row.get(0)?;
    let session_id: Option<String> = row.get(1)?;
    let recorded_at: String = row.get(2)?;
    let actor: String = row.get(3)?;
    let category: String = row.get(4)?;
    let action: String = row.get(5)?;
    let entity_type: String = row.get(6)?;
    let entity_id: String = row.get(7)?;
    let summary: String = row.get(8)?;
    let details_json: String = row.get(9)?;
    let duration_ms: i64 = row.get(10)?;
    let outcome: String = row.get(11)?;

    let details: serde_json::Value = serde_json::from_str(&details_json).unwrap_or_default();

    Ok(FlightRecord {
        id,
        session_id,
        recorded_at: chrono::DateTime::parse_from_rfc3339(&recorded_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now()),
        actor,
        category: FlightCategory::parse(&category),
        action,
        entity_type,
        entity_id,
        summary,
        details,
        duration_ms,
        outcome: FlightOutcome::parse(&outcome),
    })
}

#[async_trait]
impl FlightRepository for SqliteFlightRepository {
    async fn create_session(&self, session: &FlightSession) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO flight_sessions (id, title, purpose, actor, source, status, started_at, ended_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                session.id,
                session.title,
                session.purpose,
                session.actor,
                session.source,
                session.status.as_str(),
                session.started_at.to_rfc3339(),
                session
                    .ended_at
                    .map(|dt| dt.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    async fn close_session(&self, session_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE flight_sessions SET status = 'closed', ended_at = ?2 WHERE id = ?1",
            params![session_id, chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    async fn list_active_sessions(&self, limit: u32) -> Result<Vec<FlightSession>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, purpose, actor, source, status, started_at, ended_at
             FROM flight_sessions
             WHERE status = 'active'
             ORDER BY started_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], row_to_session)?;
        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }
        Ok(sessions)
    }

    async fn add_record(&self, record: &FlightRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO flight_records
                (id, session_id, recorded_at, actor, category, action, entity_type, entity_id,
                 summary, details_json, duration_ms, outcome)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                record.id,
                record.session_id,
                record.recorded_at.to_rfc3339(),
                record.actor,
                record.category.as_str(),
                record.action,
                record.entity_type,
                record.entity_id,
                record.summary,
                serde_json::to_string(&record.details).unwrap_or_else(|_| "{}".to_string()),
                record.duration_ms,
                record.outcome.as_str(),
            ],
        )?;
        Ok(())
    }

    async fn recent_records(
        &self,
        limit: u32,
        category: Option<&str>,
    ) -> Result<Vec<FlightRecord>> {
        let conn = self.conn.lock().unwrap();
        const SELECT: &str =
            "SELECT id, session_id, recorded_at, actor, category, action, entity_type,
                              entity_id, summary, details_json, duration_ms, outcome
                              FROM flight_records";

        let mut records = Vec::new();
        match category {
            Some(cat) => {
                let mut stmt = conn.prepare(&format!(
                    "{} WHERE category = ?1 ORDER BY recorded_at DESC LIMIT ?2",
                    SELECT
                ))?;
                let rows = stmt.query_map(params![cat, limit], row_to_record)?;
                for row in rows {
                    records.push(row?);
                }
            }
            None => {
                let mut stmt =
                    conn.prepare(&format!("{} ORDER BY recorded_at DESC LIMIT ?1", SELECT))?;
                let rows = stmt.query_map(params![limit], row_to_record)?;
                for row in rows {
                    records.push(row?);
                }
            }
        }
        Ok(records)
    }

    async fn session_records(&self, session_id: &str) -> Result<Vec<FlightRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, recorded_at, actor, category, action, entity_type,
                    entity_id, summary, details_json, duration_ms, outcome
             FROM flight_records
             WHERE session_id = ?1
             ORDER BY recorded_at ASC",
        )?;
        let rows = stmt.query_map(params![session_id], row_to_record)?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    async fn entity_replay(&self, entity_type: &str, entity_id: &str) -> Result<Vec<FlightRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, recorded_at, actor, category, action, entity_type,
                    entity_id, summary, details_json, duration_ms, outcome
             FROM flight_records
             WHERE entity_type = ?1 AND entity_id = ?2
             ORDER BY recorded_at ASC",
        )?;
        let rows = stmt.query_map(params![entity_type, entity_id], row_to_record)?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    async fn stats(&self) -> Result<FlightStats> {
        let conn = self.conn.lock().unwrap();

        let total_records: i64 =
            conn.query_row("SELECT COUNT(*) FROM flight_records", [], |r| r.get(0))?;
        let total_sessions: i64 =
            conn.query_row("SELECT COUNT(*) FROM flight_sessions", [], |r| r.get(0))?;
        let active_sessions: i64 = conn.query_row(
            "SELECT COUNT(*) FROM flight_sessions WHERE status = 'active'",
            [],
            |r| r.get(0),
        )?;

        let mut by_category = BTreeMap::new();
        {
            let mut stmt =
                conn.prepare("SELECT category, COUNT(*) FROM flight_records GROUP BY category")?;
            let mut rows = stmt.query([])?;
            while let Ok(Some(row)) = rows.next() {
                let cat: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                by_category.insert(cat, count as u64);
            }
        }

        let mut by_outcome = BTreeMap::new();
        {
            let mut stmt =
                conn.prepare("SELECT outcome, COUNT(*) FROM flight_records GROUP BY outcome")?;
            let mut rows = stmt.query([])?;
            while let Ok(Some(row)) = rows.next() {
                let outcome: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                by_outcome.insert(outcome, count as u64);
            }
        }

        Ok(FlightStats {
            total_records: total_records as u64,
            total_sessions: total_sessions as u64,
            active_sessions: active_sessions as u64,
            by_category,
            by_outcome,
        })
    }
}

// ── Context chain recording (System 5: «почему ИИ так сказал») ─────

impl SqliteFlightRepository {
    /// Save a context chain (insert or update by id).
    pub fn save_context_chain(&self, chain: &ContextChain) -> Result<()> {
        let seeds_json = serde_json::to_string(&chain.seeds)?;
        let stages_json = serde_json::to_string(&chain.stages)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO context_chains
                (id, session_id, actor, query, intent, answer_confidence, answer,
                 seeds_json, stages_json, total_tokens, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
                session_id=excluded.session_id,
                actor=excluded.actor,
                query=excluded.query,
                intent=excluded.intent,
                answer_confidence=excluded.answer_confidence,
                answer=excluded.answer,
                seeds_json=excluded.seeds_json,
                stages_json=excluded.stages_json,
                total_tokens=excluded.total_tokens",
            params![
                chain.id,
                chain.session_id,
                chain.actor,
                chain.query,
                chain.intent,
                chain.answer_confidence,
                chain.answer,
                seeds_json,
                stages_json,
                chain.total_tokens as i64,
                chain.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// One context chain by id (None if missing).
    pub fn get_context_chain(&self, id: &str) -> Result<Option<ContextChain>> {
        let conn = self.conn.lock().unwrap();
        let chain = conn
            .query_row(
                "SELECT id, session_id, actor, query, intent, answer_confidence, answer,
                        seeds_json, stages_json, total_tokens, created_at
                 FROM context_chains WHERE id = ?1",
                params![id],
                row_to_chain,
            )
            .optional()?;
        Ok(chain)
    }

    /// Recent context chains, newest first (bounded).
    pub fn recent_context_chains(&self, limit: u32) -> Result<Vec<ContextChain>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, actor, query, intent, answer_confidence, answer,
                    seeds_json, stages_json, total_tokens, created_at
             FROM context_chains
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], row_to_chain)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// How many context chains were recorded in total.
    pub fn count_context_chains(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM context_chains", [], |row| row.get(0))?;
        Ok(n as u64)
    }
}

fn row_to_chain(row: &rusqlite::Row) -> rusqlite::Result<ContextChain> {
    let seeds_json: String = row.get(7)?;
    let stages_json: String = row.get(8)?;
    let created_at: String = row.get(10)?;
    Ok(ContextChain {
        id: row.get(0)?,
        session_id: row.get(1)?,
        actor: row.get(2)?,
        query: row.get(3)?,
        intent: row.get(4)?,
        answer_confidence: row.get(5)?,
        answer: row.get(6)?,
        seeds: serde_json::from_str(&seeds_json).unwrap_or_default(),
        stages: serde_json::from_str(&stages_json).unwrap_or_default(),
        total_tokens: row.get(9)?,
        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::flight::flight_recorder::{
        FlightCategory, FlightOutcome, FlightRecord, FlightSession,
    };

    #[tokio::test]
    async fn session_lifecycle_create_and_close() {
        let repo = SqliteFlightRepository::new_in_memory().unwrap();
        let session = FlightSession::new("Test", "prove lifecycle", "user", "ui");
        repo.create_session(&session).await.unwrap();

        let active = repo.list_active_sessions(10).await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, session.id);
        assert_eq!(active[0].status, FlightSessionStatus::Active);

        repo.close_session(&session.id).await.unwrap();
        let active_after = repo.list_active_sessions(10).await.unwrap();
        assert!(active_after.is_empty());
    }

    #[tokio::test]
    async fn record_roundtrip_and_recent() {
        let repo = SqliteFlightRepository::new_in_memory().unwrap();
        let record = FlightRecord::success(
            None,
            "agent",
            FlightCategory::Rehearsal,
            "run_cycle",
            "MemoryRecord",
            "",
            "Rehearsal cycle ran",
            serde_json::json!({"strengthened": 3}),
            5,
        );
        repo.add_record(&record).await.unwrap();

        let recent = repo.recent_records(10, None).await.unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, record.id);
        assert_eq!(recent[0].category, FlightCategory::Rehearsal);
        assert_eq!(recent[0].outcome, FlightOutcome::Success);
        assert_eq!(recent[0].details["strengthened"], 3);
    }

    #[tokio::test]
    async fn recent_filters_by_category() {
        let repo = SqliteFlightRepository::new_in_memory().unwrap();
        repo.add_record(&FlightRecord::success(
            None,
            "agent",
            FlightCategory::Memory,
            "create_memory",
            "MemoryRecord",
            "mem-1",
            "created",
            serde_json::json!({}),
            1,
        ))
        .await
        .unwrap();
        repo.add_record(&FlightRecord::success(
            None,
            "agent",
            FlightCategory::Firewall,
            "quarantine",
            "MemoryRecord",
            "mem-2",
            "quarantined",
            serde_json::json!({}),
            2,
        ))
        .await
        .unwrap();

        let firewall_only = repo.recent_records(10, Some("firewall")).await.unwrap();
        assert_eq!(firewall_only.len(), 1);
        assert_eq!(firewall_only[0].action, "quarantine");

        let memory_only = repo.recent_records(10, Some("memory")).await.unwrap();
        assert_eq!(memory_only.len(), 1);
        assert_eq!(memory_only[0].action, "create_memory");
    }

    #[tokio::test]
    async fn session_records_chronological() {
        let repo = SqliteFlightRepository::new_in_memory().unwrap();
        let session = FlightSession::new("MCP run", "tool calls", "agent", "mcp");
        repo.create_session(&session).await.unwrap();

        repo.add_record(&FlightRecord::success(
            Some(session.id.clone()),
            "agent",
            FlightCategory::Mcp,
            "call_tool",
            "Tool",
            "tool-a",
            "first",
            serde_json::json!({}),
            1,
        ))
        .await
        .unwrap();
        repo.add_record(&FlightRecord::success(
            Some(session.id.clone()),
            "agent",
            FlightCategory::Mcp,
            "call_tool",
            "Tool",
            "tool-b",
            "second",
            serde_json::json!({}),
            1,
        ))
        .await
        .unwrap();

        let records = repo.session_records(&session.id).await.unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].entity_id, "tool-a");
        assert_eq!(records[1].entity_id, "tool-b");
    }

    #[tokio::test]
    async fn entity_replay_builds_chain() {
        let repo = SqliteFlightRepository::new_in_memory().unwrap();
        repo.add_record(&FlightRecord::success(
            None,
            "user",
            FlightCategory::Firewall,
            "quarantine",
            "MemoryRecord",
            "mem-42",
            "quarantined by pii heuristic",
            serde_json::json!({}),
            3,
        ))
        .await
        .unwrap();
        repo.add_record(&FlightRecord::success(
            None,
            "user",
            FlightCategory::Memory,
            "approve_quarantine",
            "MemoryRecord",
            "mem-42",
            "approved from quarantine",
            serde_json::json!({}),
            4,
        ))
        .await
        .unwrap();
        repo.add_record(&FlightRecord::success(
            None,
            "user",
            FlightCategory::Memory,
            "create_memory",
            "MemoryRecord",
            "other",
            "unrelated",
            serde_json::json!({}),
            1,
        ))
        .await
        .unwrap();

        let chain = repo.entity_replay("MemoryRecord", "mem-42").await.unwrap();
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].action, "quarantine");
        assert_eq!(chain[1].action, "approve_quarantine");
    }

    #[tokio::test]
    async fn stats_aggregate_counts() {
        let repo = SqliteFlightRepository::new_in_memory().unwrap();
        repo.create_session(&FlightSession::new("S1", "p", "user", "ui"))
            .await
            .unwrap();

        repo.add_record(&FlightRecord::success(
            None,
            "agent",
            FlightCategory::Memory,
            "create_memory",
            "MemoryRecord",
            "",
            "a",
            serde_json::json!({}),
            1,
        ))
        .await
        .unwrap();
        repo.add_record(&FlightRecord::success(
            None,
            "agent",
            FlightCategory::Memory,
            "update_memory",
            "MemoryRecord",
            "",
            "b",
            serde_json::json!({}),
            1,
        ))
        .await
        .unwrap();
        repo.add_record(&FlightRecord::new(
            None,
            "agent",
            FlightCategory::Firewall,
            "block",
            "MemoryRecord",
            "",
            "c",
            serde_json::json!({}),
            1,
            FlightOutcome::Blocked,
        ))
        .await
        .unwrap();

        let stats = repo.stats().await.unwrap();
        assert_eq!(stats.total_records, 3);
        assert_eq!(stats.total_sessions, 1);
        assert_eq!(stats.active_sessions, 1);
        assert_eq!(stats.by_category.get("memory"), Some(&2));
        assert_eq!(stats.by_category.get("firewall"), Some(&1));
        assert_eq!(stats.by_outcome.get("success"), Some(&2));
        assert_eq!(stats.by_outcome.get("blocked"), Some(&1));
    }

    #[tokio::test]
    async fn close_session_marks_ended_at() {
        let repo = SqliteFlightRepository::new_in_memory().unwrap();
        let session = FlightSession::new("S", "p", "user", "cli");
        repo.create_session(&session).await.unwrap();
        repo.close_session(&session.id).await.unwrap();

        let conn = repo.conn.lock().unwrap();
        let ended: Option<String> = conn
            .query_row(
                "SELECT ended_at FROM flight_sessions WHERE id = ?1",
                params![session.id],
                |r| r.get(0),
            )
            .optional()
            .unwrap()
            .unwrap();
        assert!(ended.is_some(), "ended_at must be set on close");
    }

    #[tokio::test]
    async fn context_chain_round_trip() {
        let repo = SqliteFlightRepository::new_in_memory().unwrap();
        let mut chain = crate::core::flight::context_chain::ContextChain::begin(
            "How does auth work?",
            "explain_architecture",
            "user",
        );
        chain.add_seed(
            crate::core::flight::context_chain::ContextKind::Architecture,
            "mem-1",
            "Auth service design",
            0.9,
            500,
        );
        chain.pass_stage(
            crate::core::flight::context_chain::ChainStage::MemorySeeds,
            5,
            "seeds selected",
        );
        chain.finish("JWT access tokens.", 0.87);
        repo.save_context_chain(&chain).unwrap();

        let loaded = repo.get_context_chain(&chain.id).unwrap().unwrap();
        assert_eq!(loaded.query, "How does auth work?");
        assert_eq!(loaded.intent, "explain_architecture");
        assert_eq!(loaded.answer_confidence, 0.87);
        assert_eq!(loaded.seeds.len(), 1);
        assert_eq!(
            loaded.seeds[0].kind,
            crate::core::flight::context_chain::ContextKind::Architecture
        );
        assert_eq!(loaded.seeds[0].tokens, 500);
        assert_eq!(loaded.total_tokens, 500);
        assert_eq!(loaded.stages.len(), 3, "request + memory_seeds + answer");
        assert_eq!(repo.count_context_chains().unwrap(), 1);
    }

    #[tokio::test]
    async fn context_chain_recent_orders_newest_first() {
        let repo = SqliteFlightRepository::new_in_memory().unwrap();
        let mut c1 = crate::core::flight::context_chain::ContextChain::begin("q1", "i", "a");
        c1.id = "chain-1".to_string();
        let mut c2 = crate::core::flight::context_chain::ContextChain::begin("q2", "i", "a");
        c2.id = "chain-2".to_string();
        repo.save_context_chain(&c1).unwrap();
        repo.save_context_chain(&c2).unwrap();

        let recent = repo.recent_context_chains(10).unwrap();
        assert_eq!(recent.len(), 2);
        // Новейшая цепочка первой (создана позже).
        assert_eq!(recent[0].id, "chain-2");
        assert_eq!(repo.count_context_chains().unwrap(), 2);
    }

    #[tokio::test]
    async fn context_chain_missing_returns_none() {
        let repo = SqliteFlightRepository::new_in_memory().unwrap();
        assert!(repo.get_context_chain("missing").unwrap().is_none());
    }
}
