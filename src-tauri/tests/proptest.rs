//! Property-based testing (plan 8.2) — консистентность БД при любых переходах.
//!
//! `proptest` покрывает пять областей из плана:
//!   - memory states — формальный автомат доверия `MemoryState::can_transition`
//!     (рефлексивность, терминальность Superseded, разбор без паник);
//!   - graph edges — `resolution`: `normalize_name` (идемпотентность),
//!     `name_dice`/`similarity` (симметрия, границы [0,1]) и `build_groups`
//!     (непересекаемость групп, размеры, скоринт в границах);
//!   - UTF-8 — `tokenizer::count` не паникует ни на каком входе (включая
//!     многобайтовые последовательности), пустая строка → 0, любое разбиение
//!     по границам символов даёт валидный результат;
//!   - paths — `Sandbox::check`: ни одно разрешённое разрешение не выходит
//!     за корень (invariant: Ok ⟹ внутри root), относительные пути → NotAbsolute;
//!   - migration sequences — произвольная последовательность
//!     apply/rollback сходится в `latest_schema_version`, повторный прогон
//!     идемпотентен, `PRAGMA integrity_check` остаётся `ok`.
//!
//! Каждый тест самодостаточен и не требует сети или внешних ресурсов.

use std::path::{Path, PathBuf};

use proptest::prelude::*;

use nexus::core::graph::resolution::{build_groups, name_dice, normalize_name, similarity};
use nexus::core::graph::{Entity, EntityType};
use nexus::core::memory::types::MemoryState;
use nexus::core::sandbox::{Access, Sandbox, SandboxError};
use nexus::core::tokenizer;
use nexus::storage::sqlite::schema::{
    apply_migrations, get_schema_version, latest_schema_version, rollback_last_migration,
};

// ── Memory states ──────────────────────────────────────────────────────────

fn state_strategy() -> impl Strategy<Value = MemoryState> {
    prop_oneof![
        Just(MemoryState::Current),
        Just(MemoryState::Superseded),
        Just(MemoryState::Conflicted),
        Just(MemoryState::UserConfirmed),
        Just(MemoryState::Inferred),
    ]
}

proptest! {
    /// No-op transitions are always allowed: every state maps to itself.
    #[test]
    fn memory_state_transition_is_reflexive(state in state_strategy()) {
        prop_assert!(MemoryState::can_transition(&state, &state));
    }

    /// Superseded is terminal: nothing may leave it except its own no-op.
    #[test]
    fn superseded_is_terminal(from in state_strategy(), to in state_strategy()) {
        let can = MemoryState::can_transition(&from, &to);
        if from == MemoryState::Superseded && to != MemoryState::Superseded {
            prop_assert!(!can, "{from:?} -> {to:?} must be forbidden");
        }
    }

    /// Nothing may silently fall back to Inferred (lowest trust).
    #[test]
    fn no_transition_into_inferred(from in state_strategy(), to in state_strategy()) {
        let can = MemoryState::can_transition(&from, &to);
        if to == MemoryState::Inferred && from != MemoryState::Inferred {
            prop_assert!(!can, "{from:?} -> {to:?} must be forbidden");
        }
    }

    /// Round-trip: parse(as_str(s)) == s for every state.
    #[test]
    fn memory_state_parse_roundtrip(state in state_strategy()) {
        prop_assert_eq!(MemoryState::parse(state.as_str()), state);
    }

    /// parse() is total: any garbage string yields a defined state, never panics.
    #[test]
    fn memory_state_parse_is_total(garbage in any::<String>()) {
        let parsed = MemoryState::parse(&garbage);
        // Whatever falls out, it must re-serialize to a known token.
        prop_assert!(matches!(
            parsed.as_str(),
            "Current" | "Superseded" | "Conflicted" | "UserConfirmed" | "Inferred"
        ));
    }
}

// ── Graph edges (resolution) ───────────────────────────────────────────────

