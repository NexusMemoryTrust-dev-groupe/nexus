//! Feature flags — enable → measure → rollback (plan 7.6).
//!
//! Flags let risky or experimental behaviour ship behind a switch. The
//! workflow the plan demands is:
//!
//!   1. **enable**   — `set_enabled("semantic_conflict_v2", true)`;
//!   2. **measure**  — run the SLA / conflict / long-horizon benches and compare
//!      NEXUS_METRIC output against `benchmarks/baseline.json`;
//!   3. **rollback** — `set_enabled(..., false)` restores the old path with no
//!      code change and no data migration.
//!
//! Flags are persisted in the same `configuration_kv` table the settings page
//! uses, under a `feature.` key prefix, so they survive restarts, show up in
//! `get_all_config`, and can be flipped from the UI/MCP exactly like any other
//! setting. Unknown keys are rejected on write (a typo must not silently
//! create a no-op flag) and read as their registry default.
//!
//! Reads are cheap single-row SELECTs and **fail open**: if the DB cannot be
//! read we behave as if the flag were at its default, so a storage hiccup can
//! never brick conflict detection or retrieval.

use crate::core::result::Result;

/// Key prefix in the config store.
const KEY_PREFIX: &str = "feature.";

/// Registry entry for one known flag.
pub struct FeatureFlag {
    /// Short key, e.g. `"semantic_conflict_v2"` (stored as `feature.<key>`).
    pub key: &'static str,
    /// Behaviour when the flag is not explicitly set.
    pub default: bool,
    /// Human-readable explanation, surfaced in `list_flags`.
    pub description: &'static str,
}

/// `semantic_conflict_v2` — use the embedding channel in conflict detection.
///
/// ON: `detect_and_mark_conflicts` consults both the lexical overlap and the
/// semantic cosine (`is_conflicting_pair(..., Some(hit.semantic))`) so
/// paraphrases with no shared vocabulary are still flagged.
/// OFF: lexical-only verdicts (`None`) — the pre-v2 path. Slower recall on
/// paraphrase conflicts, cheaper per insert.
pub const FEATURE_SEMANTIC_CONFLICT_V2: &str = "semantic_conflict_v2";

/// `hybrid_retrieval` — combine embeddings with lexical/path signals in
/// memory similarity search.
///
/// ON: `find_similar_memories` merges the semantic index with Dice text
/// overlap so fresh inserts (not yet embedded) still surface.
/// OFF: lexical overlap only — the semantic index is not consulted, so fresh
/// inserts are invisible to similarity search (the pre-hybrid path).
pub const FEATURE_HYBRID_RETRIEVAL: &str = "hybrid_retrieval";

/// All flags the system knows about. Writes outside this list are rejected.
pub const KNOWN_FLAGS: &[FeatureFlag] = &[
    FeatureFlag {
        key: FEATURE_SEMANTIC_CONFLICT_V2,
        default: true,
        description: "Semantic (embedding) channel in conflict detection",
    },
    FeatureFlag {
        key: FEATURE_HYBRID_RETRIEVAL,
        default: true,
        description: "Hybrid lexical+semantic memory similarity search",
    },
];

fn registry(key: &str) -> Option<&'static FeatureFlag> {
    KNOWN_FLAGS.iter().find(|f| f.key == key)
}

fn open() -> Result<rusqlite::Connection> {
    let conn = crate::db::open_connection().map_err(crate::core::result::AppError::Database)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS configuration_kv (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        );",
    )
    .map_err(|e| crate::core::result::AppError::Database(e.to_string()))?;
    Ok(conn)
}

/// Pure core: whether a known flag is enabled *on the given connection*.
/// Unknown keys and unreadable values fall back to the registry default.
/// Exposed for tests and embedding; production code should call [`is_enabled`],
/// which opens the global config store.
pub fn is_enabled_on(conn: &rusqlite::Connection, key: &str) -> bool {
    let Some(flag) = registry(key) else {
        return false;
    };
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM configuration_kv WHERE key = ?1",
            [format!("{KEY_PREFIX}{key}")],
            |row| row.get(0),
        )
        .ok();
    match value.as_deref() {
        Some("true") => true,
        Some("false") => false,
        _ => flag.default,
    }
}

/// Whether a known feature flag is enabled. Unknown keys and unreadable DBs
/// fall back to the registry default (fail-open).
pub fn is_enabled(key: &str) -> bool {
    let Ok(conn) = open() else {
        return registry(key).map(|f| f.default).unwrap_or(false);
    };
    is_enabled_on(&conn, key)
}

