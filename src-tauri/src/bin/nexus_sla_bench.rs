//! Nexus SLA benchmark — Performance budget (plan 6.1).
//!
//! Measures every headline operation against its documented SLA and fails
//! (exit 1) when any metric regresses past the budget:
//!
//!   startup  < 2s      — open DB + run all migrations from zero
//!   insert   < 50ms    — single memory save (averaged over a batch)
//!   search   < 100ms   — full-text search over a 10k corpus
//!   cached   < 50ms    — hybrid search with a warm embedding LRU cache
//!   context  < 200ms   — full context-pipeline build (intent→package)
//!   mcp      < 100ms   — one real tools/call round-trip (dispatch only)
//!   index    < 1s/file — semantic fingerprint per record (backfill)
//!
//! Corpus: 10 000 heterogeneous memories (typical desktop scale; 100k is the
//! load-benchmark territory, plan 5.5). Everything runs against an isolated
//! temp DB — the user's data is never touched.
//!
//! Run:  cargo run --release --bin nexus_sla_bench

use std::path::PathBuf;
use std::time::Instant;

use nexus::core::context::ContextBuilder;
use nexus::core::memory::memory_record::MemoryRecord;
use nexus::core::memory::memory_repository::MemoryRepository;
use nexus::core::memory::types::MemorySource;
use nexus::storage::sqlite::SqliteMemoryRepository;

const CORPUS: usize = 10_000;

// SLA budget (ms), straight from plan 6.1.
const SLA_STARTUP_MS: u128 = 2_000;
const SLA_INSERT_MS: u128 = 50;
const SLA_SEARCH_MS: u128 = 100;
const SLA_CACHED_MS: u128 = 50;
const SLA_CONTEXT_MS: u128 = 200;
const SLA_MCP_MS: u128 = 100;
const SLA_INDEX_MS: u128 = 1_000;

const TOPICS: &[&str] = &[
    "feature flag toggle rollout",
    "database migration plan",
    "onboarding checklist for new hires",
    "monthly budget review process",
    "authentication middleware design",
    "deployment pipeline configuration",
];

fn temp_root() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nexus-sla-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// The MCP dispatch and the backfill indexer open the *global* DB
/// (db::open_connection → %LOCALAPPDATA%/Nexus/nexus.db), so for honest
/// latency numbers we point LOCALAPPDATA at our isolated temp dir. Safe here:
/// single-threaded current_thread runtime, set before any thread spawns.
fn isolate_appdata(dir: &std::path::Path) {
    // Safety: no concurrent threads exist at this point (current_thread
    // runtime), and the value lives in an OS env var we own for this process.
    unsafe {
        std::env::set_var("LOCALAPPDATA", dir);
    }
}

