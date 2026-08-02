use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Normalize a path string that may have been stored with debug format wrappers.
/// Strips `[Path("...")]`, `Path("...")`, and surrounding quotes/brackets.
pub fn normalize_path(p: &str) -> String {
    let s = p.trim();
    // Handle: [Path("C:\...")] or Path("C:\...")
    if let Some(inner) = s.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        return normalize_path(inner);
    }
    if let Some(inner) = s
        .strip_prefix("Path(\"")
        .and_then(|r| r.strip_suffix("\")"))
    {
        return inner.replace("\\\\", "\\").replace("\\\"", "\"");
    }
    // Handle: "C:\..." (quoted)
    if let Some(inner) = s.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        return inner.to_string();
    }
    s.to_string()
}

/// A file or folder entry in the project's virtual file tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub mime_type: String,
    pub children: Option<Vec<FileEntry>>,
}

/// Result of reading a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileContent {
    pub name: String,
    pub path: String,
    pub content: String,
    pub size_bytes: u64,
    pub mime_type: String,
    pub is_editable: bool,
}

/// Scan a directory recursively into a FileEntry tree.
/// Returns ABSOLUTE paths so frontend can pass them directly to read_file/delete_file/rename_file.
fn scan_dir(dir: &Path) -> Result<FileEntry, String> {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    // Always use absolute path
    let abs_path = dir.to_string_lossy().to_string();

    let mut children: Vec<FileEntry> = Vec::new();
    let mut total_size: u64 = 0;

    if dir.is_dir() {
        let entries = fs::read_dir(dir).map_err(|e| e.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                children.push(scan_dir(&path)?);
            } else {
                let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
                let child_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let child_abs = path.to_string_lossy().to_string();
                total_size += meta.len();
                children.push(FileEntry {
                    name: child_name,
                    path: child_abs,
                    is_dir: false,
                    size_bytes: meta.len(),
                    mime_type: mime_from_ext(&path),
                    children: None,
                });
            }
        }
        // Sort: dirs first, then by name
        children.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
    }

    Ok(FileEntry {
        name,
        path: abs_path,
        is_dir: dir.is_dir(),
        size_bytes: total_size,
        mime_type: "inode/directory".to_string(),
        children: Some(children),
    })
}

