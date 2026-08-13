use std::path::{Path, PathBuf};

use crate::core::backup::{
    BackupInfo, HistoryEntry, RestoreReport, create_backup, default_backup_dir, delete_backup,
    list_backups, list_history, restore_backup, verify_backup,
};
use crate::core::result::AppError;

/// Convert a core error into the IPC string, logging the full taxonomy first so
/// the structured log carries error_code/severity/component/recoverable.
fn err_msg(e: AppError) -> String {
    crate::infra::log_error(&e);
    e.to_string()
}

/// Tauri command: create a full backup into the default backup directory.
#[tauri::command]
pub fn backup_create() -> std::result::Result<BackupInfo, String> {
    let dir = default_backup_dir();
    create_backup(&dir).map_err(err_msg)
}

/// Tauri command: create a backup into an explicit directory.
#[tauri::command]
pub fn backup_create_to(dest_dir: String) -> std::result::Result<BackupInfo, String> {
    let dir = PathBuf::from(dest_dir);
    create_backup(&dir).map_err(err_msg)
}

/// Tauri command: verify a backup file (checksum + SQLite integrity).
#[tauri::command]
pub fn backup_verify(path: String) -> std::result::Result<BackupInfo, String> {
    verify_backup(Path::new(&path)).map_err(err_msg)
}

/// Tauri command: list backups in the default backup directory.
#[tauri::command]
pub fn backup_list() -> std::result::Result<Vec<BackupInfo>, String> {
    list_backups(&default_backup_dir()).map_err(err_msg)
}

/// Tauri command: list backups in an explicit directory.
#[tauri::command]
pub fn backup_list_in(dir: String) -> std::result::Result<Vec<BackupInfo>, String> {
    list_backups(Path::new(&dir)).map_err(err_msg)
}

/// Tauri command: delete a backup file.
#[tauri::command]
pub fn backup_delete(path: String) -> std::result::Result<(), String> {
    delete_backup(Path::new(&path)).map_err(err_msg)
}

/// Tauri command: restore a backup into the live database.
#[tauri::command]
pub fn backup_restore(path: String) -> std::result::Result<RestoreReport, String> {
    restore_backup(Path::new(&path)).map_err(err_msg)
}

/// Tauri command: list the backup journal (history of created/restored/deleted).
#[tauri::command]
pub fn backup_history() -> std::result::Result<Vec<HistoryEntry>, String> {
    list_history().map_err(err_msg)
}

/// Tauri command: the default backup directory (for UI hints).
#[tauri::command]
pub fn backup_default_dir() -> std::result::Result<String, String> {
    Ok(default_backup_dir().display().to_string())
}