async fn build_corpus(repo: &SqliteMemoryRepository, n: usize) {
    for i in 0..n {
        let topic = TOPICS[i % TOPICS.len()];
        let record = MemoryRecord::new(
            format!("{topic}: memory number {i}"),
            format!(
                "Notes about {topic} for item {i}. Includes the technical \
                 background, open questions and the agreed next step."
            ),
            "sla-bench".into(),
            MemorySource::Manual,
        )
        .expect("valid record");
        repo.save(&record).await.expect("save must succeed");
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let root = temp_root();
    // Isolate once, up front: every subsystem (repo, MCP, indexer) now talks
    // to OUR database under root/Nexus/nexus.db.
    isolate_appdata(&root);

    println!("## Nexus SLA benchmark (plan 6.1)");
    println!();
    println!("| Metric | Measured | SLA | OK |");
    println!("|---|---|---|---|");

    let mut gate_ok = true;
    let mut measured: Vec<(String, u128)> = Vec::new();
    let mut record = |metric: &str, measured_ms: u128, sla_ms: u128| {
        let ok = measured_ms < sla_ms;
        gate_ok &= ok;
        measured.push((metric.to_string(), measured_ms));
        println!(
            "| {metric} | {} ms | < {} ms | {} |",
            measured_ms,
            sla_ms,
            if ok { "✓" } else { "✗" }
        );
    };

    // ── startup: open + migrate from zero ──
    // A single shot is too noisy for the relative regression gate (plan 6.2):
    // observed 18-40 ms across identical runs on the same machine (page-cache
    // state, AV scans). Take the median of 5 fresh migrations instead: the DB
    // is deleted before each sample so every run is a true from-zero open.
    let mut startup_samples: Vec<u128> = Vec::with_capacity(5);
    for _ in 0..5 {
        // Wipe the DB so the next sample is a genuine from-zero migration.
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(root.join("Nexus").join(format!("nexus.db{suffix}")));
        }
        let start = Instant::now();
        let repo = SqliteMemoryRepository::new(nexus::db::open_connection().expect("open db"))
            .expect("init repo");
        startup_samples.push(start.elapsed().as_millis());
        drop(repo);
    }
    startup_samples.sort_unstable();
    let startup_ms = startup_samples[startup_samples.len() / 2];
    record("startup", startup_ms, SLA_STARTUP_MS);

    // Re-open for the corpus build below (the median loop dropped its repo).
    let repo = SqliteMemoryRepository::new(nexus::db::open_connection().expect("open db"))
        .expect("init repo");

    // ── insert: average over the whole corpus ──
    let start = Instant::now();
    build_corpus(&repo, CORPUS).await;
    let insert_total = start.elapsed().as_millis();
    record("insert", insert_total / CORPUS as u128, SLA_INSERT_MS);

    // ── search: FTS over the 10k corpus ──
    // A single shot is too noisy for the relative regression gate (plan 6.2):
    // observed 8-16 ms across identical runs on the same machine, driven by
    // page-cache state and AV scans. Take the median of 5 runs instead so the
    // metric reflects the typical latency, not the luckiest page cache.
    let mut search_samples: Vec<u128> = Vec::with_capacity(5);
    let mut search_hits = 0usize;
    for _ in 0..5 {
        let start = Instant::now();
        let hits = repo
            .search("deployment pipeline configuration")
            .await
            .expect("search");
        search_hits = hits.len();
        search_samples.push(start.elapsed().as_millis());
    }
    search_samples.sort_unstable();
    let search_ms = search_samples[search_samples.len() / 2];
    println!("| | | | | search hits: {search_hits}");
    record("search", search_ms, SLA_SEARCH_MS);

    // ── index: semantic fingerprint per record (backfill on this corpus) ──
    let sem = nexus::core::context::semantic_search::SemanticSearch::new(
        nexus::db::open_connection().expect("sem conn"),
    )
    .expect("semantic search");
    let start = Instant::now();
    let report = nexus::core::context::indexer::backfill_with_cancel(&sem, None).expect("backfill");
    let index_ms = start.elapsed().as_millis();
    let per_record = index_ms / report.indexed.max(1) as u128;
    println!("| | | | | index records: {}", report.indexed);
    record("index", per_record, SLA_INDEX_MS);

    // ── context: full pipeline build ──
    let graph_repo = nexus::storage::sqlite::SqliteGraphRepository::new(
        nexus::db::open_connection().expect("graph conn"),
    )
    .expect("graph repo");
    let mem_repo = SqliteMemoryRepository::new(nexus::db::open_connection().expect("builder conn"))
        .expect("builder repo");
    let builder =
        nexus::core::context::context_builder::ContextBuilderImpl::new(graph_repo, mem_repo);
    let start = Instant::now();
    let pkg = builder
        .build_for_query("deployment pipeline configuration")
        .await
        .expect("context build");
    println!("| | | | | context tokens: {}", pkg.token_count);
    record("context", start.elapsed().as_millis(), SLA_CONTEXT_MS);

    // ── cached: same query again, served from the context-package cache ──
    // The production path (commands::context::build_context) wires the builder
    // through ContextService + global_cache; a repeated query must be served
    // from the in-memory cache, not recomputed.
    let snap_conn = nexus::storage::sqlite::context_repository::SqliteContextRepository::new(
        nexus::db::open_connection().expect("snap conn"),
    )
    .expect("snapshot repo");
    let service = nexus::core::context::context_service::ContextService::new(
        builder,
        nexus::core::context::context_cache::global_cache(),
        snap_conn,
    );
    let request = nexus::core::context::context_request::ContextRequest {
        query: "deployment pipeline configuration".into(),
        ..Default::default()
    };
    let _ = service.build_context(&request).await.expect("warm cache");
    let start = Instant::now();
    let cached_pkg = service.build_context(&request).await.expect("cached build");
    println!("| | | | | cached tokens: {}", cached_pkg.token_count);
    record("cached", start.elapsed().as_millis(), SLA_CACHED_MS);

    // ── MCP: one tools/call round-trip (dispatch path, no process spawn) ──
    let mcp_start = Instant::now();
    let line = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"nexus_memory_list","arguments":{"limit":5}}}"#;
    let resp = nexus::ai::mcp_server::handle_request_line(line).await;
    let mcp_ms = mcp_start.elapsed().as_millis();
    assert!(resp.is_some(), "MCP tools/call must produce a response");
    println!(
        "| | | | | mcp response: {} bytes",
        resp.expect("checked").len()
    );
    record("mcp", mcp_ms, SLA_MCP_MS);

    println!();
    println!("GATE: {}", if gate_ok { "PASS" } else { "FAIL" });
    // Machine-readable output for the CI regression gate (plan 6.2).
    for (name, value) in &measured {
        println!("NEXUS_METRIC sla_{name}_ms={value}");
    }
    std::process::exit(if gate_ok { 0 } else { 1 });
}
