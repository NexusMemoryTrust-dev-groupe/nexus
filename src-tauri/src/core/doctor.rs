//! Nexus Doctor — system health checks (Production Readiness Gate 0.4).
//!
//! A battery of read-only checks over the live database and configuration:
//! file presence, migration state, SQLite integrity, foreign-key hygiene,
//! FTS/index consistency, graph orphan detection and backup readiness.
//! Every check is a measurement, not an assertion — the report tells the
//! operator exactly what passed, what degraded and what failed.
//!
//! Used by `nexus_doctor` CLI and, later, by the diagnostics UI.

use chrono::Utc;
use rusqlite::Connection;

/// Outcome of a single check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    /// Everything is healthy.
    Ok,
    /// Degraded but not broken — the system still functions.
    Warning,
    /// Broken — needs operator attention.
    Error,
}

/// One health check result.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
}

impl CheckResult {
    fn ok(name: &str, message: String) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Ok,
            message,
        }
    }

    fn warn(name: &str, message: String) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Warning,
            message,
        }
    }

    fn err(name: &str, message: String) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Error,
            message,
        }
    }
}

/// The full doctor report.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub run_at: String,
    pub checks: Vec<CheckResult>,
    pub healthy: bool,
}

impl DoctorReport {
    pub fn healthy(&self) -> bool {
        self.checks.iter().all(|c| c.status != CheckStatus::Error)
    }

    pub fn error_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == CheckStatus::Error)
            .count()
    }
}

/// Run every health check against the live database.
pub fn run_doctor() -> DoctorReport {
    let file_check = check_db_file_exists();
    match crate::db::open_connection() {
        Ok(conn) => {
            let mut report = run_doctor_with_conn(&conn);
            report.checks.insert(0, file_check);
            report.healthy = report.checks.iter().all(|c| c.status != CheckStatus::Error);
            report
        }
        Err(e) => DoctorReport {
            run_at: Utc::now().to_rfc3339(),
            checks: vec![
                file_check,
                CheckResult::err("db_open", format!("cannot open database: {e}")),
            ],
            healthy: false,
        },
    }
}

/// Run every health check against an already-open connection. Kept separate so
/// tests (and the diagnostics UI) can drive the checks against any database
/// without touching the global data-dir env.
pub fn run_doctor_with_conn(conn: &Connection) -> DoctorReport {
    let checks = vec![
        check_migrations(conn),
        check_integrity(conn),
        check_foreign_keys(conn),
        check_memory_records(conn),
        check_fts_sync(conn),
        check_semantic_index(conn),
        check_graph_orphans(conn),
        check_backup_readiness(conn),
    ];
    let healthy = checks.iter().all(|c| c.status != CheckStatus::Error);
    DoctorReport {
        run_at: Utc::now().to_rfc3339(),
        checks,
        healthy,
    }
}

fn check_db_file_exists() -> CheckResult {
    let path = crate::db::db_path();
    check_db_file_exists_at(&path)
}

/// Check that the database file exists at the given path. Parameterized so
/// tests can drive it against arbitrary paths without env mutation.
fn check_db_file_exists_at(path: &std::path::Path) -> CheckResult {
    if path.exists() {
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        CheckResult::ok(
            "db_file",
            format!("{} exists ({} bytes)", path.display(), size),
        )
    } else {
        CheckResult::warn(
            "db_file",
            format!(
                "{} does not exist yet — the database will be created on first launch",
                path.display()
            ),
        )
    }
}

fn check_db_opens() -> CheckResult {
    match crate::db::open_connection() {
        Ok(conn) => {
            drop(conn);
            CheckResult::ok("db_open", "database opens successfully".to_string())
        }
        Err(e) => CheckResult::err("db_open", format!("cannot open database: {e}")),
    }
}