/// Pure core: set a known flag on the given connection. Unknown keys are
/// rejected so a typo cannot create a silent no-op toggle.
pub fn set_enabled_on(conn: &rusqlite::Connection, key: &str, enabled: bool) -> Result<()> {
    if registry(key).is_none() {
        return Err(crate::core::result::AppError::Configuration(format!(
            "unknown feature flag: {key}"
        )));
    }
    let value = if enabled { "true" } else { "false" };
    conn.execute(
        "INSERT OR REPLACE INTO configuration_kv (key, value) VALUES (?1, ?2)",
        [format!("{KEY_PREFIX}{key}"), value.to_string()],
    )
    .map_err(|e| crate::core::result::AppError::Database(e.to_string()))?;
    Ok(())
}

/// Set a known flag on the global config store. Unknown keys are rejected.
pub fn set_enabled(key: &str, enabled: bool) -> Result<()> {
    let conn = open()?;
    set_enabled_on(&conn, key, enabled)
}

/// Effective state of one flag for the settings/ops surface.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FeatureFlagStatus {
    pub key: &'static str,
    pub enabled: bool,
    pub default: bool,
    pub description: &'static str,
}

/// Effective state of every known flag.
pub fn list_flags() -> Vec<FeatureFlagStatus> {
    KNOWN_FLAGS
        .iter()
        .map(|f| FeatureFlagStatus {
            key: f.key,
            enabled: is_enabled(f.key),
            default: f.default,
            description: f.description,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE configuration_kv (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn unknown_key_defaults_to_disabled() {
        let conn = repo();
        assert!(!is_enabled_on(&conn, "no_such_flag"));
    }

    #[test]
    fn registry_has_both_planned_flags() {
        assert!(registry(FEATURE_SEMANTIC_CONFLICT_V2).is_some());
        assert!(registry(FEATURE_HYBRID_RETRIEVAL).is_some());
    }

    #[test]
    fn registry_defaults_match_documented_behaviour() {
        let conn = repo();
        // Both flags default ON: current production behaviour is the hybrid,
        // semantic-aware path. Flipping them OFF is the rollback.
        assert!(is_enabled_on(&conn, FEATURE_SEMANTIC_CONFLICT_V2));
        assert!(is_enabled_on(&conn, FEATURE_HYBRID_RETRIEVAL));
    }

    #[test]
    fn set_enabled_rejects_unknown_keys() {
        let conn = repo();
        let err = set_enabled_on(&conn, "typo_flag", true).unwrap_err();
        assert!(err.to_string().contains("unknown feature flag"));
    }

    #[test]
    fn set_enabled_round_trip_on_in_memory_table() {
        let conn = repo();
        assert!(is_enabled_on(&conn, FEATURE_SEMANTIC_CONFLICT_V2));
        set_enabled_on(&conn, FEATURE_SEMANTIC_CONFLICT_V2, false).unwrap();
        assert!(!is_enabled_on(&conn, FEATURE_SEMANTIC_CONFLICT_V2));
        set_enabled_on(&conn, FEATURE_SEMANTIC_CONFLICT_V2, true).unwrap();
        assert!(is_enabled_on(&conn, FEATURE_SEMANTIC_CONFLICT_V2));
    }

    #[test]
    fn flags_persist_across_connections() {
        // A real flag write survives reopening the store: write on one
        // connection, read on another.
        let dir = std::env::temp_dir().join(format!("nexus-ff-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("flags.db");
        let _ = std::fs::remove_file(&db_path);

        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE configuration_kv (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);",
            )
            .unwrap();
            set_enabled_on(&conn, FEATURE_HYBRID_RETRIEVAL, false).unwrap();
        }
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            assert!(!is_enabled_on(&conn, FEATURE_HYBRID_RETRIEVAL));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_value_falls_back_to_default() {
        let conn = repo();
        conn.execute(
            "INSERT OR REPLACE INTO configuration_kv (key, value) VALUES (?1, ?2)",
            [
                format!("{KEY_PREFIX}{}", FEATURE_SEMANTIC_CONFLICT_V2),
                "maybe".to_string(),
            ],
        )
        .unwrap();
        assert!(is_enabled_on(&conn, FEATURE_SEMANTIC_CONFLICT_V2));
    }
}
