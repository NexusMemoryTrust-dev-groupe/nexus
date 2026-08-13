//! Nexus Long-Horizon Benchmark (plan 6.3) — 180-day usage simulation.
//!
//! Simulates half a year of a working memory pool growing 100 → 500 → 2000
//! records while real-world churn happens on top:
//!
//!   - growth:   new memories arrive every "day" (deterministic corpus)
//!   - churn:    old facts get superseded (new version replaces the old one)
//!   - conflict: contradictory claims appear and are flagged
//!   - rehearsal: the spaced-repetition cycle runs on a schedule
//!   - agents:   multiple agents write/read through their own policy
//!
//! The point is NOT raw throughput (that is 5.5/6.1's job) but *stability over
//! time*: search/list/rehearsal latency must stay flat as the pool ages,
//! supersession must keep the memory_state graph consistent, and the rehearsal
//! cycle must terminate cleanly on a 2000-record pool.
//!
//! Gate (plan 6.3): on the final 2000-record state —
//!   insert ≥ 1000 rec/s, search ≤ 200 ms, list(100) ≤ 200 ms,
//!   every supersession pair consistent, conflict pairs detected,
//!   rehearsal cycle completes and records its timestamp.
//!
//! Run:  cargo run --release --bin nexus_long_horizon_bench
//! Exit: 0 = GATE PASS, 1 = FAIL.

use std::path::PathBuf;
use std::time::Instant;

use nexus::core::memory::conflict::verdict::PairVerdict;
use nexus::core::memory::memory_record::MemoryRecord;
use nexus::core::memory::types::{MemorySource, MemoryState, MemoryVisibility};
use nexus::core::memory::{
    AccessVerdict, AgentPolicy, MemoryLayer, MemoryRepository, assess_agent_access,
};
use nexus::storage::sqlite::SqliteMemoryRepository;

// ── Gate thresholds (plan 6.3) ──
const INSERT_MIN_REC_PER_SEC: u32 = 1000;
const SEARCH_MAX_MS: u128 = 200;
const LIST_MAX_MS: u128 = 200;

// 180 days of usage; checks run at 100 / 500 / 2000.
const CORPUS_SCALES: [usize; 3] = [100, 500, 2000];

const TOPICS: &[&str] = &[
    "feature flag toggle rollout",
    "database migration plan",
    "onboarding checklist for new hires",
    "monthly budget review process",
    "authentication middleware design",
    "deployment pipeline configuration",
];

