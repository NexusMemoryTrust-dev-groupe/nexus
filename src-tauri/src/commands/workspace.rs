use crate::commands::files::{self, FileEntry, normalize_path};
use crate::db::open_connection;
use rusqlite::params;
use rusqlite::OptionalExtension;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

// ── DB Row type ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct SyncResult {
    pub tree: Option<FileEntry>,
    pub stale_found: bool,
}

#[derive(Debug, Clone)]
struct WsRow {
    id: String,
    _project_id: String,
    name: String,
    native_path: String,
    parent_id: Option<String>,
    is_dir: bool,
    size_bytes: u64,
    mime_type: String,
    _created_at: String,
    sort_order: i32,
}

fn read_rows(project_id: &str) -> Result<Vec<WsRow>, String> {
    let conn = open_connection()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, project_id, name, native_path, parent_id, is_dir, size_bytes, mime_type, created_at, sort_order
             FROM workspace_entries WHERE project_id = ?1 ORDER BY is_dir DESC, sort_order ASC, name ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![project_id], |row| {
            Ok(WsRow {
                id: row.get(0)?,
                _project_id: row.get(1)?,
                name: row.get(2)?,
                native_path: row.get(3)?,
                parent_id: row.get(4)?,
                is_dir: row.get::<_, i32>(5)? == 1,
                size_bytes: row.get::<_, i64>(6)? as u64,
                mime_type: row.get(7)?,
                _created_at: row.get(8)?,
                sort_order: row.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

fn insert_row(
    project_id: &str,
    name: &str,
    native_path: &str,
    parent_id: Option<&str>,
    is_dir: bool,
    size_bytes: u64,
    mime_type: &str,
    sort_order: i32,
) -> Result<String, String> {
    let conn = open_connection()?;
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO workspace_entries (id, project_id, name, native_path, parent_id, is_dir, size_bytes, mime_type, created_at, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            project_id,
            name,
            native_path,
            parent_id,
            is_dir as i32,
            size_bytes as i64,
            mime_type,
            now,
            sort_order,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

fn delete_by_native_path(project_id: &str, native_path: &str) -> Result<(), String> {
    let conn = open_connection()?;
    // Delete this entry AND all descendants (recursive)
    let children = read_rows(project_id)?;
    let prefix = native_path.trim_end_matches('\\').trim_end_matches('/').to_string();
    let ids_to_delete: Vec<String> = children
        .iter()
        .filter(|r| {
            r.native_path == native_path
                || r.native_path.starts_with(&(prefix.clone() + "\\"))
                || r.native_path.starts_with(&(prefix.clone() + "/"))
        })
        .map(|r| r.id.clone())
        .collect();
    for id in &ids_to_delete {
        conn.execute("DELETE FROM workspace_entries WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn _delete_by_id(project_id: &str, id: &str) -> Result<(), String> {
    let conn = open_connection()?;
    // Get the native_path first
    let row: Option<WsRow> = {
        let mut stmt = conn
            .prepare("SELECT id, project_id, name, native_path, parent_id, is_dir, size_bytes, mime_type, created_at, sort_order FROM workspace_entries WHERE id = ?1 AND project_id = ?2")
            .map_err(|e| e.to_string())?;
        stmt.query_row(params![id, project_id], |row| {
            Ok(WsRow {
                id: row.get(0)?,
                _project_id: row.get(1)?,
                name: row.get(2)?,
                native_path: row.get(3)?,
                parent_id: row.get(4)?,
                is_dir: row.get::<_, i32>(5)? == 1,
                size_bytes: row.get::<_, i64>(6)? as u64,
                mime_type: row.get(7)?,
                _created_at: row.get(8)?,
                sort_order: row.get(9)?,
            })
        })
        .optional()
        .map_err(|e| e.to_string())?
    };
    if let Some(r) = row {
        delete_by_native_path(project_id, &r.native_path)?;
    }
    Ok(())
}

/// Build a FileEntry tree from flat workspace rows.
fn build_tree(rows: &[WsRow]) -> Option<FileEntry> {
    // Map: id → row
    // Map parent_id → children
    let mut children_of: HashMap<Option<String>, Vec<&WsRow>> = HashMap::new();
    for r in rows {
        children_of
            .entry(r.parent_id.clone())
            .or_default()
            .push(r);
    }

    // Recursive builder
    fn build_sub(parent_id: Option<&str>, children_of: &HashMap<Option<String>, Vec<&WsRow>>) -> Vec<FileEntry> {
        let key = parent_id.map(|s| s.to_string());
        let kids = children_of.get(&key).cloned().unwrap_or_default();
        let mut entries: Vec<FileEntry> = kids
            .into_iter()
            .map(|r| {
                let sub_children = if r.is_dir {
                    Some(build_sub(Some(&r.id), children_of))
                } else {
                    None
                };
                FileEntry {
                    name: r.name.clone(),
                    path: r.native_path.clone(),
                    is_dir: r.is_dir,
                    size_bytes: r.size_bytes,
                    mime_type: r.mime_type.clone(),
                    children: sub_children,
                }
            })
            .collect();
        entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        entries
    }

    let root_children = build_sub(None, &children_of);
    if root_children.is_empty() {
        return None;
    }
    // Create a virtual root entry
    Some(FileEntry {
        name: "workspace".to_string(),
        path: String::new(),
        is_dir: true,
        size_bytes: 0,
        mime_type: "inode/directory".to_string(),
        children: Some(root_children),
    })
}

/// Recursively add a native folder to the workspace.
fn add_folder_recursive(
    project_id: &str,
    folder_path: &Path,
    parent_id: Option<&str>,
    sort_order: &mut i32,
) -> Result<String, String> {
    let name = folder_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let abs = folder_path.to_string_lossy().to_string();
    let id = insert_row(project_id, &name, &abs, parent_id, true, 0, "inode/directory", *sort_order)?;
    *sort_order += 1;

    if folder_path.is_dir() {
        if let Ok(entries) = fs::read_dir(folder_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    add_folder_recursive(project_id, &path, Some(&id), sort_order)?;
                } else {
                    let child_name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let meta = fs::metadata(&path).ok();
                    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                    let mime = files::mime_from_ext_public(&path);
                    insert_row(project_id, &child_name, &path.to_string_lossy(), Some(&id), false, size, &mime, *sort_order)?;
                    *sort_order += 1;
                }
            }
        }
    }
    Ok(id)
}

// ── Tauri Commands ──────────────────────────────────────────────

/// Get the workspace tree for a project. Returns the virtual root with children.
#[tauri::command]
pub async fn get_workspace_tree(project_id: String) -> Result<Option<FileEntry>, String> {
    let rows = read_rows(&project_id)?;
    Ok(build_tree(&rows))
}

/// Add native file(s)/folder(s) to the workspace (deduplicates by native_path).
#[tauri::command]
pub async fn add_to_workspace(
    project_id: String,
    paths: Vec<String>,
) -> Result<Option<FileEntry>, String> {
    // Collect existing paths to prevent duplicates (React StrictMode fires effects twice)
    let existing_rows = read_rows(&project_id)?;
    let existing_paths: std::collections::HashSet<String> = existing_rows
        .iter()
        .map(|r| r.native_path.trim_end_matches('\\').trim_end_matches('/').to_lowercase())
        .collect();
    let mut sort_order: i32 = existing_rows.len() as i32;
    for raw in &paths {
        let p = PathBuf::from(normalize_path(raw));
        let normalized = p.to_string_lossy().trim_end_matches('\\').trim_end_matches('/').to_lowercase();
        if existing_paths.contains(&normalized) {
            continue; // Already in workspace — skip
        }
        if p.is_dir() {
            add_folder_recursive(&project_id, &p, None, &mut sort_order)?;
        } else if p.is_file() {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let meta = fs::metadata(&p).ok();
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let mime = files::mime_from_ext_public(&p);
            insert_row(&project_id, &name, &p.to_string_lossy(), None, false, size, &mime, sort_order)?;
            sort_order += 1;
        }
    }
    let rows = read_rows(&project_id)?;
    Ok(build_tree(&rows))
}

/// Create a new file in the workspace (creates on disk AND adds to DB).
#[tauri::command]
pub async fn create_workspace_entry(
    project_id: String,
    parent_path: String,
    name: String,
    is_dir: bool,
) -> Result<FileEntry, String> {
    let sep = if parent_path.contains('\\') { '\\' } else { '/' };
    let child_path = format!("{}{}{}", parent_path, sep, name);

    // Create on disk
    if is_dir {
        fs::create_dir_all(&child_path).map_err(|e| e.to_string())?;
    } else {
        if let Some(parent) = Path::new(&child_path).parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&child_path, "").map_err(|e| e.to_string())?;
    }

    // Add to workspace DB — find parent_id by native_path
    let parent_id = find_id_by_path(&project_id, &parent_path)?;
    let mime = if is_dir {
        "inode/directory".to_string()
    } else {
        files::mime_from_ext_public(Path::new(&child_path))
    };
    let meta = fs::metadata(&child_path).ok();
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let sort_order = count_children(&project_id, parent_id.as_deref()) as i32;
    let _id = insert_row(&project_id, &name, &child_path, parent_id.as_deref(), is_dir, size, &mime, sort_order)?;

    Ok(FileEntry {
        name,
        path: child_path,
        is_dir,
        size_bytes: size,
        mime_type: mime,
        children: if is_dir { Some(vec![]) } else { None },
    })
}

/// Rename a workspace entry (renames on disk AND updates DB).
#[tauri::command]
pub async fn rename_workspace_entry(
    project_id: String,
    old_path: String,
    new_name: String,
) -> Result<String, String> {
    let normalized = normalize_path(&old_path);
    let path = PathBuf::from(&normalized);
    if !path.exists() {
        return Err(format!("'{}' not found", normalized));
    }
    let parent = path
        .parent()
        .ok_or_else(|| "Cannot determine parent directory".to_string())?;
    let new_path = parent.join(&new_name);

    // Rename on disk
    fs::rename(&path, &new_path).map_err(|e| e.to_string())?;
    let new_abs = new_path.to_string_lossy().to_string();

    // Update all workspace entries with this path prefix
    let conn = open_connection()?;
    // Update the entry itself
    conn.execute(
        "UPDATE workspace_entries SET name = ?1, native_path = ?2 WHERE project_id = ?3 AND native_path = ?4",
        params![new_name, new_abs, project_id, normalized],
    )
    .map_err(|e| e.to_string())?;
    // Update all children (recursive rename of path prefix)
    let old_prefix = normalized.trim_end_matches('\\').trim_end_matches('/');
    let new_prefix = new_abs.trim_end_matches('\\').trim_end_matches('/');
    let offset = (old_prefix.len() + 1) as i32;
    conn.execute(
        "UPDATE workspace_entries SET native_path = ?1 || SUBSTR(native_path, ?2) WHERE project_id = ?3 AND native_path LIKE ?4",
        params![new_prefix, offset, project_id, format!("{}\\%", old_prefix)],
    )
    .map_err(|e| e.to_string())?;
    // Also handle forward-slash paths
    conn.execute(
        "UPDATE workspace_entries SET native_path = ?1 || SUBSTR(native_path, ?2) WHERE project_id = ?3 AND native_path LIKE ?4",
        params![new_prefix, offset, project_id, format!("{}/%", old_prefix)],
    )
    .map_err(|e| e.to_string())?;

    Ok(new_abs)
}

/// Delete a workspace entry (deletes from disk AND removes from DB).
#[tauri::command]
pub async fn delete_workspace_entry(
    project_id: String,
    file_path: String,
) -> Result<(), String> {
    let normalized = normalize_path(&file_path);
    let path = PathBuf::from(&normalized);

    // Delete from disk
    if path.exists() {
        if path.is_dir() {
            fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
        } else {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }

    // Remove from workspace DB (including all descendants)
    delete_by_native_path(&project_id, &normalized)?;
    Ok(())
}

/// Move a workspace entry (moves on disk AND updates DB).
#[tauri::command]
pub async fn move_workspace_entry(
    project_id: String,
    source_path: String,
    dest_dir: String,
) -> Result<String, String> {
    let src = PathBuf::from(normalize_path(&source_path));
    let dst_dir = PathBuf::from(normalize_path(&dest_dir));

    if !src.exists() {
        return Err(format!("Source does not exist: {}", src.display()));
    }
    if !dst_dir.is_dir() {
        return Err(format!("Destination is not a directory: {}", dst_dir.display()));
    }

    let file_name = src.file_name().ok_or("Cannot get file name from source")?;
    let dest = dst_dir.join(file_name);
    if dest.exists() {
        return Err(format!("A file or folder already exists at: {}", dest.display()));
    }

    let old_abs = src.to_string_lossy().to_string();

    // Move on disk
    fs::rename(&src, &dest).map_err(|e| format!("Move failed: {}", e))?;
    let new_abs = dest.to_string_lossy().to_string();

    // Update DB entries
    let conn = open_connection()?;
    let old_prefix = old_abs.trim_end_matches('\\').trim_end_matches('/');
    let new_prefix = new_abs.trim_end_matches('\\').trim_end_matches('/');
    let offset = (old_prefix.len() + 1) as i32;
    conn.execute(
        "UPDATE workspace_entries SET native_path = ?1 || SUBSTR(native_path, ?2) WHERE project_id = ?3 AND native_path LIKE ?4",
        params![new_prefix, offset, project_id, format!("{}\\%", old_prefix)],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE workspace_entries SET native_path = ?1 || SUBSTR(native_path, ?2) WHERE project_id = ?3 AND native_path LIKE ?4",
        params![new_prefix, offset, project_id, format!("{}/%", old_prefix)],
    )
    .map_err(|e| e.to_string())?;
    // Update the entry itself (path + parent_id)
    conn.execute(
        "UPDATE workspace_entries SET native_path = ?1 WHERE project_id = ?2 AND native_path = ?3",
        params![new_abs, project_id, old_abs],
    )
    .map_err(|e| e.to_string())?;

    // Repoint parent_id to the destination directory
    let dest_dir_abs = dst_dir.to_string_lossy().to_string();
    let dest_dir_id = find_id_by_path(&project_id, &dest_dir_abs)?;
    conn.execute(
        "UPDATE workspace_entries SET parent_id = ?1 WHERE project_id = ?2 AND native_path = ?3",
        params![dest_dir_id, project_id, new_abs],
    )
    .map_err(|e| e.to_string())?;

    Ok(new_abs)
}

/// Remove an entry from the workspace DB only (does NOT delete from disk).
#[tauri::command]
pub async fn remove_from_workspace(
    project_id: String,
    file_path: String,
) -> Result<(), String> {
    let normalized = normalize_path(&file_path);
    delete_by_native_path(&project_id, &normalized)
}

/// Sync workspace: check all entries against disk, remove stale ones, rescan root dirs for new files.
/// Returns the updated tree and whether stale entries were found.
#[tauri::command]
pub async fn sync_workspace(project_id: String) -> Result<SyncResult, String> {
    let rows = read_rows(&project_id)?;

    // 0. Remove duplicate entries (same native_path) — keeps the row with the lowest sort_order
    {
        let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new(); // native_path → id to keep
        let mut dup_ids: Vec<String> = Vec::new();
        for row in &rows {
            let key = row.native_path.trim_end_matches('\\').trim_end_matches('/').to_lowercase();
            if let Some(existing_id) = seen.get(&key) {
                // Keep the one with lower sort_order
                if row.sort_order < rows.iter().find(|r| &r.id == existing_id).map(|r| r.sort_order).unwrap_or(i32::MAX) {
                    dup_ids.push(existing_id.clone());
                    seen.insert(key, row.id.clone());
                } else {
                    dup_ids.push(row.id.clone());
                }
            } else {
                seen.insert(key, row.id.clone());
            }
        }
        if !dup_ids.is_empty() {
            let conn = open_connection()?;
            for id in &dup_ids {
                conn.execute("DELETE FROM workspace_entries WHERE id = ?1", params![id])
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    // 1. Remove entries whose native_path no longer exists on disk
    let mut stale_found = false;
    let mut stale_ids: Vec<String> = Vec::new();
    for row in &rows {
        let path = Path::new(&row.native_path);
        if !path.exists() {
            stale_ids.push(row.id.clone());
            stale_found = true;
        }
    }
    if !stale_ids.is_empty() {
        let conn = open_connection()?;
        for id in &stale_ids {
            conn.execute("DELETE FROM workspace_entries WHERE id = ?1", params![id])
                .map_err(|e| e.to_string())?;
        }
    }

    // 2. Re-read rows after cleanup
    let clean_rows = read_rows(&project_id)?;

    // 3. For root-level directories, scan for new files/folders on disk that aren't in DB
    let root_entries: Vec<&WsRow> = clean_rows.iter().filter(|r| r.parent_id.is_none() && r.is_dir).collect();
    for root in &root_entries {
        let root_path = Path::new(&root.native_path);
        if !root_path.is_dir() { continue; }
        sync_scan_dir(&project_id, root_path, Some(&root.id), &clean_rows)?;
    }

    // 4. Return updated tree
    let final_rows = read_rows(&project_id)?;
    Ok(SyncResult { tree: build_tree(&final_rows), stale_found })
}

/// Recursively scan a directory and add new entries to workspace DB.
fn sync_scan_dir(
    project_id: &str,
    dir: &Path,
    parent_id: Option<&str>,
    existing_rows: &[WsRow],
) -> Result<(), String> {
    if let Ok(entries) = fs::read_dir(dir) {
        let mut sort_order = count_children(project_id, parent_id) as i32;
        for entry in entries.flatten() {
            let path = entry.path();
            let abs = path.to_string_lossy().to_string();
            // Check if already in DB
            let already_exists = existing_rows.iter().any(|r| r.native_path == abs);
            if already_exists {
                // If it's a directory, recurse into it to find new children
                if path.is_dir() {
                    // Find the existing row's ID
                    if let Some(row) = existing_rows.iter().find(|r| r.native_path == abs) {
                        sync_scan_dir(project_id, &path, Some(&row.id), existing_rows)?;
                    }
                }
                continue;
            }
            // New entry — add it
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if path.is_dir() {
                add_folder_recursive(project_id, &path, parent_id, &mut sort_order)?;
            } else {
                let meta = fs::metadata(&path).ok();
                let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                let mime = files::mime_from_ext_public(&path);
                insert_row(project_id, &name, &abs, parent_id, false, size, &mime, sort_order)?;
                sort_order += 1;
            }
        }
    }
    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────

fn find_id_by_path(project_id: &str, native_path: &str) -> Result<Option<String>, String> {
    let rows = read_rows(project_id)?;
    let normalized = normalize_path(native_path);
    Ok(rows
        .iter()
        .find(|r| r.native_path == normalized)
        .map(|r| r.id.clone()))
}

fn count_children(project_id: &str, parent_id: Option<&str>) -> usize {
    let rows = match read_rows(project_id) {
        Ok(r) => r,
        Err(_) => return 0,
    };
    rows.iter().filter(|r| r.parent_id.as_deref() == parent_id).count()
}

/// Rename the managed (auto-created) project folder on disk and update all workspace entries.
/// Only works when old_path ends with the old_name under a managed parent (e.g. Desktop).
/// Returns the new path on success.
#[tauri::command]
pub async fn rename_managed_folder(
    project_id: String,
    old_path: String,
    new_name: String,
) -> Result<String, String> {
    let normalized = normalize_path(&old_path);
    let path = PathBuf::from(&normalized);

    if !path.exists() {
        return Err(format!("Folder '{}' not found", normalized));
    }
    if !path.is_dir() {
        return Err(format!("'{}' is not a directory", normalized));
    }

    let parent = path
        .parent()
        .ok_or_else(|| "Cannot determine parent directory".to_string())?;
    let new_path = parent.join(&new_name);

    if new_path.exists() {
        return Err(format!("A folder '{}' already exists at: {}", new_name, new_path.display()));
    }

    // Rename on disk
    fs::rename(&path, &new_path).map_err(|e| e.to_string())?;
    let old_abs = normalized;
    let new_abs = new_path.to_string_lossy().to_string();

    // Update DB entries — rename path prefix for all descendants
    let conn = open_connection()?;
    let old_prefix = old_abs.trim_end_matches('\\').trim_end_matches('/');
    let new_prefix = new_abs.trim_end_matches('\\').trim_end_matches('/');
    let offset = (old_prefix.len() + 1) as i32;

    // Update entry itself
    let _old_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    conn.execute(
        "UPDATE workspace_entries SET name = ?1, native_path = ?2 WHERE project_id = ?3 AND native_path = ?4",
        params![new_name, new_abs, project_id, old_abs],
    )
    .map_err(|e| e.to_string())?;

    // Update children (backslash paths)
    conn.execute(
        "UPDATE workspace_entries SET native_path = ?1 || SUBSTR(native_path, ?2) WHERE project_id = ?3 AND native_path LIKE ?4",
        params![new_prefix, offset, project_id, format!("{}\\%", old_prefix)],
    )
    .map_err(|e| e.to_string())?;

    // Update children (forward-slash paths)
    conn.execute(
        "UPDATE workspace_entries SET native_path = ?1 || SUBSTR(native_path, ?2) WHERE project_id = ?3 AND native_path LIKE ?4",
        params![new_prefix, offset, project_id, format!("{}/%", old_prefix)],
    )
    .map_err(|e| e.to_string())?;

    Ok(new_abs)
}

/// Check all projects for stale folders — returns list of project_ids whose ALL workspace root dirs
/// no longer exist on disk.
#[tauri::command]
pub async fn check_stale_projects() -> Result<Vec<String>, String> {
    let conn = open_connection()?;
    let mut stmt = conn
        .prepare("SELECT DISTINCT project_id FROM workspace_entries")
        .map_err(|e| e.to_string())?;
    let project_ids: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    let mut stale_projects: Vec<String> = Vec::new();
    for pid in &project_ids {
        let rows = read_rows(pid)?;
        if rows.is_empty() { continue; }
        let all_stale = rows.iter().all(|r| !Path::new(&r.native_path).exists());
        if all_stale {
            stale_projects.push(pid.clone());
        }
    }
    Ok(stale_projects)
}

/// Delete all workspace entries for a project (cleanup when project is deleted).
#[tauri::command]
pub async fn delete_workspace_for_project(project_id: String) -> Result<(), String> {
    let conn = open_connection()?;
    conn.execute("DELETE FROM workspace_entries WHERE project_id = ?1", params![project_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
