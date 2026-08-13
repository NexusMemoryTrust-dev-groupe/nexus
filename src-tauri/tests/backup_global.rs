//! Global backup/restore wrappers against a redirected database (coverage
//! 8.1): `default_backup_dir` / `create_backup` / `list_history` /
//! `delete_backup` / `restore_backup` read `db::db_path()`, which resolves
//! `LOCALAPPDATA` **at call time**.
//!
//! This test lives in its own binary on purpose: redirecting the process-global
//! `LOCALAPPDATA` inside the lib test process would race every other test that
//! touches the global database (tokenizer's `configured_model`, MCP tools such
//! as `nexus_create_memory`/`nexus_agent_access_check`).

use std::path::Path;

use nexus::core::backup::{
    create_backup, default_backup_dir, delete_backup, list_history, restore_backup,
};
use nexus::db;
use nexus::storage::sqlite::schema;

#[test]
fn global_wrappers_follow_the_isolated_database() {
    let tmp = std::env::temp_dir().join(format!("nexus-backup-global-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let prev = std::env::var("LOCALAPPDATA").ok();
    // Rust 2024: set_var/remove_var are unsafe.
    unsafe { std::env::set_var("LOCALAPPDATA", &tmp) };

    {
        let conn = db::open_connection().expect("open isolated db");
        schema::apply_migrations(&conn).expect("migrate isolated db");
        conn.execute(
            "INSERT INTO memory_records \
             (id, title, summary, content, created_at, updated_at, author, source) \
             VALUES ('g1', 'global', '', 'wrapper content', \
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'test', 'test')",
            [],
        )
        .expect("insert memory");
    }

    let dir = default_backup_dir();
    assert!(
        dir.file_name().is_some_and(|n| n == "backups"),
        "unexpected default dir: {}",
        dir.display()
    );

    let info = create_backup(&dir).expect("create backup");
    assert!(Path::new(&info.path).exists());

    let history = list_history().expect("read history");
    assert!(!history.is_empty());

    let missing = format!("{}.missing", info.path);
    let err = delete_backup(Path::new(&missing)).expect_err("missing file must error");
    assert!(err.to_string().contains("does not exist"), "got: {err}");

    let report = restore_backup(Path::new(&info.path)).expect("restore backup");
    assert!(report.pre_restore_backup.ends_with(".nexusbackup"));

    delete_backup(Path::new(&info.path)).expect("delete backup");
    assert!(!Path::new(&info.path).exists());

    let _ = std::fs::remove_dir_all(&tmp);
    match prev {
        Some(v) => unsafe { std::env::set_var("LOCALAPPDATA", v) },
        None => unsafe { std::env::remove_var("LOCALAPPDATA") },
    }
}