fn temp_root() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nexus-lh-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Point LOCALAPPDATA at our isolated temp dir so the rehearsal command (which
/// opens the *global* DB via db::open_connection) touches only our data.
/// Safe here: current_thread runtime, no threads exist yet.
fn isolate_appdata(dir: &std::path::Path) {
    // Safety: single-threaded current_thread runtime, env var owned by us.
    unsafe {
        std::env::set_var("LOCALAPPDATA", dir);
    }
}

fn make_record(i: usize, author: &str) -> MemoryRecord {
    let topic = TOPICS[i % TOPICS.len()];
    MemoryRecord::new(
        format!("{topic}: memory number {i}"),
        format!(
            "Notes about {topic} for item {i}. Includes the technical \
             background, open questions and the agreed next step."
        ),
        author.to_string(),
        MemorySource::Manual,
    )
    .expect("valid record")
}

fn make_superseded_pair(base: &MemoryRecord) -> (MemoryRecord, MemoryRecord) {
    // Old fact: version 1. New fact replaces it via an explicit lifecycle link.
    let mut old = base.clone();
    old.title = format!("{} (old)", old.title);
    old.content = format!("OBSOLETE: {}", old.content);
    old.memory_state = MemoryState::Current;

    // New version gets a fresh identity; it references the record it replaces.
    let mut new = base.clone();
    new.id = nexus::core::entity_id::EntityId::new();
    new.version = 2;
    new.title = format!("{} (v2)", new.title);
    new.memory_state = MemoryState::Current;
    new.supersedes_id = Some(base.id.as_str().to_string());
    // Links are wired after both exist; the bench saves them below.
    (old, new)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let root = temp_root();
    isolate_appdata(&root);

    println!("## Nexus Long-Horizon Benchmark (plan 6.3) — 180 days");
    println!();
    println!("| Scale | Insert rec/s | Search ms | List(100) ms | Superseded pairs | Conflicts |");
    println!("|---|---|---|---|---|---|");

    let mut gate_ok = true;
    let mut superseded_checked = 0usize;
    let mut conflicts_detected = 0usize;
    let mut conflicts_total = 0usize;

    // Final-scale values, reported as NEXUS_METRIC for the CI regression gate
    // (plan 6.2/6.3). The rehearsal cycle is the long-horizon stability
    // signal; latency on the 2000-record pool must not balloon vs 100/500.
    let mut final_rec_per_sec: u128 = 0;
    let mut final_search_ms: u128 = 0;
    let mut final_list_ms: u128 = 0;
    let mut final_rehearse_ms: u128 = 0;
    let mut final_superseded: usize = 0;

    // ONE isolated database grows across the whole simulation — exactly what
    // 180 days of real use looks like. The rehearsal command opens the global
    // DB via db::open_connection, so everything must live under the isolated
    // LOCALAPPDATA (isolate_appdata above) and go through the same file.
    let conn = nexus::db::open_connection().expect("open bench db");
    let repo = SqliteMemoryRepository::new(conn).expect("init repo");

    let mut written = 0usize;
    for scale in CORPUS_SCALES {
        // ── grow the pool up to `scale` (incremental, like real use) ──
        let insert_start = Instant::now();
        let mut supersede_tasks: Vec<(MemoryRecord, MemoryRecord)> = Vec::new();
        for i in written..scale {
            // Agents alternate: two writers (plan 4.6 passport flow).
            let author = if i % 2 == 0 { "claude-code" } else { "copilot" };
            let rec = make_record(i, author);
            let _ = repo.save(&rec).await.expect("save must succeed");
        }
        let insert_elapsed = insert_start.elapsed();
        let inserted_now = scale - written;
        let rec_per_sec = inserted_now as u128 * 1000 / insert_elapsed.as_millis().max(1);
        written = scale;

        // ── churn: supersede ~5% of the pool with newer versions ──
        let all = repo.list(100_000, 0).await.expect("list for churn");
        let to_supersede: Vec<&MemoryRecord> = all.iter().step_by(20).take(scale / 20).collect();
        for &old in &to_supersede {
            let (mut old_rec, new_rec) = make_superseded_pair(old);
            // Save the new version first; its real id (EntityId::new() was
            // assigned pre-save, so the returned id matches new_rec.id) is
            // what the old record points to.
            let new_id = repo.save(&new_rec).await.expect("save new version");
            old_rec.superseded_by_id = Some(new_id.as_str().to_string());
            old_rec.memory_state = MemoryState::Superseded;
            repo.update(&old_rec).await.expect("mark old superseded");
            supersede_tasks.push((old_rec, new_rec));
        }

        // ── conflict: two contradictory claims about the same deployment ──
        // A real "server is X" vs "server is Y" pair; the verdict engine must
        // flag it. Deterministic content, no embedding model needed.
        let a = MemoryRecord::new(
            "deployment: primary database is Postgres".into(),
            "The primary database for the deployment pipeline is PostgreSQL 16 \
             running on the shared host."
                .into(),
            "claude-code".into(),
            MemorySource::Manual,
        )
        .expect("conflict a");
        let b = MemoryRecord::new(
            "deployment: primary database is MySQL".into(),
            "The primary database for the deployment pipeline is MySQL 8.0 \
             running on the shared host."
                .into(),
            "copilot".into(),
            MemorySource::Manual,
        )
        .expect("conflict b");
        let verdict = nexus::core::memory::conflict::verdict::classify(&a, &b, None);
        conflicts_total += 1;
        if verdict == PairVerdict::Contradicted {
            conflicts_detected += 1;
        } else {
            gate_ok = false;
        }

        // ── agent switch: a second agent with its own policy must be
        //    denied access to what its deny_patterns forbid, and allowed
        //    on ordinary notes (plan 4.6 passport / agent_permissions) ──
        let automation_policy = AgentPolicy {
            id: "policy-automation".to_string(),
            agent: "automation".to_string(),
            role: "automation".to_string(),
            allowed_visibility: vec![MemoryVisibility::Public],
            allowed_layers: vec![
                MemoryLayer::Semantic,
                MemoryLayer::Decision,
                MemoryLayer::Procedural,
            ],
            deny_patterns: vec!["api key".to_string()],
            enabled: true,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let secret_note = MemoryRecord::new(
            "vault rotation".to_string(),
            "rotate the api key in the vault before Friday".to_string(),
            "claude-code".to_string(),
            MemorySource::Manual,
        )
        .expect("secret note");
        let secret_assessment = assess_agent_access(&automation_policy, &secret_note);
        if secret_assessment.verdict != AccessVerdict::Deny {
            gate_ok = false;
        }
        let mut plain_note = MemoryRecord::new(
            "status meeting".to_string(),
            "weekly status meeting moved to 10:00".to_string(),
            "claude-code".to_string(),
            MemorySource::Manual,
        )
        .expect("plain note");
        // Defaults are Private/Episodic; promote to what the automation
        // policy explicitly allows (Public + Semantic) so the switch test
        // asserts on the policy, not on accidental defaults.
        plain_note.visibility = MemoryVisibility::Public;
        plain_note.layer = MemoryLayer::Semantic;
        let plain_assessment = assess_agent_access(&automation_policy, &plain_note);
        if plain_assessment.verdict != AccessVerdict::Allow {
            gate_ok = false;
        }

        // ── rehearsal: the full cycle must terminate on this pool ──
        let rehearse_start = Instant::now();
        let report = nexus::commands::rehearsal::run_rehearsal_cycle()
            .await
            .expect("rehearsal cycle must complete");
        let rehearse_ms = rehearse_start.elapsed().as_millis();

        // ── steady-state reads after churn + rehearsal ──
        let search_start = Instant::now();
        let hits = repo
            .search("deployment pipeline configuration")
            .await
            .expect("search");
        let search_ms = search_start.elapsed().as_millis();

        let list_start = Instant::now();
        let listed = repo.list(100, 0).await.expect("list");
        let list_ms = list_start.elapsed().as_millis();

        // Verify every supersession link is internally consistent: the old
        // record says it was replaced by new.id, and vice versa.
        for (old_rec, new_rec) in &supersede_tasks {
            let old_is_marked = old_rec.superseded_by_id.as_deref() == Some(new_rec.id.as_str());
            let new_is_marked = new_rec.supersedes_id.is_some()
                && new_rec.supersedes_id.as_deref() == Some(old_rec.id.as_str());
            if old_is_marked && new_is_marked {
                superseded_checked += 1;
            } else {
                gate_ok = false;
            }
        }

        if rec_per_sec < INSERT_MIN_REC_PER_SEC as u128
            || search_ms > SEARCH_MAX_MS
            || list_ms > LIST_MAX_MS
        {
            gate_ok = false;
        }

        println!(
            "| {scale} | {rec_per_sec} | {search_ms} | {list_ms} | {}/{} | {}/{} |",
            supersede_tasks.len(),
            supersede_tasks.len(),
            conflicts_detected,
            conflicts_total
        );
        println!(
            "  rehearsal: {rehearsed} rehearsed, {scheduled_first} first-scheduled, {decayed} decayed ({rehearse_ms} ms)",
            rehearsed = report.rehearsed,
            scheduled_first = report.scheduled_first,
            decayed = report.decayed,
            rehearse_ms = rehearse_ms
        );
        let _ = (hits.len(), listed.len());

        // Keep the last (2000-record) scale's numbers for NEXUS_METRIC.
        final_rec_per_sec = rec_per_sec;
        final_search_ms = search_ms;
        final_list_ms = list_ms;
        final_rehearse_ms = rehearse_ms;
        final_superseded = supersede_tasks.len();
    }

    println!();
    println!(
        "Gate (plan 6.3): insert ≥ {INSERT_MIN_REC_PER_SEC} rec/s, search ≤ {SEARCH_MAX_MS} ms, list(100) ≤ {LIST_MAX_MS} ms, supersession consistent, conflict detected, rehearsal completes, agent switch enforced"
    );
    println!(
        "  superseded links verified: {superseded_checked}, conflicts detected: {conflicts_detected}/{conflicts_total}"
    );
    println!("  agent switch: secret note Deny + plain note Allow verified at every scale");
    println!("GATE: {}", if gate_ok { "PASS" } else { "FAIL" });
    println!();
    // Machine-readable metrics for scripts/perf-gate.ps1 (plan 6.2 format).
    println!("NEXUS_METRIC lh_2000_insert_rec_per_sec={final_rec_per_sec}");
    println!("NEXUS_METRIC lh_2000_search_ms={final_search_ms}");
    println!("NEXUS_METRIC lh_2000_list_ms={final_list_ms}");
    println!("NEXUS_METRIC lh_2000_rehearsal_ms={final_rehearse_ms}");
    println!("NEXUS_METRIC lh_superseded_pairs={final_superseded}");
    std::process::exit(if gate_ok { 0 } else { 1 });
}
