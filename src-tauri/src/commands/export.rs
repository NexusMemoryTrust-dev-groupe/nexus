use crate::core::export::{ImportReport, ProjectExport, export_project, import_project};
use crate::core::result::AppError;

/// Convert a core error into the IPC string, logging the full taxonomy first so
/// the structured log carries error_code/severity/component/recoverable.
fn err_msg(e: AppError) -> String {
    crate::infra::log_error(&e);
    e.to_string()
}

/// Tauri command: export the whole project to the portable, versioned JSON
/// format (plan 9.2). The result is a self-describing `ProjectExport` that can
/// be saved as a `.nexus.json` artifact and imported elsewhere.
#[tauri::command]
pub async fn project_export() -> std::result::Result<ProjectExport, String> {
    export_project().await.map_err(err_msg)
}

/// Tauri command: import a project export into the live database. Preserves
/// every ID and timestamp; intended for a fresh/empty database.
#[tauri::command]
pub async fn project_import(json: String) -> std::result::Result<ImportReport, String> {
    let export = ProjectExport::from_json(&json).map_err(err_msg)?;
    import_project(&export).await.map_err(err_msg)
}
