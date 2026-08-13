//! Live filesystem-sandbox policy driven by the on-disk database (coverage
//! 8.1): `collect_roots` / `current` / `guard` read `db::db_path()`, which
//! resolves `LOCALAPPDATA` **at call time**.
//!
//! This test lives in its own binary on purpose: redirecting the process-global
//! `LOCALAPPDATA` inside the lib test process would race every other test that
//! touches the global database (tokenizer's `configured_model`, MCP tools).

use std::path::Path;

use nexus::core::sandbox::{Access, EXTRA_ROOTS_KEY, current, guard};
use nexus::db;

/// Compare two paths per component, case-insensitively on Windows, mirroring
/// the sandbox's own `components_match`. `std::fs::canonicalize` resolves the
/// real on-disk casing of the data dir, while `db::db_path()` builds it from
/// the `LOCALAPPDATA` env var — the two differ in case on CI runners
/// (`C:\Users\RunnerAdmin\...` vs `C:\Users\runneradmin\...`).
fn same_path(a: &Path, b: &Path) -> bool {
    a.components().count() == b.components().count()
        && a.components().zip(b.components()).all(|(x, y)| {
            x.as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(&y.as_os_str().to_string_lossy())
        })
}

#[test]
fn live_policy_collects_data_dir_workspace_and_extra_roots() {
    let tmp = std::env::temp_dir().join(format!("nexus-sandbox-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let prev = std::env::var("LOCALAPPDATA").ok();
    // Rust 2024: set_var/remove_var are unsafe.
    unsafe { std::env::set_var("LOCALAPPDATA", &tmp) };

    // Pre-create the tables `collect_roots` reads (the app's migrations are
    // not run here) and seed one row per source.
    let db_path = db::db_path();
    {
        let conn = db::open_connection().expect("open temp db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS workspace_entries \
                 (native_path TEXT, parent_id INTEGER);\
             CREATE TABLE IF NOT EXISTS configuration_kv \
                 (key TEXT PRIMARY KEY, value TEXT);",
        )
        .expect("create tables");
        conn.execute(
            "INSERT INTO workspace_entries (native_path, parent_id) VALUES (?1, NULL)",
            [r"C:\ws\root"],
        )
        .expect("insert workspace");
        conn.execute(
            "INSERT INTO configuration_kv (key, value) VALUES (?1, ?2)",
            [EXTRA_ROOTS_KEY, r"C:\extra1;C:\extra2"],
        )
        .expect("insert extra roots");
    }

    // The data directory itself is always a root.
    let sb = current();
    let data_root = db_path.parent().expect("db has parent").to_path_buf();
    assert!(
        sb.roots()
            .iter()
            .any(|r| same_path(Path::new(r), &data_root)),
        "data dir missing from roots: {:?}",
        sb.roots()
    );
    // The seeded workspace/extra roots do not exist on disk, so from_roots
    // drops them — the DB reads themselves are what we exercise here.

    // guard() allows paths inside the data dir.
    let target = data_root.join("live.txt");
    let ok = guard(&target.display().to_string(), Access::Read);
    assert!(ok.is_ok(), "got {:?}", ok);
    assert!(
        same_path(
            &ok.unwrap(),
            &db::db_path().parent().unwrap().join("live.txt")
        ),
        "resolved path differs from expected data-dir path"
    );

    // guard() rejects paths outside with a user-facing message.
    let err = guard(r"C:\Windows\System32\config\SAM", Access::Delete).unwrap_err();
    assert!(err.contains("delete"), "missing verb: {}", err);
    assert!(
        err.contains("outside your Nexus workspace"),
        "missing reason: {}",
        err
    );

    let _ = std::fs::remove_dir_all(&tmp);
    match prev {
        Some(v) => unsafe { std::env::set_var("LOCALAPPDATA", v) },
        None => unsafe { std::env::remove_var("LOCALAPPDATA") },
    }
}