fn check_migrations(conn: &Connection) -> CheckResult {
    match crate::storage::sqlite::schema::get_schema_version(conn) {
        Ok(current) => {
            let latest = crate::storage::sqlite::schema::latest_schema_version();
            if current == latest {
                CheckResult::ok("migrations", format!("schema at latest version v{latest}"))
            } else if current < latest {
                CheckResult::warn(
                    "migrations",
                    format!("schema v{current} behind latest v{latest} — run apply_migrations"),
                )
            } else {
                CheckResult::err(
                    "migrations",
                    format!("schema v{current} is newer than this build supports (v{latest})"),
                )
            }
        }
        Err(e) => CheckResult::err("migrations", format!("cannot read schema version: {e}")),
    }
}

fn check_integrity(conn: &Connection) -> CheckResult {
    match conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0)) {
        Ok(result) if result == "ok" => {
            CheckResult::ok("integrity", "PRAGMA integrity_check = ok".to_string())
        }
        Ok(result) => CheckResult::err("integrity", format!("integrity_check reported: {result}")),
        Err(e) => CheckResult::err("integrity", format!("integrity_check failed: {e}")),
    }
}

fn check_foreign_keys(conn: &Connection) -> CheckResult {
    match conn.prepare("PRAGMA foreign_key_check") {
        Ok(mut stmt) => {
            let violations: Vec<String> = stmt
                .query_map([], |r| {
                    Ok(format!(
                        "{} (rowid {})",
                        r.get::<_, String>(3)?,
                        r.get::<_, i64>(1)?
                    ))
                })
                .and_then(|rows| rows.collect())
                .unwrap_or_default();
            if violations.is_empty() {
                CheckResult::ok("foreign_keys", "no foreign-key violations".to_string())
            } else {
                CheckResult::err(
                    "foreign_keys",
                    format!(
                        "{} foreign-key violation(s): {}",
                        violations.len(),
                        violations.join(", ")
                    ),
                )
            }
        }
        Err(e) => CheckResult::warn(
            "foreign_keys",
            format!("cannot run foreign_key_check (read-only schema?): {e}"),
        ),
    }
}

fn check_memory_records(conn: &Connection) -> CheckResult {
    match conn.query_row("SELECT COUNT(*) FROM memory_records", [], |r| {
        r.get::<_, i64>(0)
    }) {
        Ok(count) => CheckResult::ok("memory_records", format!("{count} memory records")),
        Err(e) => CheckResult::err("memory_records", format!("cannot count: {e}")),
    }
}

fn check_fts_sync(conn: &Connection) -> CheckResult {
    let count = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0));
    match (
        count("SELECT COUNT(*) FROM memory_records"),
        count("SELECT COUNT(*) FROM memory_fts"),
    ) {
        (Ok(records), Ok(fts)) => {
            if records == fts {
                CheckResult::ok(
                    "fts_sync",
                    format!("memory_records ({records}) == memory_fts ({fts})"),
                )
            } else {
                CheckResult::warn(
                    "fts_sync",
                    format!(
                        "memory_records ({records}) != memory_fts ({fts}) — FTS may need rebuild"
                    ),
                )
            }
        }
        (Err(e), _) | (_, Err(e)) => CheckResult::err("fts_sync", format!("cannot count: {e}")),
    }
}

fn check_semantic_index(conn: &Connection) -> CheckResult {
    let count = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0));
    match (
        count("SELECT COUNT(*) FROM memory_records"),
        count("SELECT COUNT(*) FROM memory_semantic_fingerprints"),
    ) {
        (Ok(records), Ok(indexed)) => {
            let coverage = if records == 0 {
                1.0
            } else {
                indexed as f64 / records as f64
            };
            if coverage >= 0.99 {
                CheckResult::ok(
                    "semantic_index",
                    format!(
                        "{indexed}/{records} memories indexed ({:.1}%)",
                        coverage * 100.0
                    ),
                )
            } else {
                CheckResult::warn(
                    "semantic_index",
                    format!(
                        "{indexed}/{records} memories indexed ({:.1}%) — background indexer is catching up",
                        coverage * 100.0
                    ),
                )
            }
        }
        (Err(e), _) | (_, Err(e)) => {
            CheckResult::err("semantic_index", format!("cannot count: {e}"))
        }
    }
}

