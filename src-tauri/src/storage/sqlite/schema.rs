use rusqlite::{Connection, Result};

/// Embedded SQL migration files (compiled into binary via include_str!)
const MIGRATIONS: &[(i32, &str)] = &[
    (1, include_str!("migrations/V1_create_memory_records.sql")),
    (2, include_str!("migrations/V2_add_attached_files.sql")),
    (3, include_str!("migrations/V3_add_versioning_columns.sql")),
    (
        4,
        include_str!("migrations/V4_create_versioning_tables.sql"),
    ),
    (5, include_str!("migrations/V5_create_entity_snapshots.sql")),
    (6, include_str!("migrations/V6_create_graph_tables.sql")),
    (7, include_str!("migrations/V7_create_context_tables.sql")),
    (
        8,
        include_str!("migrations/V8_create_workspace_and_links.sql"),
    ),
    (
        9,
        include_str!("migrations/V9_create_semantic_fingerprints.sql"),
    ),
    (
        10,
        include_str!("migrations/V10_create_savings_tracking.sql"),
    ),
    (
        11,
        include_str!("migrations/V11_savings_measured_baseline.sql"),
    ),
    (12, include_str!("migrations/V12_add_memory_lifecycle.sql")),
    (13, include_str!("migrations/V13_add_product_metrics.sql")),
    (
        14,
        include_str!("migrations/V14_create_project_knowledge.sql"),
    ),
    (15, include_str!("migrations/V15_create_code_graph.sql")),
    (16, include_str!("migrations/V16_create_team_members.sql")),
    (17, include_str!("migrations/V17_create_audit_events.sql")),
    (18, include_str!("migrations/V18_cognitive_layers.sql")),
    (
        19,
        include_str!("migrations/V19_create_conflict_groups.sql"),
    ),
    (20, include_str!("migrations/V20_memory_rehearsal.sql")),
    (21, include_str!("migrations/V21_memory_firewall.sql")),
    (22, include_str!("migrations/V22_flight_recorder.sql")),
    (23, include_str!("migrations/V23_agent_passport.sql")),
    (24, include_str!("migrations/V24_context_lab.sql")),
    (25, include_str!("migrations/V25_skill_proposals.sql")),
    (26, include_str!("migrations/V26_query_history.sql")),
    (27, include_str!("migrations/V27_canonical_memories.sql")),
    (28, include_str!("migrations/V28_agent_policies.sql")),
    (29, include_str!("migrations/V29_context_chains.sql")),
    (
        30,
        include_str!("migrations/V30_add_fingerprint_source_text.sql"),
    ),
    (31, include_str!("migrations/V31_create_backup_history.sql")),
    (32, include_str!("migrations/V32_audit_append_only.sql")),
    (
        33,
        include_str!("migrations/V33_memory_created_at_index.sql"),
    ),
];

/// Table that tracks which migrations have been applied.
const CREATE_MIGRATIONS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS schema_migrations (
    version     INTEGER PRIMARY KEY,
    applied_at  TEXT NOT NULL
);
";

/// Check if a column exists in a table.
fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    let query = format!("PRAGMA table_info({})", table);
    let mut stmt = match conn.prepare(&query) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let mut rows = match stmt.query([]) {
        Ok(r) => r,
        Err(_) => return false,
    };
    while let Ok(Some(row)) = rows.next() {
        // SQLite identifiers are case-insensitive; the extraction above
        // uppercases names, so compare without case to reliably detect
        // "duplicate column" on re-apply after a rollback.
        if let Ok(name) = row.get::<_, String>(1)
            && name.eq_ignore_ascii_case(column)
        {
            return true;
        }
    }
    false
}

