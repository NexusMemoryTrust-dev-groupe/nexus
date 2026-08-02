mod ai;
mod core;
mod commands;
mod db;
mod infra;
mod storage;

use std::sync::Arc;
use rusqlite::Connection;
use tauri::Manager;

use crate::core::event_bus::EventBus;
use crate::core::event_bus::domain_event_bus::InMemoryEventBus;
use crate::storage::sqlite::SqliteVersioningRepository;

fn main() {
    // MCP mode: if --mcp flag is passed, run the MCP stdio server instead of Tauri GUI
    if std::env::args().any(|a| a == "--mcp") {
        eprintln!("[nexus] Starting MCP server mode (stdio)");
        // Initialize DB
        let db_path = crate::db::db_path();
        {
            let conn = Connection::open(&db_path).expect("Failed to open DB connection for migrations");
            storage::sqlite::schema::apply_migrations(&conn).expect("Failed to apply migrations");
        }
        // Run MCP server on stdio (blocking)
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(crate::ai::mcp_server::run_stdio());
        return;
    }

    let _ = infra::init_logging();
    tracing::info!("Starting Nexus Memory Trust");

    // Initialize SQLite database in user's home directory (not src-tauri/) to avoid
    // Tauri file watcher infinite rebuild loop from WAL files
    let db_path = crate::db::db_path();
    tracing::info!("Database path: {:?}", db_path);

    // Apply migrations via a single connection
    {
        let conn = Connection::open(&db_path).expect("Failed to open DB connection for migrations");
        storage::sqlite::schema::apply_migrations(&conn).expect("Failed to apply migrations");
        drop(conn);
    }

    // M28: Versioning repository — only one used directly in main (for event listener)
    let versioning_conn = Connection::open(&db_path).expect("Failed to open versioning DB connection");
    let versioning_repo = SqliteVersioningRepository::new(versioning_conn).expect("Failed to create versioning repository");

    // Event bus — shared across all modules
    let event_bus = Arc::new(InMemoryEventBus::default());

    // M2→M28 Integration: commit service for versioning listener
    let commit_service: Arc<dyn crate::core::versioning::CommitService> = Arc::new(versioning_repo);

    tracing::info!("All services initialized successfully");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(event_bus)
        .setup(move |app| {
            // Subscribe versioning listener to memory events inside Tauri runtime
            let handler = crate::core::versioning::create_versioning_handler(commit_service);
            let bus = app.state::<Arc<InMemoryEventBus>>().inner().clone();
            tauri::async_runtime::spawn(async move {
                bus.subscribe(handler).await;
            });

            // Bring the semantic index up to date in the background.
            //
            // Fingerprints used to be written only when something called the MCP
            // tool by hand, so on a real database the semantic index was empty and
            // similarity search silently returned nothing. This walks whatever is
            // still unindexed on a background thread; it is a no-op once the index
            // has caught up, and it never blocks the window from opening.
            crate::core::context::indexer::spawn_backfill();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::memory::get_memories,
            commands::memory::get_memory,
            commands::memory::create_memory,
            commands::memory::search_memories,
            commands::memory::get_project_memories,
            commands::memory::create_project_memory,
            commands::memory::update_memory,
            commands::memory::delete_memory,
            commands::graph::get_graph,
            commands::graph::get_entity,
            commands::graph::create_entity,
            commands::graph::get_projects,
            commands::graph::get_project_entities,
            commands::graph::link_entity_to_project,
            commands::graph::delete_relationship,
            commands::graph::update_entity,
            commands::graph::get_entity_metadata,
            commands::graph::delete_entity,
            commands::context::build_context,
            commands::context::build_context_for_entity,
            commands::context::export_context,
            commands::config::get_config,
            commands::config::get_all_config,
            commands::config::set_config,
            commands::config::delete_config,
            commands::config::get_db_stats,
            commands::ai::ai_health_check,
            commands::ai::ai_chat,
            commands::ai::ai_chat_stream,
            commands::ai::ai_list_models,
            commands::files::scan_folder,
            commands::files::read_file,
            commands::files::write_file,
            commands::files::delete_file,
            commands::files::delete_folder,
            commands::files::rename_file,
            commands::files::create_file,
            commands::files::create_folder,
            commands::files::pick_files,
            commands::files::pick_folder,
            commands::files::move_entry,
            commands::files::get_desktop_dir,
            commands::files::path_exists,
            commands::files::list_dir_names,
            commands::workspace::get_workspace_tree,
            commands::workspace::add_to_workspace,
            commands::workspace::create_workspace_entry,
            commands::workspace::rename_workspace_entry,
            commands::workspace::delete_workspace_entry,
            commands::workspace::move_workspace_entry,
            commands::workspace::remove_from_workspace,
            commands::workspace::sync_workspace,
            commands::workspace::rename_managed_folder,
            commands::workspace::check_stale_projects,
            commands::workspace::delete_workspace_for_project,
            commands::copilot::copilot_execute,
            commands::copilot::copilot_list_commands,
            commands::savings::get_savings_stats,
            commands::savings::record_savings_event,
            commands::savings::get_savings_report,
            commands::savings::get_model_savings,
            commands::setup::setup_status,
            commands::setup::setup_needed,
            commands::setup::install_opencode,
            commands::setup::register_mcp,
            commands::setup::save_api_key,
            commands::setup::select_model,
            commands::setup::complete_setup,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
