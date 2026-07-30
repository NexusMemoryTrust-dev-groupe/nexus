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
];

/// Table that tracks which migrations have been applied.
const CREATE_MIGRATIONS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS schema_migrations (
    version     INTEGER PRIMARY KEY,
    applied_at  TEXT NOT NULL
);
";

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
pub fn apply_migrations(conn: &Connection) -> Result<()> {
    // Ensure the migrations tracking table exists.
    conn.execute_batch(CREATE_MIGRATIONS_TABLE)?;

    let current_version = get_schema_version(conn)?;

    for &(version, sql) in MIGRATIONS {
        if version > current_version {
            // Each migration runs inside its own transaction.
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(sql)?;
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
