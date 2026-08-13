//! Fuzz smoke targets (plan 8.3): parsers, path sanitizer, MCP input, graph
//! resolver, context builder.
//!
//! Real `cargo-fuzz` needs a nightly toolchain that is not present in this
//! environment, so each target is a deterministic xorshift-driven loop that
//! throws garbage at the public entry point for a bounded budget
//! (`NEXUS_FUZZ_SECONDS`, default 60 — the plan's "60s smoke per target") and
//! fails the test if any call panics. Inputs are reproducible: the seed is
//! fixed, so a failing run can be replayed exactly.
//!
//! Budget control:
//!   NEXUS_FUZZ_SECONDS=60   (default, plan acceptance)
//!   NEXUS_FUZZ_SECONDS=3    (fast local run)
//!
//! Every target also runs a fixed set of adversarial seeds (empty, NULs,
//! UTF-8 boundaries, max-length, path tricks) before the random loop.

use std::time::{Duration, Instant};

use nexus::ai::copilot::parse_command;
use nexus::core::graph::entity::Entity;
use nexus::core::graph::entity_types::EntityType;
use nexus::core::graph::resolution::{build_groups, name_dice, normalize_name, similarity};
use nexus::core::interpreter::code_parser::{
    parse_c_cpp, parse_go, parse_java, parse_javascript, parse_python, parse_rust, parse_typescript,
};
use nexus::core::interpreter::config_parser::{parse_json, parse_toml, parse_yaml};
use nexus::core::interpreter::markdown_parser::{parse as parse_markdown, strip_markdown};
use nexus::core::sandbox::{Access, Sandbox};

// ── Deterministic PRNG ─────────────────────────────────────────────────────

/// xorshift64* — tiny, fast, deterministic, no external state.
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn byte(&mut self) -> u8 {
        (self.next() & 0xFF) as u8
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % (n as u64 + 1)) as usize
    }

    /// Random byte string (not necessarily valid UTF-8).
    fn bytes(&mut self, max_len: usize) -> Vec<u8> {
        let len = self.below(max_len);
        (0..len).map(|_| self.byte()).collect()
    }

    /// Random valid UTF-8 string, mixing scripts and sizes.
    fn string(&mut self, max_len: usize) -> String {
        let len = self.below(max_len);
        let mut s = String::with_capacity(len + 8);
        while s.len() < len {
            s.push(match self.below(4) {
                0 => (b'a' + (self.byte() % 26)) as char,
                1 => char::from_u32('а' as u32 + (self.byte() as u32 % 32)).unwrap(),
                2 => '\u{4E00}',
                _ => (b'0' + (self.byte() % 10)) as char,
            });
        }
        s
    }
}