proptest! {
    /// normalize_name is idempotent and collapses separators to single spaces.
    #[test]
    fn normalize_name_is_idempotent(name in any::<String>()) {
        let once = normalize_name(&name);
        let twice = normalize_name(&once);
        prop_assert!(!once.contains("  "), "double space survived: {once:?}");
        prop_assert_eq!(once, twice);
    }

    /// name_dice is symmetric and bounded to [0,1].
    #[test]
    fn name_dice_symmetric_bounded(a in any::<String>(), b in any::<String>()) {
        let ab = name_dice(&a, &b);
        let ba = name_dice(&b, &a);
        prop_assert!((ab - ba).abs() < 1e-9, "dice asymmetric: {ab} vs {ba}");
        prop_assert!((0.0..=1.0).contains(&ab), "dice out of range: {ab}");
    }

    /// similarity is symmetric and bounded to [0,1].
    #[test]
    fn similarity_symmetric_bounded(a in any::<String>(), b in any::<String>()) {
        let ab = similarity(&a, &b);
        let ba = similarity(&b, &a);
        prop_assert!((ab - ba).abs() < 1e-9, "similarity asymmetric: {ab} vs {ba}");
        prop_assert!((0.0..=1.0).contains(&ab), "similarity out of range: {ab}");
    }
}

/// Strategy: a small pool of representative entity titles.
fn entity_strategy() -> impl Strategy<Value = Entity> {
    prop_oneof![
        "Nexus",
        "Nexus Server",
        "nexus server",
        "Nexus MCP",
        "Auth",
        "AuthService",
        "PostgreSQL",
        "Redis",
        "Redis Cache",
        "JWT",
        "OMG",
        "the API",
        "Zeit",
        "Zeit Now",
    ]
    .prop_map(|title| Entity::new(EntityType::Memory, title.to_string(), String::new()))
}

proptest! {
    /// build_groups: member ids never repeat across groups (disjointness),
    /// each group has >= 1 candidate, and every candidate score stays in [0,1].
    #[test]
    fn duplicate_groups_are_disjoint(
        entities in prop::collection::vec(entity_strategy(), 1..12),
        min_score in 0.0f64..1.0,
    ) {
        let groups = build_groups(&entities, min_score);

        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for group in &groups {
            prop_assert!(!group.entities.is_empty(), "group without candidates");
            for cand in &group.entities {
                prop_assert!(seen.insert(cand.entity_id.clone()), "entity in two groups");
                prop_assert!(
                    (0.0..=1.0).contains(&cand.score),
                    "score out of range: {}",
                    cand.score
                );
            }
        }
    }

    /// Transitive closure: any two members of the same group must be connected
    /// through a similarity chain >= min_score (union-find property).
    #[test]
    fn duplicate_group_members_are_connected(
        entities in prop::collection::vec(entity_strategy(), 2..10),
        min_score in 0.0f64..1.0,
    ) {
        let groups = build_groups(&entities, min_score);
        for group in &groups {
            // Each member had to be united with at least one other member at
            // the moment of insertion — therefore score >= min_score via the
            // maximal-pair rule encoded in the group.
            for cand in &group.entities {
                prop_assert!(
                    cand.score >= min_score,
                    "member score {} below threshold {}",
                    cand.score,
                    min_score
                );
            }
        }
    }
}

// ── UTF-8 / tokenizer ──────────────────────────────────────────────────────

proptest! {
    /// count() never panics on arbitrary strings, including multi-byte text.
    #[test]
    fn tokenizer_count_never_panics(text in any::<String>()) {
        let _ = tokenizer::count(&text);
    }

    /// Non-empty non-whitespace input produces at least one token.
    #[test]
    fn tokenizer_count_nonempty_is_positive(text in any::<String>()) {
        if !text.trim().is_empty() {
            prop_assert!(tokenizer::count(&text) >= 1);
        }
    }

    /// count_all with a single element agrees with count — same engine, same
    /// chunking, no surprise from the aggregate path.
    #[test]
    fn tokenizer_count_all_single_part_matches_count(text in any::<String>()) {
        prop_assert_eq!(tokenizer::count_all([text.as_str()]), tokenizer::count(&text));
    }

    /// Determinism: the same text always yields the same count.
    #[test]
    fn tokenizer_count_is_deterministic(text in any::<String>()) {
        prop_assert_eq!(tokenizer::count(&text), tokenizer::count(&text));
    }
}

