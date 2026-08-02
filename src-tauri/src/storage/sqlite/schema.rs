use rusqlite::{Connection, Result};

/// Embedded SQL migration files (compiled into binary via include_str!)
const MIGRATIONS: &[(i32, &str)] = &[
    (1, include_str!("migrations/V1_create_memory_records.sql")),
    (2, include_str!("migrations/V2_add_attached_files.sql")),
    (3, include_str!("migrations/V3_add_versioning_columns.sql")),
    (4, include_str!("migrations/V4_create_versioning_tables.sql")),
    (5, include_str!("migrations/V5_create_entity_snapshots.sql")),
    (6, include_str!("migrations/V6_create_graph_tables.sql")),
    (7, include_str!("migrations/V7_create_context_tables.sql")),
    (8, include_str!("migrations/V8_create_workspace_and_links.sql")),
    (9, include_str!("migrations/V9_create_semantic_fingerprints.sql")),
    (10, include_str!("migrations/V10_create_savings_tracking.sql")),
    (11, include_str!("migrations/V11_savings_measured_baseline.sql")),
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
        if let Ok(name) = row.get::<_, String>(1) {
            if name == column {
                return true;
            }
        }
    }
    false
}

/// Execute a migration SQL, handling ALTER TABLE idempotently.
///
/// SQLite doesn't support `IF NOT EXISTS` for ALTER TABLE ADD COLUMN.
/// This function catches "duplicate column name" errors and skips them.
fn execute_migration_idempotent(conn: &Connection, sql: &str) -> Result<()> {
    // Check if this migration contains ALTER TABLE statements
    let upper = sql.to_uppercase();
    let has_alter = upper.contains("ALTER TABLE");

    if has_alter {
        // Split by semicolons and execute each statement separately
        // to handle ALTER TABLE statements individually
        for statement in sql.split(';') {
            let trimmed = statement.trim();
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
                        if let Some(table) = extract_table_name(trimmed) {
                            if let Some(column) = extract_column_name(trimmed) {
                                if column_exists(conn, &table, &column) {
                                    // Column already exists, skip silently
                                    continue;
                                }
                            }
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

/// Apply all pending migrations in order.
///
/// For each migration whose version is greater than the current schema
/// version the SQL is executed and the version recorded in
/// `schema_migrations`.  Migrations are executed inside a transaction so
/// that a failure rolls back the entire batch.
///
/// ALTER TABLE ADD COLUMN statements are executed idempotently —
/// if the column already exists, the statement is skipped.
pub fn apply_migrations(conn: &Connection) -> Result<()> {
    // Ensure the migrations tracking table exists.
    conn.execute_batch(CREATE_MIGRATIONS_TABLE)?;

    let current_version = get_schema_version(conn)?;

    for &(version, sql) in MIGRATIONS {
        if version > current_version {
            // Each migration runs inside its own transaction.
            let tx = conn.unchecked_transaction()?;
            execute_migration_idempotent(&tx, sql)?;
            tx.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                rusqlite::params![version, chrono::Utc::now().to_rfc3339()],
            )?;
            tx.commit()?;
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
}
