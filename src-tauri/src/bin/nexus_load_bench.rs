//! Nexus Load Benchmark — memory limits & upper bounds (plan 5.5).
//!
//! Measures honest, reproducible numbers for inserting and searching N
//! memories through the REAL repository (SQLite, real MemoryRecord lifecycle):
//!
//!   - insert throughput (records/sec)
//!   - full-text search latency at each scale
//!   - list(count) latency
//!   - database file size (on-disk footprint per memory)
//!
//! Scales: 1k / 10k / 100k (plan 5.5 target set). No mocks: every row goes
//! through `SqliteMemoryRepository::save_many` (batch transactions), every
//! query through `MemoryRepository::search`. The DB lives in a temp dir, so
//! the benchmark never touches the user's real data.
//!
//! Run:  cargo run --release --bin nexus_load_bench
//!
//! Upper bounds enforced (gate, catastrophe detectors): insert ≥ 300 rec/s at
//! 100k scale, search ≤ 2000ms at 100k scale, list(count) ≤ 2000ms.
//! Exit code 0/1 drives CI; the relative regression check against
//! benchmarks/baseline.json lives in scripts/perf-gate.ps1.

use std::path::PathBuf;
use std::time::Instant;

use nexus::core::memory::memory_record::MemoryRecord;
use nexus::core::memory::memory_repository::MemoryRepository;
use nexus::core::memory::types::MemorySource;
use nexus::storage::sqlite::SqliteMemoryRepository;

const SCALES: &[usize] = &[1_000, 10_000, 100_000];

/// Insert throughput lower bound at the largest scale (records/sec).
/// This is a *catastrophe detector*, not the expected number (same philosophy
/// as SEARCH_MAX_MS below). Measured on the shared windows-latest runner under
/// Defender: ~940 rec/s for batched inserts, vs ~5 300 rec/s on a dev machine
/// (~5.7x slower). A 1 000 rec/s gate would flag the CI hardware, not a
/// regression. Real regressions are caught by the relative check in
/// scripts/perf-gate.ps1 against benchmarks/baseline.json.
const INSERT_MIN_REC_PER_SEC: u64 = 300;
/// Search latency upper bound at the largest scale (ms).
/// This is a *catastrophe detector*, not the expected number: the same budget
/// philosophy as the SLA bench (which gates search at 100 ms against a ~8 ms
/// real value). On a shared CI runner search at 100k is 3-5x slower than on a
/// dev machine, so a 200 ms gate would flag the hardware, not a regression.
/// Real regressions are caught by the relative check in scripts/perf-gate.ps1
/// against benchmarks/baseline.json.
const SEARCH_MAX_MS: u128 = 2_000;
/// List(count) latency upper bound (ms).
const LIST_MAX_MS: u128 = 2_000;

fn temp_db_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nexus-load-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp bench dir");
    dir
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    println!("## Nexus Load Benchmark (plan 5.5)");
    println!();
    println!("| Scale | Insert rec/s | Search ms | List(count) ms | DB size |");
    println!("|---|---|---|---|---|");

    let dir = temp_db_dir();
    let mut gate_ok = true;
    // Values of the last (100k) iteration, for the CI regression gate (plan 6.2).
    let mut last_rec_per_sec = 0u128;
    let mut last_search_ms = 0u128;
    let mut last_list_ms = 0u128;

    for &scale in SCALES {
        // Fresh database per scale so growth is measured from zero.
        let db_path = dir.join(format!("bench-{scale}.db"));
        if db_path.exists() {
            let _ = std::fs::remove_file(&db_path);
        }

        let conn = nexus::db::open_connection_at(&db_path).expect("open bench db");
        let repo = SqliteMemoryRepository::new(conn).expect("init repo");

        // ── insert ──
        // Corpus is heterogeneous like a real memory pool: several topics, so
        // a keyword query matches a *subset* of rows (a homogeneous corpus
        // where every row matches every query is a degenerate worst case).
        let topics = [
            "feature flag toggle rollout",
            "database migration plan",
            "onboarding checklist for new hires",
            "monthly budget review process",
            "authentication middleware design",
            "deployment pipeline configuration",
        ];
        let insert_start = Instant::now();
        // Insert in transaction batches: `save_many` wraps the chunk in one
        // SQLite transaction, so the per-row WAL commit cost (which dominates
        // on CI runners with antivirus/Defender on every write) is paid once
        // per batch, not 100_000 times. Every row still goes through the REAL
        // repository code path - no direct SQL, no mocked storage.
        const INSERT_BATCH: usize = 1_000;
        let mut batch = Vec::with_capacity(INSERT_BATCH);
        for i in 0..scale {
            let topic = topics[i % topics.len()];
            let record = MemoryRecord::new(
                format!("{topic}: memory number {i}"),
                format!(
                    "Notes about {topic} for item {i}. Includes the technical \
                     background, open questions and the agreed next step."
                ),
                "load-bench".into(),
                MemorySource::Manual,
            )
            .expect("valid record");
            batch.push(record);
            if batch.len() == INSERT_BATCH {
                repo.save_many(&batch)
                    .await
                    .expect("batch save must succeed");
                batch.clear();
            }
        }
        if !batch.is_empty() {
            repo.save_many(&batch)
                .await
                .expect("final batch save must succeed");
        }
        let insert_elapsed = insert_start.elapsed();
        let rec_per_sec = scale as u128 * 1000 / insert_elapsed.as_millis().max(1);

        // ── search (matches ~1/6 of the pool, like a real query) ──
        let search_start = Instant::now();
        let hits = repo
            .search("deployment pipeline configuration")
            .await
            .expect("search");
        let search_ms = search_start.elapsed().as_millis();

        // ── list(count) with the page size the UI actually uses ──
        let list_start = Instant::now();
        let listed = repo.list(100, 0).await.expect("list");
        let list_ms = list_start.elapsed().as_millis();

        // ── db size ──
        let size_bytes = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

        if rec_per_sec < INSERT_MIN_REC_PER_SEC as u128
            || search_ms > SEARCH_MAX_MS
            || list_ms > LIST_MAX_MS
        {
            gate_ok = false;
        }

        last_rec_per_sec = rec_per_sec;
        last_search_ms = search_ms;
        last_list_ms = list_ms;

        println!(
            "| {scale} | {rec_per_sec} | {search_ms} | {list_ms} | {} KiB |",
            size_bytes / 1024
        );
        let _ = (hits.len(), listed.len());
    }

    println!();
    println!(
        "Gate (plan 5.5): insert ≥ {INSERT_MIN_REC_PER_SEC} rec/s, search ≤ {SEARCH_MAX_MS} ms, list(100) ≤ {LIST_MAX_MS} ms at 100k"
    );
    println!("GATE: {}", if gate_ok { "PASS" } else { "FAIL" });
    // Machine-readable 100k numbers for the CI regression gate (plan 6.2).
    println!("NEXUS_METRIC load_100k_insert_rec_per_sec={last_rec_per_sec}");
    println!("NEXUS_METRIC load_100k_search_ms={last_search_ms}");
    println!("NEXUS_METRIC load_100k_list_ms={last_list_ms}");
    std::process::exit(if gate_ok { 0 } else { 1 });
}
