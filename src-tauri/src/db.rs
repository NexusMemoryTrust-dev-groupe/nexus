use std::path::PathBuf;
use rusqlite::Connection;

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

/// Open a new SQLite connection to the canonical database.
pub fn open_connection() -> Result<Connection, String> {
    Connection::open(db_path()).map_err(|e| format!("Failed to open DB: {}", e))
}
