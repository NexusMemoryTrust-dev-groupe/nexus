use rusqlite::{Connection, OptionalExtension, params};
use std::sync::Mutex;

use crate::core::context::context_lab::{LabExperiment, LabResult};
use crate::core::result::Result;
use crate::storage::sqlite::schema;

/// SQLite-backed repository for Context Lab experiments (System 6).
///
/// Сохраняет каждый прогон лаборатории: вопрос, JSON-срез всех стратегий и
/// победителя. История позволяет Nexus учиться: какая стратегия на каких
/// вопросах выигрывает чаще всего.
pub struct SqliteContextLabRepository {
    conn: Mutex<Connection>,
}

impl SqliteContextLabRepository {
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

    /// Сохранить эксперимент. Возвращает id записи.
    pub fn save_experiment(&self, exp: &LabExperiment) -> Result<String> {
        let id = format!(
            "lab_{}_{}",
            chrono::Utc::now().timestamp_millis(),
            exp.query.len().min(4)
        );
        let results_json = serde_json::to_string(&exp.results)?;
        let best = exp
            .best()
            .map(|r| r.strategy.name.clone())
            .unwrap_or_default();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO context_lab_runs (id, query, results_json, best_strategy, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, exp.query, results_json, best, exp.created_at,],
        )?;
        Ok(id)
    }

    /// Последние N экспериментов (свежие первыми).
    pub fn recent_experiments(&self, limit: usize) -> Result<Vec<LabExperiment>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, query, results_json, best_strategy, created_at
             FROM context_lab_runs
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            let id: String = row.get(0)?;
            let query: String = row.get(1)?;
            let results_json: String = row.get(2)?;
            let created_at: String = row.get(4)?;
            let results: Vec<LabResult> = serde_json::from_str(&results_json).unwrap_or_default();
            Ok((id, query, results, created_at))
        })?;
        let mut out = Vec::new();
        for r in rows {
            let (id, query, results, created_at) = r?;
            out.push(LabExperiment {
                query,
                created_at,
                results,
            });
            let _ = id;
        }
        Ok(out)
    }

    /// Сколько экспериментов проведено всего.
    pub fn count(&self) -> Result<u32> {
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM context_lab_runs", [], |r| r.get(0))?;
        Ok(n as u32)
    }

    /// Какая стратегия побеждает чаще всего (для обучения выбору стратегии).
    pub fn best_strategy_overall(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let best: Option<String> = conn
            .query_row(
                "SELECT best_strategy
                 FROM context_lab_runs
                 WHERE best_strategy != ''
                 GROUP BY best_strategy
                 ORDER BY COUNT(*) DESC
                 LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(best)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::context_lab::{ContextStrategy, LabMetrics, LabResult};

    fn sample_result(name: &str, acc: f64) -> LabResult {
        LabResult {
            query: "auth".to_string(),
            strategy: ContextStrategy {
                name: name.to_string(),
                max_tokens: 100,
                max_entities: 1,
                max_depth: 1,
                min_relevance: 0.3,
            },
            metrics: LabMetrics {
                memories: 3,
                entities: 2,
                tokens: 500,
                baseline_tokens: 1000,
                avg_relevance: 0.7,
                maturity: 0.5,
                accuracy: acc,
                build_ms: 3,
            },
            package_id: "p".to_string(),
        }
    }

    #[test]
    fn save_and_recent_roundtrip() {
        let repo = SqliteContextLabRepository::new_in_memory().unwrap();
        let exp = LabExperiment {
            query: "how does auth work".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            results: vec![sample_result("compact", 0.9), sample_result("rich", 0.7)],
        };
        let id = repo.save_experiment(&exp).unwrap();
        assert!(!id.is_empty());
        let recent = repo.recent_experiments(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].query, exp.query);
        assert_eq!(recent[0].results.len(), 2);
        assert_eq!(repo.count().unwrap(), 1);
        assert_eq!(
            repo.best_strategy_overall().unwrap().as_deref(),
            Some("compact")
        );
    }

    #[test]
    fn empty_history_returns_empty() {
        let repo = SqliteContextLabRepository::new_in_memory().unwrap();
        assert_eq!(repo.recent_experiments(5).unwrap().len(), 0);
        assert_eq!(repo.count().unwrap(), 0);
        assert!(repo.best_strategy_overall().unwrap().is_none());
    }

    #[test]
    fn ordering_is_newest_first() {
        let repo = SqliteContextLabRepository::new_in_memory().unwrap();
        let mut exp = LabExperiment {
            query: "q1".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            results: vec![sample_result("compact", 0.8)],
        };
        repo.save_experiment(&exp).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        exp.query = "q2".to_string();
        exp.created_at = chrono::Utc::now().to_rfc3339();
        repo.save_experiment(&exp).unwrap();
        let recent = repo.recent_experiments(10).unwrap();
        assert_eq!(recent[0].query, "q2");
        assert_eq!(recent[1].query, "q1");
    }
}