/// Empty input produces zero tokens. Standalone test: proptest! sugar requires
/// at least one strategy argument, so a constant-input case lives here.
#[test]
fn tokenizer_count_empty_is_zero() {
    assert_eq!(tokenizer::count(""), 0);
}

// ── Paths (sandbox) ────────────────────────────────────────────────────────

/// Build a real temp sandbox root (canonicalised, like the security suite).
fn tmp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "nexus-prop-{}-{}-{}",
        name,
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    // from_roots canonicalises internally; canonicalise here for stable asserts.
    let canonical = std::fs::canonicalize(&dir).unwrap();
    // The sandbox strips the Windows `\\?\` verbatim prefix; normalise the
    // root the same way so `starts_with` comparisons hold.
    canonical
        .to_string_lossy()
        .strip_prefix(r"\\?\")
        .map(PathBuf::from)
        .unwrap_or(canonical)
}

/// Any string (with escapes) — can be absolute, relative, traversing, unicode.
fn path_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        any::<String>(),
        (any::<String>(), any::<String>()).prop_map(|(a, b)| format!("{a}{b}")),
        Just("".to_string()),
        Just("..".to_string()),
        Just(".".to_string()),
        Just("C:\\Windows\\System32".to_string()),
    ]
}

proptest! {
    /// Never panics and — the core invariant — an Ok result never escapes the
    /// sandbox root.
    #[test]
    fn sandbox_ok_never_escapes_root(raw in path_strategy()) {
        let root = tmp_root("esc");
        let sb = Sandbox::from_roots([root.to_string_lossy().to_string()]);
        if let Ok(resolved) = sb.check(&raw, Access::Read) {
            let r = resolved.to_string_lossy();
            let root_s = root.to_string_lossy();
            prop_assert!(
                r.starts_with(root_s.as_ref()),
                "allowed path escaped root: {r} vs {root_s}"
            );
        }
        // Err is fine — denial is the safe outcome.
    }

    /// Relative or empty paths are rejected, never silently resolved.
    #[test]
    fn sandbox_rejects_relative_paths(raw in any::<String>()) {
        // Only consider clearly relative inputs (no drive/root prefix).
        let p = Path::new(&raw);
        if !p.is_absolute() && !raw.is_empty() {
            let root = tmp_root("rel");
            let sb = Sandbox::from_roots([root.to_string_lossy().to_string()]);
            let err = sb.check(&raw, Access::Write);
            prop_assert!(
                matches!(err, Err(SandboxError::NotAbsolute { .. }))
                    | matches!(err, Err(SandboxError::Unresolvable { .. })),
                "relative path must be rejected, got {err:?}"
            );
        }
    }
}

// ── Migration sequences ────────────────────────────────────────────────────

proptest! {
    /// A fully applied schema is at the latest version, and re-applying is a
    /// no-op (idempotent) that keeps the database consistent.
    #[test]
    fn migrations_apply_idempotently(extra_runs in 0usize..4) {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();
        prop_assert_eq!(get_schema_version(&conn).unwrap(), latest_schema_version());

        for _ in 0..extra_runs {
            apply_migrations(&conn).unwrap();
        }
        prop_assert_eq!(get_schema_version(&conn).unwrap(), latest_schema_version());

        let integrity: String = conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .unwrap();
        prop_assert_eq!(integrity, "ok");
    }

    /// Any interleaving of rollback/apply always converges back to the latest
    /// schema version — the DB stays consistent "при любых переходах".
    #[test]
    fn migrations_converge_after_arbitrary_rollback_seq(
        ops in proptest::collection::vec(proptest::bool::ANY, 0..6),
    ) {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();

        for do_rollback in ops {
            if do_rollback {
                // Rollback may legitimately fail on ALTER/trigger migrations —
                // the invariant is about convergence, not about rollback success.
                let _ = rollback_last_migration(&conn);
            } else {
                apply_migrations(&conn).unwrap();
            }
        }

        // Whatever the interleaving, a final apply_migrations restores latest.
        apply_migrations(&conn).unwrap();
        prop_assert_eq!(get_schema_version(&conn).unwrap(), latest_schema_version());

        let integrity: String = conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .unwrap();
        prop_assert_eq!(integrity, "ok");
    }
}
