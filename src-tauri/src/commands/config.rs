use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// A configuration key-value pair.
#[derive(Serialize, Deserialize, Clone)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
}

fn open_config_db() -> Result<Connection, String> {
    let conn = crate::db::open_connection()?;
    // Ensure config table exists
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS configuration_kv (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        );",
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

/// Get a configuration value by key.
#[tauri::command]
pub async fn get_config(key: String) -> Result<Option<String>, String> {
    get_config_sync(key)
}

/// Synchronous version — usable from spawn_blocking contexts.
pub fn get_config_sync(key: String) -> Result<Option<String>, String> {
    let conn = open_config_db()?;
    let result: Option<String> = conn
        .query_row(
            "SELECT value FROM configuration_kv WHERE key = ?1",
            [&key],
            |row| row.get(0),
        )
        .ok();
    Ok(result)
}

/// Get all configuration entries.
#[tauri::command]
pub async fn get_all_config() -> Result<Vec<ConfigEntry>, String> {
    let conn = open_config_db()?;
    let mut stmt = conn
        .prepare("SELECT key, value FROM configuration_kv ORDER BY key")
        .map_err(|e| e.to_string())?;
    let entries = stmt
        .query_map([], |row| {
            Ok(ConfigEntry {
                key: row.get(0)?,
                value: row.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(entries)
}

/// Set a configuration value. Creates or updates.
#[tauri::command]
pub async fn set_config(key: String, value: String) -> Result<(), String> {
    set_config_sync(&key, &value)
}

/// Synchronous version — usable from non-Tauri contexts (e.g. the updater).
pub fn set_config_sync(key: &str, value: &str) -> Result<(), String> {
    let conn = open_config_db()?;
    conn.execute(
        "INSERT OR REPLACE INTO configuration_kv (key, value) VALUES (?1, ?2)",
        [key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Delete a configuration value.
#[tauri::command]
pub async fn delete_config(key: String) -> Result<(), String> {
    delete_config_sync(&key)
}

/// Synchronous version — usable from non-Tauri contexts.
pub fn delete_config_sync(key: &str) -> Result<(), String> {
    let conn = open_config_db()?;
    conn.execute("DELETE FROM configuration_kv WHERE key = ?1", [key])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Get database stats for the settings page.
#[tauri::command]
pub async fn get_db_stats() -> Result<DbStats, String> {
    let conn = open_config_db()?;

    let memory_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM memory_records", [], |row| row.get(0))
        .unwrap_or(0);

    let entity_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM graph_entities", [], |row| row.get(0))
        .unwrap_or(0);

    let relationship_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM graph_relationships", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    let commit_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM automatic_commits", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    let snapshot_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM context_snapshots", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    // Get DB file size
    let file_size = std::fs::metadata(crate::db::db_path())
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(DbStats {
        memory_count,
        entity_count,
        relationship_count,
        commit_count,
        snapshot_count,
        db_size_bytes: file_size,
    })
}

/// Effective state of every known feature flag (plan 7.6).
#[tauri::command]
pub async fn get_feature_flags() -> Result<Vec<crate::core::config::FeatureFlagStatus>, String> {
    Ok(crate::core::config::list_flags())
}

/// Flip a feature flag ON/OFF (plan 7.6: enable → measure → rollback).
/// Unknown keys are rejected.
#[tauri::command]
pub async fn set_feature_flag(key: String, enabled: bool) -> Result<(), String> {
    crate::core::config::set_enabled(&key, enabled).map_err(|e| e.to_string())
}

#[derive(Serialize, Deserialize)]
pub struct DbStats {
    pub memory_count: i64,
    pub entity_count: i64,
    pub relationship_count: i64,
    pub commit_count: i64,
    pub snapshot_count: i64,
    pub db_size_bytes: u64,
}