/// Get mime type from file extension.
pub fn mime_from_ext(path: &Path) -> String {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "md" | "markdown" => "text/markdown".to_string(),
        "txt" => "text/plain".to_string(),
        "json" => "application/json".to_string(),
        "yaml" | "yml" => "text/yaml".to_string(),
        "toml" => "text/toml".to_string(),
        "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "go" | "java" | "c" | "cpp" | "h" => {
            "text/x-source".to_string()
        }
        "html" | "htm" => "text/html".to_string(),
        "css" => "text/css".to_string(),
        "svg" => "image/svg+xml".to_string(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" => "image/*".to_string(),
        "pdf" => "application/pdf".to_string(),
        "zip" | "tar" | "gz" => "application/archive".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}

/// Check if a file is editable (text-based).
pub fn is_editable(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str(),
        "md" | "markdown"
            | "txt"
            | "json"
            | "yaml"
            | "yml"
            | "toml"
            | "rs"
            | "py"
            | "js"
            | "ts"
            | "tsx"
            | "jsx"
            | "go"
            | "java"
            | "c"
            | "cpp"
            | "h"
            | "html"
            | "htm"
            | "css"
            | "svg"
            | "sql"
            | "sh"
            | "bat"
            | "ps1"
            | "xml"
            | "csv"
            | "log"
            | "gitignore"
            | "dockerignore"
            | "env"
            | "cfg"
            | "conf"
            | "ini"
    )
}

// ── Tauri Commands ─────────────────────────────────────────────────

/// Scan a directory and return its file tree with ABSOLUTE paths.
#[tauri::command]
pub async fn scan_folder(folder_path: String) -> Result<FileEntry, String> {
    let path = PathBuf::from(&folder_path);
    if !path.is_dir() {
        return Err(format!("'{}' is not a directory", folder_path));
    }
    scan_dir(&path)
}

/// Read a file's content.
#[tauri::command]
pub async fn read_file(file_path: String) -> Result<FileContent, String> {
    let normalized = normalize_path(&file_path);
    let path = PathBuf::from(&normalized);
    if !path.exists() {
        return Err(format!("File '{}' not found", file_path));
    }
    let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let content = if is_editable(&path) {
        fs::read_to_string(&path).map_err(|e| e.to_string())?
    } else {
        String::new()
    };

    Ok(FileContent {
        name,
        path: normalized,
        content,
        size_bytes: meta.len(),
        mime_type: mime_from_ext(&path),
        is_editable: is_editable(&path),
    })
}

/// Write content to a file (creates if not exists, overwrites if exists).
#[tauri::command]
pub async fn write_file(file_path: String, content: String) -> Result<(), String> {
    let normalized = normalize_path(&file_path);
    let path = PathBuf::from(&normalized);
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, content).map_err(|e| e.to_string())
}

/// Get the user's Desktop path.
#[tauri::command]
pub async fn get_desktop_dir() -> Result<String, String> {
    let desktop = dirs::desktop_dir().ok_or("Cannot determine desktop directory")?;
    Ok(desktop.to_string_lossy().to_string())
}

/// Delete a file or empty directory.
#[tauri::command]
pub async fn delete_file(file_path: String) -> Result<(), String> {
    let normalized = normalize_path(&file_path);
    let path = PathBuf::from(&normalized);
    if !path.exists() {
        return Err(format!("'{}' not found", file_path));
    }
    if path.is_dir() {
        fs::remove_dir(&path).map_err(|e| e.to_string())
    } else {
        fs::remove_file(&path).map_err(|e| e.to_string())
    }
}

/// Rename a file or directory. Returns the new ABSOLUTE path.
#[tauri::command]
pub async fn rename_file(old_path: String, new_name: String) -> Result<String, String> {
    let normalized = normalize_path(&old_path);
    let path = PathBuf::from(&normalized);
    if !path.exists() {
        return Err(format!("'{}' not found", old_path));
    }
    let parent = path
        .parent()
        .ok_or_else(|| "Cannot determine parent directory".to_string())?;
    let new_path = parent.join(&new_name);
    fs::rename(&path, &new_path).map_err(|e| e.to_string())?;
    // Return absolute path so frontend can use it directly
    Ok(new_path.to_string_lossy().to_string())
}

/// Create a new file with optional initial content.
#[tauri::command]
pub async fn create_file(file_path: String, content: Option<String>) -> Result<(), String> {
    let normalized = normalize_path(&file_path);
    let path = PathBuf::from(&normalized);
    if path.exists() {
        return Err(format!("File '{}' already exists", file_path));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, content.unwrap_or_default()).map_err(|e| e.to_string())
}

/// Create a new directory.
#[tauri::command]
pub async fn create_folder(folder_path: String) -> Result<(), String> {
    let normalized = normalize_path(&folder_path);
    let path = PathBuf::from(&normalized);
    if path.exists() {
        return Err(format!("Folder '{}' already exists", folder_path));
    }
    fs::create_dir_all(&path).map_err(|e| e.to_string())
}

/// Delete a folder and all its contents recursively.
#[tauri::command]
pub async fn delete_folder(folder_path: String) -> Result<(), String> {
    let normalized = normalize_path(&folder_path);
    let path = PathBuf::from(&normalized);
    if !path.exists() {
        return Err(format!("'{}' not found", folder_path));
    }
    if !path.is_dir() {
        return Err(format!("'{}' is not a folder", folder_path));
    }
    fs::remove_dir_all(&path).map_err(|e| e.to_string())
}

/// Open native file dialog to pick files. Returns list of selected paths.
#[tauri::command]
pub async fn pick_files(
    app: tauri::AppHandle,
    title: Option<String>,
    filters: Option<Vec<String>>,
) -> Result<Vec<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let mut builder = app.dialog().file();
    if let Some(t) = title {
        builder = builder.set_title(&t);
    }
    if let Some(f) = filters {
        for filter_str in f {
            let parts: Vec<&str> = filter_str.split('|').collect();
            let name = *parts.first().unwrap_or(&"Files");
            let exts: Vec<&str> = parts
                .get(1..)
                .unwrap_or(&[])
                .iter()
                .flat_map(|s| s.split(','))
                .map(|s| s.trim())
                .collect();
            builder = builder.add_filter(name, &exts);
        }
    }

    let file_paths = builder.blocking_pick_files().unwrap_or_default();
    let paths: Vec<String> = file_paths.into_iter().map(|fp| fp.to_string()).collect();

    Ok(paths)
}

/// Open native file dialog to pick a folder. Returns the selected folder path.
#[tauri::command]
pub async fn pick_folder(
    app: tauri::AppHandle,
    title: Option<String>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let mut builder = app.dialog().file();
    if let Some(t) = title {
        builder = builder.set_title(&t);
    }

    let folder_path = builder.blocking_pick_folder();
    Ok(folder_path.map(|fp| fp.to_string()))
}

/// Move a file or folder to a new parent directory. Returns the new ABSOLUTE path.
#[tauri::command]
pub async fn move_entry(source_path: String, dest_dir: String) -> Result<String, String> {
    let src = PathBuf::from(normalize_path(&source_path));
    let dst_dir = PathBuf::from(normalize_path(&dest_dir));

    if !src.exists() {
        return Err(format!("Source does not exist: {}", src.display()));
    }
    if !dst_dir.is_dir() {
        return Err(format!(
            "Destination is not a directory: {}",
            dst_dir.display()
        ));
    }

    let file_name = src.file_name().ok_or("Cannot get file name from source")?;
    let dest = dst_dir.join(file_name);

    if dest.exists() {
        return Err(format!(
            "A file or folder already exists at: {}",
            dest.display()
        ));
    }

    fs::rename(&src, &dest).map_err(|e| format!("Move failed: {}", e))?;
    // Return absolute path so frontend can use it directly
    Ok(dest.to_string_lossy().to_string())
}

/// Public wrapper for mime_from_ext — used by workspace.rs.
pub fn mime_from_ext_public(path: &Path) -> String {
    mime_from_ext(path)
}

/// Check whether a path exists on disk (file or directory).
#[tauri::command]
pub fn path_exists(path: String) -> Result<bool, String> {
    let p = Path::new(&path);
    Ok(p.exists())
}

/// List immediate children names inside a directory (for name collision checks).
#[tauri::command]
pub fn list_dir_names(dir_path: String) -> Result<Vec<String>, String> {
    let p = Path::new(&dir_path);
    if !p.is_dir() {
        return Ok(vec![]);
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(p).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if let Some(name) = entry.file_name().to_str() {
            names.push(name.to_string());
        }
    }
    Ok(names)
}
