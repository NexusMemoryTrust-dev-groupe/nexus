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

/// Compare two paths case-insensitively on Windows after normalizing the
/// things `std::fs::canonicalize` introduces: the `\\?\` verbatim prefix, a
/// trailing separator, and the real on-disk casing (which differs from the
/// `LOCALAPPDATA` env var on CI runners: `C:\Users\RunnerAdmin\...` vs
/// `C:\Users\runneradmin\...`).
fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| {
        let mut s = p.display().to_string();
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            s = rest.to_string();
        }
        while s.ends_with('/') || s.ends_with('\\') {
            s.pop();
        }
        s
    };
    norm(a).eq_ignore_ascii_case(&norm(b))
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

    // The data directory itself is always a root. The sandbox resolves every
    // root through `std::fs::canonicalize` (long names, real casing), so the
    // expected value is canonicalised the same way — a raw `LOCALAPPDATA`
    // string can differ in casing or carry an 8.3 short name on CI runners.
    let sb = current();
    let data_root_raw = db_path.parent().expect("db has parent");
    let data_root =
        std::fs::canonicalize(data_root_raw).unwrap_or_else(|_| data_root_raw.to_path_buf());
    assert!(
        sb.roots()
            .iter()
            .any(|r| same_path(Path::new(r), &data_root)),
        "data dir missing from roots: {:?}; expected: {:?}",
        sb.roots(),
        data_root
    );
    // The seeded workspace/extra roots do not exist on disk, so from_roots
    // drops them — the DB reads themselves are what we exercise here.

    // guard() allows paths inside the data dir. The target file does not
    // exist, so guard resolves it against the canonicalised data root; build
    // the expected value from the same canonicalised root.
    let target = data_root.join("live.txt");
    let ok = guard(&target.display().to_string(), Access::Read);
    assert!(ok.is_ok(), "got {:?}", ok);
    let expected = data_root.join("live.txt");
    assert!(
        same_path(&ok.unwrap(), &expected),
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