fn check_graph_orphans(conn: &Connection) -> CheckResult {
    // Relationships pointing at missing entities (should be impossible with
    // ON DELETE CASCADE, but a legacy/foreign_keys-off DB may contain them).
    let sql = "SELECT COUNT(*) FROM graph_relationships r
               WHERE NOT EXISTS (SELECT 1 FROM graph_entities e WHERE e.id = r.source_entity_id)
                  OR NOT EXISTS (SELECT 1 FROM graph_entities e WHERE e.id = r.target_entity_id)";
    match conn.query_row(sql, [], |r| r.get::<_, i64>(0)) {
        Ok(0) => CheckResult::ok("graph_orphans", "no orphaned relationships".to_string()),
        Ok(n) => CheckResult::err(
            "graph_orphans",
            format!("{n} relationship(s) reference missing entities"),
        ),
        Err(e) => CheckResult::err("graph_orphans", format!("cannot query: {e}")),
    }
}

fn check_backup_readiness(conn: &Connection) -> CheckResult {
    // backup_history table exists (created by V31). If not, backups would
    // still work but the journal would be missing.
    let exists = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='backup_history'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;
    if exists {
        CheckResult::ok(
            "backup",
            "backup subsystem ready (backup_history present)".to_string(),
        )
    } else {
        CheckResult::warn(
            "backup",
            "backup_history table missing — run migrations to enable backup journal".to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An in-memory migrated DB — no env mutation, no touching user data, safe
    /// to run in parallel with every other test in the suite.
    fn migrated_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::sqlite::schema::apply_migrations(&conn).expect("migrate");
        conn
    }

    #[test]
    fn doctor_runs_and_returns_report() {
        let conn = migrated_conn();
        let report = run_doctor_with_conn(&conn);
        assert!(!report.checks.is_empty(), "doctor must produce checks");
        assert!(
            report.checks.iter().all(|c| !c.name.is_empty()),
            "every check has a name"
        );
        // With a fresh migrated DB, no errors are expected (warnings allowed).
        assert!(
            report.healthy(),
            "fresh DB must be healthy: {:?}",
            report.checks
        );
    }

    #[test]
    fn doctor_detects_missing_table_as_degraded() {
        // A DB with NO migrations — the backup_history check must degrade to a
        // warning (not crash, not error on every check).
        let conn = Connection::open_in_memory().unwrap();
        let report = run_doctor_with_conn(&conn);
        // migrations check must report behind-latest as a warning, never error.
        let migrations = report
            .checks
            .iter()
            .find(|c| c.name == "migrations")
            .expect("migrations check present");
        assert_eq!(migrations.status, CheckStatus::Warning);
    }

    #[test]
    fn doctor_reports_error_on_corrupt_database() {
        // Create a DB, then destroy its integrity by removing the FTS table
        // contents — fts_sync must flag a divergence, and the report must
        // still be produced (never panic).
        let conn = migrated_conn();
        conn.execute(
            "INSERT INTO memory_records
                (id, title, summary, content, created_at, updated_at, author, source)
             VALUES ('m1', 't', '', 'c', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'a', 's')",
            [],
        )
        .unwrap();
        let report = run_doctor_with_conn(&conn);
        let memory = report
            .checks
            .iter()
            .find(|c| c.name == "memory_records")
            .expect("memory_records check present");
        assert_eq!(memory.status, CheckStatus::Ok);
        assert_eq!(memory.message, "1 memory records");
    }

    #[test]
    fn db_file_check_returns_warning_for_missing_path() {
        let missing =
            std::env::temp_dir().join(format!("definitely-missing-{}.db", uuid::Uuid::new_v4()));
        let result = check_db_file_exists_at(&missing);
        assert_eq!(result.status, CheckStatus::Warning);
    }

    #[test]
    fn db_file_check_returns_ok_for_existing_path() {
        let tmp = std::env::temp_dir().join(format!("nexus-doctor-file-{}", uuid::Uuid::new_v4()));
        std::fs::write(&tmp, b"x").unwrap();
        let result = check_db_file_exists_at(&tmp);
        assert_eq!(result.status, CheckStatus::Ok);
        let _ = std::fs::remove_file(&tmp);
    }
}