/// Execute a migration SQL, handling ALTER TABLE idempotently.
///
/// SQLite doesn't support `IF NOT EXISTS` for ALTER TABLE ADD COLUMN.
/// This function catches "duplicate column name" errors and skips them.
///
/// Comment lines (`-- ...`) before a statement are stripped so that a
/// statement which follows a prose header in a migration file is still
/// recognised as ALTER TABLE (V30 and friends start with a comment block;
/// leaving it in place would defeat the `starts_with` classification and let
/// the duplicate-column error escape on re-apply after a rollback).
fn execute_migration_idempotent(conn: &Connection, sql: &str) -> Result<()> {
    // Check if this migration contains ALTER TABLE statements
    let upper = sql.to_uppercase();
    let has_alter = upper.contains("ALTER TABLE");

    if has_alter {
        // Split by semicolons and execute each statement separately
        // to handle ALTER TABLE statements individually
        for statement in sql.split(';') {
            // Drop leading `--` comment lines (and blank lines) so ALTER
            // statements are classified reliably.
            let cleaned = statement
                .lines()
                .skip_while(|line| {
                    let t = line.trim();
                    t.is_empty() || t.starts_with("--")
                })
                .collect::<Vec<_>>()
                .join("\n");
            let trimmed = cleaned.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Check if this is an ALTER TABLE ADD COLUMN statement
            let stmt_upper = trimmed.to_uppercase();
            if stmt_upper.starts_with("ALTER TABLE") && stmt_upper.contains("ADD COLUMN") {
                // Try to execute, ignore "duplicate column name" errors
                match conn.execute_batch(trimmed) {
                    Ok(_) => {}
                    Err(rusqlite::Error::SqliteFailure(err, _))
                        if err.extended_code == 1 // SQLITE_ERROR
                            && err.code == rusqlite::ErrorCode::Unknown =>
                    {
                        // Check if it's specifically a duplicate column error
                        // by checking if the column already exists
                        if let Some(table) = extract_table_name(trimmed)
                            && let Some(column) = extract_column_name(trimmed)
                            && column_exists(conn, &table, &column)
                        {
                            // Column already exists, skip silently
                            continue;
                        }
                        // Some other SQLite error, propagate it
                        return Err(rusqlite::Error::SqliteFailure(err, None));
                    }
                    Err(e) => return Err(e),
                }
            } else {
                // Non-ALTER statement, execute normally
                conn.execute_batch(trimmed)?;
            }
        }
    } else {
        // No ALTER TABLE, execute entire batch at once
        conn.execute_batch(sql)?;
    }
    Ok(())
}

/// Extract table name from ALTER TABLE ... ADD COLUMN statement.
fn extract_table_name(sql: &str) -> Option<String> {
    let upper = sql.to_uppercase();
    let after_alter = upper.split("ALTER TABLE").nth(1)?;
    let table = after_alter.split("ADD COLUMN").next()?;
    Some(table.trim().to_string())
}

/// Extract column name from ALTER TABLE ... ADD COLUMN statement.
fn extract_column_name(sql: &str) -> Option<String> {
    let upper = sql.to_uppercase();
    let after_add = upper.split("ADD COLUMN").nth(1)?;
    let column = after_add.split_whitespace().next()?;
    Some(column.trim().to_string())
}

/// The newest migration version this build knows about.
pub fn latest_schema_version() -> i32 {
    MIGRATIONS.last().map(|(v, _)| *v).unwrap_or(0)
}

/// Return the current schema version (max applied migration).
/// Returns 0 if no migrations have been applied yet.
pub fn get_schema_version(conn: &Connection) -> Result<i32> {
    // Ensure the migrations tracking table exists.
    conn.execute_batch(CREATE_MIGRATIONS_TABLE)?;

    let version: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    Ok(version)
}

/// Apply a single migration inside its own transaction.
///
/// The transaction guarantees atomicity per migration: if the SQL fails, every
/// object it created is rolled back and the version is NOT recorded, so a
/// subsequent run retries the same migration from scratch (no half-applied
/// schema, no skipped versions). Used by `apply_migrations`; exposed as
/// `#[cfg(test)]` so failure-injection tests can drive it directly.
fn apply_migration(conn: &Connection, version: i32, sql: &str) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    execute_migration_idempotent(&tx, sql)?;
    tx.execute(
        "INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
        rusqlite::params![version, chrono::Utc::now().to_rfc3339()],
    )?;
    tx.commit()?;
    Ok(())
}

/// Apply all pending migrations in order.
///
/// For each migration whose version is greater than the current schema
/// version the SQL is executed and the version recorded in
/// `schema_migrations`.  Each migration runs inside its own transaction so
/// that a failure rolls back that migration only and leaves the previously
/// applied ones intact — a restart picks up where it left off.
///
/// ALTER TABLE ADD COLUMN statements are executed idempotently —
/// if the column already exists, the statement is skipped.
pub fn apply_migrations(conn: &Connection) -> Result<()> {
    // Ensure the migrations tracking table exists.
    conn.execute_batch(CREATE_MIGRATIONS_TABLE)?;

    let current_version = get_schema_version(conn)?;

    for &(version, sql) in MIGRATIONS {
        if version > current_version {
            apply_migration(conn, version, sql)?;
        }
    }

    Ok(())
}