/// Seconds per target from the environment.
fn budget() -> Duration {
    let secs = std::env::var("NEXUS_FUZZ_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    Duration::from_secs(secs)
}

/// Adversarial fixed inputs every target must survive.
fn adversarial_inputs() -> Vec<String> {
    vec![
        String::new(),
        "\0".to_string(),
        "\0\0\0\0".to_string(),
        "a".repeat(70_000),
        "Привет мир — тест кириллицы и дефисов 🚀".to_string(),
        " ".repeat(10_000),
        "\\u{0}\\u{10FFFF}".to_string(),
        "\u{200B}\u{200C}\u{200D}".to_string(),
        "C:\\Windows\\System32\\..\\..\\Windows".to_string(),
        "\\\\?\\C:\\Users\\..\\..".to_string(),
        "../../../../etc/passwd".to_string(),
        "/dev/null".to_string(),
        "path with  spaces  and\tabs".to_string(),
        "<script>alert(1)</script>".to_string(),
        "{{7*7}}".to_string(),
        "%00%0A%0D".to_string(),
        "[\"a\",{\"b\":[1,2,3]}]".to_string(),
        "---\na: [1, 2\nb: !!str x".to_string(),
        "{\"a\":".to_string(),
    ]
}

/// Run `f` against every adversarial input, then random inputs until the
/// budget is exhausted. `f` returns a fresh closure-friendly call; panic in
/// any call fails the test.
fn fuzz_loop(mut f: impl FnMut(&str)) {
    let deadline = Instant::now() + budget();
    for seed in adversarial_inputs() {
        f(&seed);
    }
    let mut rng = XorShift(0x8E5A_2B3C_4D5E_6F70);
    let mut iterations: u64 = 0;
    while Instant::now() < deadline {
        let input = match rng.below(3) {
            0 => String::from_utf8_lossy(&rng.bytes(4096)).into_owned(),
            1 => rng.string(2048),
            _ => String::from_utf8_lossy(&rng.bytes(16)).into_owned(),
        };
        f(&input);
        iterations += 1;
    }
    eprintln!("[fuzz] {} iterations completed", iterations);
}

// ── Target 1: parsers ──────────────────────────────────────────────────────

#[test]
fn fuzz_parsers() {
    fuzz_loop(|s| {
        let _ = parse_python(s);
        let _ = parse_javascript(s);
        let _ = parse_typescript(s);
        let _ = parse_rust(s);
        let _ = parse_go(s);
        let _ = parse_java(s);
        let _ = parse_c_cpp(s, ".c");
        let _ = parse_json(s, "file.json");
        let _ = parse_yaml(s, "file.yaml");
        let _ = parse_toml(s, "file.toml");
        let _ = parse_markdown(s);
        let _ = strip_markdown(s);
    });
}

// ── Target 2: path sanitizer ───────────────────────────────────────────────

#[test]
fn fuzz_path_sanitizer() {
    let dir = std::env::temp_dir().join(format!("nexus-fuzz-sandbox-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let sb = Sandbox::from_roots([dir.to_string_lossy().to_string()]);

    fuzz_loop(|s| {
        let _ = sb.check(s, Access::Read);
        let _ = sb.check(s, Access::Write);
    });

    let _ = std::fs::remove_dir_all(&dir);
}

// ── Target 3: MCP input ────────────────────────────────────────────────────

#[test]
fn fuzz_mcp_input() {
    fuzz_loop(|s| {
        // Slash-command parser is the front door for MCP tools/call.
        let _ = parse_command(s);
    });
}

// ── Target 4: graph resolver ───────────────────────────────────────────────

#[test]
fn fuzz_graph_resolver() {
    fuzz_loop(|s| {
        let _ = normalize_name(s);
        let _ = name_dice(s, s);
        let _ = similarity(s, s);
        let _ = build_groups(
            &[
                Entity::new(EntityType::Person, s.to_string(), s.to_string()),
                Entity::new(EntityType::Project, s.to_string(), s.to_string()),
            ],
            0.5,
        );
    });
}

// ── Target 5: context builder ──────────────────────────────────────────────

#[test]
fn fuzz_context_builder() {
    use nexus::core::context::context_builder::{ContextBuilder, ContextBuilderImpl};
    use nexus::core::context::context_request::ContextRequest;
    use nexus::storage::sqlite::graph_repository::SqliteGraphRepository;
    use nexus::storage::sqlite::memory_repository_sqlite::SqliteMemoryRepository;

    // One in-memory store for the whole run (migrations are slow per-call).
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    nexus::storage::sqlite::schema::apply_migrations(&conn).unwrap();
    let graph = SqliteGraphRepository::new(conn).unwrap();

    let memory_conn = rusqlite::Connection::open_in_memory().unwrap();
    nexus::storage::sqlite::schema::apply_migrations(&memory_conn).unwrap();
    let memory = SqliteMemoryRepository::new(memory_conn).unwrap();

    let builder = ContextBuilderImpl::new(graph, memory);

    let rt = tokio::runtime::Runtime::new().unwrap();

    fuzz_loop(|s| {
        let req = ContextRequest {
            query: s.to_string(),
            max_tokens: 1 + (s.len() as u32 % 8000),
            max_entities: 1 + (s.len() as u32 % 200),
            max_depth: s.len() as u32 % 5,
            min_relevance: (s.len() as f64 % 100.0) / 100.0,
            ..Default::default()
        };
        let _ = rt.block_on(builder.build(&req));
    });
}
