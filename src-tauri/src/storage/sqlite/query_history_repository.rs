use rusqlite::{Connection, params};
use std::sync::Mutex;

use crate::core::context::predictive::QueryLogEntry;
use crate::core::result::Result;
use crate::storage::sqlite::schema;

/// SQLite-backed repository for the query history (Predictive Context, System 8).
///
/// Хранит каждый запрос с интентом и сущностями; отдаёт последние N записей
/// для построения марковских переходов.
pub struct SqliteQueryHistoryRepository {
    conn: Mutex<Connection>,
}

impl SqliteQueryHistoryRepository {
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

    /// Сохранить запрос в историю.
    pub fn log_query(&self, query: &str, intent_type: &str, entities: &[String]) -> Result<()> {
        let id = crate::core::entity_id::EntityId::new().to_string();
        let entities_json = serde_json::to_string(entities)?;
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO query_history (id, query, intent_type, entities_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, query, intent_type, entities_json, now],
        )?;
        Ok(())
    }

    /// Последние N запросов (старые первыми — для порядка переходов).
    pub fn recent(&self, limit: usize) -> Result<Vec<QueryLogEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT query, intent_type, entities_json, created_at
             FROM query_history
             ORDER BY created_at ASC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            let query: String = row.get(0)?;
            let intent_type: String = row.get(1)?;
            let entities_json: String = row.get(2)?;
            let created_at: String = row.get(3)?;
            let entities: Vec<String> = serde_json::from_str(&entities_json).unwrap_or_default();
            Ok(QueryLogEntry {
                query,
                intent_type,
                entities,
                created_at,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Сколько запросов в истории всего.
    pub fn count(&self) -> Result<u32> {
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM query_history", [], |r| r.get(0))?;
        Ok(n as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_and_recent_roundtrip() {
        let repo = SqliteQueryHistoryRepository::new_in_memory().unwrap();
        repo.log_query("how does auth work", "explain", &["e-auth".to_string()])
            .unwrap();
        let recent = repo.recent(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].query, "how does auth work");
        assert_eq!(recent[0].intent_type, "explain");
        assert_eq!(recent[0].entities, vec!["e-auth".to_string()]);
        assert_eq!(repo.count().unwrap(), 1);
    }

    #[test]
    fn recent_is_oldest_first() {
        let repo = SqliteQueryHistoryRepository::new_in_memory().unwrap();
        repo.log_query("first", "explain", &[]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        repo.log_query("second", "explore", &[]).unwrap();
        let recent = repo.recent(10).unwrap();
        assert_eq!(recent[0].query, "first");
        assert_eq!(recent[1].query, "second");
    }

    #[test]
    fn empty_history() {
        let repo = SqliteQueryHistoryRepository::new_in_memory().unwrap();
        assert!(repo.recent(5).unwrap().is_empty());
        assert_eq!(repo.count().unwrap(), 0);
    }
}
