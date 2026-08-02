use rusqlite::Connection;
use std::path::PathBuf;

/// Get the canonical database path shared by all modules.
/// Uses %LOCALAPPDATA%/Nexus/nexus.db on Windows, ~/.nexus/nexus.db otherwise.
pub fn db_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
        let p = PathBuf::from(appdata).join("Nexus");
        let _ = std::fs::create_dir_all(&p);
        return p.join("nexus.db");
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join(".nexus");
        let _ = std::fs::create_dir_all(&p);
        return p.join("nexus.db");
    }
    let _ = std::fs::create_dir_all(".nexus");
    PathBuf::from(".nexus/nexus.db")
}

/// Busy timeout for every connection. The app opens several independent
/// connections to the same file (graph, memory, snapshots, savings), so a
/// concurrent writer must wait instead of failing fast with SQLITE_BUSY.
const BUSY_TIMEOUT_MS: u32 = 5_000;

/// Apply the pragmas every connection needs.
///
/// `foreign_keys` is OFF by default in SQLite — without it the
/// `ON DELETE CASCADE` clauses on `graph_relationships` are silently ignored,
/// which leaves orphaned edges pointing at deleted entities.
fn configure(conn: &Connection) -> Result<(), String> {
    conn.busy_timeout(std::time::Duration::from_millis(BUSY_TIMEOUT_MS as u64))
        .map_err(|e| format!("Failed to set busy_timeout: {}", e))?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;\
         PRAGMA foreign_keys=ON;\
         PRAGMA synchronous=NORMAL;",
    )
    .map_err(|e| format!("Failed to configure DB: {}", e))?;
    Ok(())
}

/// Open a new SQLite connection to the canonical database.
pub fn open_connection() -> Result<Connection, String> {
    let conn = Connection::open(db_path()).map_err(|e| format!("Failed to open DB: {}", e))?;
    configure(&conn)?;
    Ok(conn)
}