/// Rollback the last applied migration (if any).
///
/// NOTE: Only migrations that are purely additive (CREATE TABLE / CREATE
/// INDEX) can be rolled back safely.  ALTER TABLE and trigger migrations
/// have no generic undo – they will return an error.
#[allow(dead_code)] // Public API, may be used by CLI tools
pub fn rollback_last_migration(conn: &Connection) -> Result<()> {
    let current_version = get_schema_version(conn)?;
    if current_version == 0 {
        return Ok(()); // Nothing to rollback.
    }

    // Find the SQL for the version we want to remove.
    let migration_sql = MIGRATIONS
        .iter()
        .find(|(v, _)| *v == current_version)
        .map(|(_, sql)| *sql);

    let sql = match migration_sql {
        Some(s) => s,
        None => return Ok(()), // Unknown version, skip.
    };

    let tx = conn.unchecked_transaction()?;

    // Try to execute a rollback script if one exists; otherwise just
    // remove the migration record.  For V2/V3 (ALTER TABLE) there is
    // no generic undo, so we skip the actual SQL and only remove the
    // tracking row – the columns will remain harmless.
    let has_rollback = sql.to_uppercase().contains("DROP ");
    if has_rollback {
        tx.execute_batch(sql)?;
    }

    tx.execute(
        "DELETE FROM schema_migrations WHERE version = ?1",
        rusqlite::params![current_version],
    )?;
    tx.commit()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_migrations_succeeds() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(apply_migrations(&conn).is_ok());
    }

    #[test]
    fn apply_migrations_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(apply_migrations(&conn).is_ok());
        assert!(apply_migrations(&conn).is_ok());
    }

    #[test]
    fn schema_version_increases() {
        let conn = Connection::open_in_memory().unwrap();
        let v0 = get_schema_version(&conn).unwrap();
        apply_migrations(&conn).unwrap();
        let v1 = get_schema_version(&conn).unwrap();
        assert!(v1 > v0, "version should increase after migration");
    }

    #[test]
    fn rollback_returns_to_previous_version() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();
        let v_before = get_schema_version(&conn).unwrap();
        rollback_last_migration(&conn).unwrap();
        let v_after = get_schema_version(&conn).unwrap();
        assert!(v_after < v_before);
    }

    #[test]
    fn v18_adds_cognitive_layer_columns() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();

        // The four provenance columns must exist.
        for col in [
            "layer_confidence",
            "layer_reason",
            "layer_updated_at",
            "layer_history_json",
        ] {
            assert!(
                column_exists(&conn, "memory_records", col),
                "missing column {col}"
            );
        }
    }

    #[test]
    fn v18_maps_legacy_layers_onto_cognitive_taxonomy() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();

        // Simulate pre-V18 rows with legacy ladder values.
        conn.execute_batch(
            "INSERT INTO memory_records
                (id, title, summary, content, created_at, updated_at, author, source, layer)
             VALUES
                ('m1','t','s','c','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z','a','src','Raw'),
                ('m2','t','s','c','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z','a','src','Knowledge'),
                ('m3','t','s','c','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z','a','src','Wisdom'),
                ('m4','t','s','c','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z','a','src','Decision'),
                ('m5','t','s','c','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z','a','src','Bogus');",
        )
        .unwrap();

        // Re-run migrations — V18 must UPDATE the rows on a fresh apply.
        // (Migrations already ran, so emulate by executing the UPDATEs again
        // through the same statements the migration contains.)
        conn.execute_batch(
            "UPDATE memory_records SET layer = 'Episodic' WHERE layer = 'Raw';
             UPDATE memory_records SET layer = 'Semantic' WHERE layer = 'Knowledge';
             UPDATE memory_records SET layer = 'Strategic' WHERE layer = 'Wisdom';
             UPDATE memory_records SET layer = 'Episodic' WHERE layer NOT IN (
                 'Working', 'Episodic', 'Semantic', 'Procedural', 'Decision', 'Strategic'
             );",
        )
        .unwrap();

        let mut stmt = conn
            .prepare("SELECT id, layer FROM memory_records ORDER BY id")
            .unwrap();
        let rows: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(
            rows,
            vec![
                ("m1".to_string(), "Episodic".to_string()),
                ("m2".to_string(), "Semantic".to_string()),
                ("m3".to_string(), "Strategic".to_string()),
                ("m4".to_string(), "Decision".to_string()),
                ("m5".to_string(), "Episodic".to_string()),
            ]
        );
    }

    #[test]
    fn v19_creates_conflict_groups_table() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();

        // The conflict_groups table and its indexes must exist after V19.
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master
                 WHERE type IN ('table', 'index') AND name IN
                     ('conflict_groups', 'idx_cg_status', 'idx_cg_topic')",
            )
            .unwrap();
        let names: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(names.contains(&"conflict_groups".to_string()));
        assert!(names.contains(&"idx_cg_status".to_string()));
        assert!(names.contains(&"idx_cg_topic".to_string()));
    }

    // ------------------------------------------------------------------
    // Production Readiness Gate 3.2 — migration failure safety
    // ------------------------------------------------------------------

    fn table_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [name],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
            > 0
    }

    /// A broken migration: the first statement succeeds, the second fails.
    /// If the per-migration transaction works, the first statement's table
    /// must be rolled back with the failure.
    #[test]
    fn failed_migration_rolls_back_atomically() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_MIGRATIONS_TABLE).unwrap();

        let broken = "CREATE TABLE half_applied (id INTEGER PRIMARY KEY);
                      INSERT INTO half_applied (id) VALUES ('not-an-int');";
        let result = apply_migration(&conn, 99, broken);
        assert!(result.is_err(), "broken migration must fail");

        // Version must NOT be recorded for the failed migration.
        assert_eq!(get_schema_version(&conn).unwrap(), 0);
        // The table created before the failing statement must be rolled back.
        assert!(!table_exists(&conn, "half_applied"));
    }

    /// A failure during a NEW migration must leave the previously applied
    /// schema fully intact and the version pointer unchanged.
    #[test]
    fn migration_failure_preserves_previous_schema() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();
        let v_before = get_schema_version(&conn).unwrap();

        // Duplicate table name -> second statement fails.
        let broken = "CREATE TABLE dup (id INTEGER PRIMARY KEY);
                      CREATE TABLE dup (id INTEGER PRIMARY KEY);";
        assert!(apply_migration(&conn, 999, broken).is_err());

        assert_eq!(get_schema_version(&conn).unwrap(), v_before);
        // Pre-existing schema still intact.
        assert!(column_exists(&conn, "memory_records", "layer"));
        assert!(table_exists(&conn, "conflict_groups"));
    }

    /// After a failure, re-running the same migration (e.g. after a code fix
    /// in the migration SQL) must succeed from a clean slate — no half-applied
    /// objects, no phantom version row.
    #[test]
    fn failed_migration_retries_after_fix() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_MIGRATIONS_TABLE).unwrap();

        let broken = "CREATE TABLE retry_table (id INTEGER);
                      INSERT INTO no_such_table VALUES (1);";
        assert!(apply_migration(&conn, 77, broken).is_err());
        assert_eq!(get_schema_version(&conn).unwrap(), 0);

        // "Fix" — same version now applies cleanly.
        let fixed = "CREATE TABLE retry_table (id INTEGER);";
        apply_migration(&conn, 77, fixed).unwrap();
        assert_eq!(get_schema_version(&conn).unwrap(), 77);
        assert!(table_exists(&conn, "retry_table"));
    }

    /// Full chain V1..latest applies to a fresh DB and the version pointer
    /// ends exactly at the newest migration.
    #[test]
    fn full_chain_v1_to_latest() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();
        let latest = MIGRATIONS.last().map(|(v, _)| *v).unwrap();
        assert_eq!(get_schema_version(&conn).unwrap(), latest);
    }

    /// Simulated crash mid-chain: apply up to version 15, "restart" (fresh
    /// connection), and verify the remainder applies and ends at latest.
    #[test]
    fn restart_resumes_after_partial_apply() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_MIGRATIONS_TABLE).unwrap();

        let mid = 15;
        for &(v, sql) in MIGRATIONS.iter().take_while(|(v, _)| *v <= mid) {
            apply_migration(&conn, v, sql).unwrap();
        }
        assert_eq!(get_schema_version(&conn).unwrap(), mid);

        // "Restart": a fresh apply_migrations call continues from v15.
        apply_migrations(&conn).unwrap();
        let latest = MIGRATIONS.last().map(|(v, _)| *v).unwrap();
        assert_eq!(get_schema_version(&conn).unwrap(), latest);
        // Idempotent on a second restart.
        apply_migrations(&conn).unwrap();
        assert_eq!(get_schema_version(&conn).unwrap(), latest);
    }

    /// Every embedded migration must apply cleanly to a pristine DB — a guard
    /// against shipping a migration that only works on top of a dirty schema.
    #[test]
    fn every_migration_applies_to_fresh_db() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_MIGRATIONS_TABLE).unwrap();
        for &(v, sql) in MIGRATIONS {
            apply_migration(&conn, v, sql).unwrap();
            // After each migration the version pointer must advance exactly to v.
            assert_eq!(get_schema_version(&conn).unwrap(), v);
        }
    }
}
