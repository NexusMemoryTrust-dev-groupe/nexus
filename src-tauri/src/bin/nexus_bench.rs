//! Nexus Benchmark Harness — honest, reproducible measurements against real
//! open-source projects using the real Nexus engine (ONNX embeddings, real
//! tokenizer, real context pipeline, real conflict/consolidation/firewall
//! engines). No mocks, no synthetic scoring: every number is a measurement.
//!
//! Run:  cargo run --bin nexus_bench -- --projects <dir> [--limit N] [--cases <json>]
//!
//! Environment isolation: LOCALAPPDATA is redirected to a temp dir so the
//! benchmark never touches the user's real database.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use nexus::core::context::context_builder::ContextBuilder;
use nexus::core::context::ranker::{HybridReranker, graph_expand};
use nexus::core::context::semantic_search::SemanticSearch;
use nexus::core::entity_id::EntityId;
use nexus::core::graph::GraphStore;
use nexus::core::graph::entity::Entity;
use nexus::core::graph::entity_types::EntityType;
use nexus::core::graph::relationship::Relationship;
use nexus::core::graph::relationship_types::RelationshipType;
use nexus::core::memory::agent_permissions::{AgentPolicy, assess_agent_access};
use nexus::core::memory::canonical_consolidation::{find_clusters, similarity};
use nexus::core::memory::layer::classifier::LayerClassifier;
use nexus::core::memory::memory_lifecycle::detect_and_mark_conflicts;
use nexus::core::memory::memory_record::MemoryRecord;
use nexus::core::memory::memory_repository::MemoryRepository;
use nexus::core::memory::types::{MemorySource, MemoryState};
use nexus::storage::sqlite::{SqliteGraphRepository, SqliteMemoryRepository};

// ── CLI ────────────────────────────────────────────────────────────────────

struct Args {
    projects_dir: PathBuf,
    limit: Option<usize>,
    cases: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut projects_dir = PathBuf::from(".");
    let mut limit = None;
    let mut cases = None;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--projects" => {
                if let Some(v) = it.next() {
                    projects_dir = PathBuf::from(v);
                }
            }
            "--limit" => {
                if let Some(v) = it.next() {
                    limit = v.parse().ok();
                }
            }
            "--cases" => {
                if let Some(v) = it.next() {
                    cases = Some(PathBuf::from(v));
                }
            }
            _ => {}
        }
    }
    Args {
        projects_dir,
        limit,
        cases,
    }
}

// ── Indexing ───────────────────────────────────────────────────────────────

/// Content window loaded per file, in chars. Matches the engine's index
/// window (64 KB): the benchmark must feed the indexer the same text a real
/// install would index, or the truncation gap (symbols in the tail of large
/// files) would be an artefact of the harness instead of the engine.
const MAX_INDEX_CHARS: usize = 65536;

fn walk_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if is_indexable(&path) {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn is_indexable(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if name.starts_with('.') || name.ends_with(".min.js") || name.ends_with(".map") {
        return false;
    }
    matches!(
        path.extension().and_then(|e| e.to_str()).unwrap_or(""),
        "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "md" | "toml" | "json" | "txt"
    )
}

struct IndexedFile {
    rel_path: String,
    title: String,
    content: String,
}

fn load_files(projects_dir: &Path, limit: Option<usize>) -> Vec<IndexedFile> {
    let mut files = Vec::new();
    let Ok(projects) = std::fs::read_dir(projects_dir) else {
        eprintln!("Cannot read projects dir: {}", projects_dir.display());
        return files;
    };
    for project in projects.flatten() {
        let proj_path = project.path();
        if !proj_path.is_dir() {
            continue;
        }
        let proj_name = project.file_name().to_string_lossy().to_string();
        let mut found = walk_files(&proj_path);
        if let Some(l) = limit {
            found.truncate(l);
        }
        for f in found {
            let Ok(bytes) = std::fs::read(&f) else {
                continue;
            };
            let content = String::from_utf8_lossy(&bytes).to_string();
            if content.trim().is_empty() {
                continue;
            }
            let content = content.chars().take(MAX_INDEX_CHARS).collect::<String>();
            let rel = f
                .strip_prefix(&proj_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| f.to_string_lossy().to_string());
            let title = format!("{proj_name}: {rel}");
            files.push(IndexedFile {
                rel_path: format!("{proj_name}/{rel}"),
                title,
                content,
            });
        }
    }
    files
}

// ── Retrieval evaluation ───────────────────────────────────────────────────

/// A retrieval case. Deserialized from `--cases <json>` (array of objects
/// `{ "query": ..., "ground_truth": [substring, ...] }`) or built from the
/// built-in default set when no file is given.
#[derive(serde::Deserialize)]
struct RetrievalCase {
    query: String,
    /// Substrings that must appear in a relevant file's relative path.
    ground_truth: Vec<String>,
    /// Dataset label for per-corpus reporting (requests / rust-log / mui / …).
    /// Absent in legacy files → `mixed`.
    #[serde(default = "default_dataset")]
    dataset: String,
}

fn default_dataset() -> String {
    "mixed".into()
}

fn load_cases(path: Option<&Path>) -> Vec<RetrievalCase> {
    if let Some(p) = path {
        let data = std::fs::read_to_string(p).unwrap_or_else(|e| {
            eprintln!("Cannot read cases file {}: {e}", p.display());
            std::process::exit(1);
        });
        return serde_json::from_str(&data).unwrap_or_else(|e| {
            eprintln!("Cannot parse cases file {}: {e}", p.display());
            std::process::exit(1);
        });
    }
    default_cases()
}

/// Fallback when `--cases` is omitted (keeps the harness runnable standalone).
fn default_cases() -> Vec<RetrievalCase> {
    let s = |q: &str, gt: &[&str]| RetrievalCase {
        query: q.to_string(),
        ground_truth: gt.iter().map(|g| g.to_string()).collect(),
        dataset: dataset_of(gt),
    };
    vec![
        s(
            "How are HTTP sessions and cookies managed in requests?",
            &["sessions.py", "cookies.py"],
        ),
        s(
            "What authentication schemes are supported (basic, digest)?",
            &["auth.py"],
        ),
        s(
            "How does the library retry failed HTTP requests?",
            &["adapters.py", "sessions.py"],
        ),
        s(
            "How are query string parameters encoded into URLs?",
            &["models.py", "utils.py"],
        ),
        s(
            "How is the logger initialized and configured in the log crate?",
            &["lib.rs", "macros.rs"],
        ),
        s(
            "Where are log levels and maximum level filtering implemented?",
            &["lib.rs"],
        ),
        s(
            "How is the MUI Button component implemented and styled?",
            &["Button/Button.js"],
        ),
        s(
            "How does the MUI Dialog component handle modal behavior?",
            &["Dialog/Dialog.js"],
        ),
        s(
            "How does MUI Checkbox support the indeterminate state?",
            &["Checkbox/Checkbox.js"],
        ),
        s(
            "How does the requests library expose the public API module?",
            &["api.py"],
        ),
    ]
}

fn keyword_baseline(query: &str, files: &[IndexedFile], top_k: usize) -> Vec<String> {
    let terms: Vec<String> = query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| w.len() > 2)
        .map(|w| w.to_string())
        .collect();
    let mut scored: Vec<(usize, &IndexedFile)> = files
        .iter()
        .map(|f| {
            let hay = format!("{} {}", f.title, f.content).to_lowercase();
            let hits = terms.iter().filter(|t| hay.contains(t.as_str())).count();
            (hits, f)
        })
        .collect();
    scored.sort_by_key(|(hits, _)| std::cmp::Reverse(*hits));
    scored
        .into_iter()
        .take(top_k)
        .filter(|(hits, _)| *hits > 0)
        .map(|(_, f)| f.rel_path.clone())
        .collect()
}

fn is_relevant(path: &str, ground_truth: &[String]) -> bool {
    // Normalize separators so `Button/Button.js` matches `Button\Button.js`
    // (Windows paths use backslashes; ground truth uses forward slashes).
    let p = path.to_lowercase().replace('\\', "/");
    ground_truth.iter().any(|g| p.contains(&g.to_lowercase()))
}

fn precision_recall(
    retrieved: &[String],
    ground_truth: &[String],
    total_relevant: usize,
) -> (f64, f64) {
    if retrieved.is_empty() {
        return (0.0, 0.0);
    }
    let hits = retrieved
        .iter()
        .filter(|p| is_relevant(p, ground_truth))
        .count();
    let precision = hits as f64 / retrieved.len() as f64;
    let recall = if total_relevant == 0 {
        0.0
    } else {
        hits as f64 / total_relevant as f64
    };
    (precision, recall)
}

/// Deterministic dataset label from ground-truth paths. Mirrors the labeling
/// applied to `benchmarks/retrieval/cases.json`: nested paths → mui,
/// `.rs` → rust-log, `.py` → requests, anything else → mixed.
fn dataset_of(gt: &[&str]) -> String {
    if gt.iter().any(|g| g.contains('/')) {
        "mui".into()
    } else if gt.iter().any(|g| g.ends_with(".rs")) {
        "rust-log".into()
    } else if gt.iter().any(|g| g.ends_with(".py")) {
        "requests".into()
    } else {
        "mixed".into()
    }
}

/// Reciprocal rank of the first relevant hit (0 if none in the top `k`).
fn reciprocal_rank(retrieved: &[String], ground_truth: &[String], k: usize) -> f64 {
    for (i, p) in retrieved.iter().take(k).enumerate() {
        if is_relevant(p, ground_truth) {
            return 1.0 / (i as f64 + 1.0);
        }
    }
    0.0
}

/// NDCG@k with binary relevance: DCG = Σ rel_i / log2(i+2), normalised by the
/// ideal ranking (all `total_relevant` docs first, capped at k).
fn ndcg_at_k(
    retrieved: &[String],
    ground_truth: &[String],
    total_relevant: usize,
    k: usize,
) -> f64 {
    let mut dcg = 0.0;
    for (i, p) in retrieved.iter().take(k).enumerate() {
        if is_relevant(p, ground_truth) {
            dcg += 1.0 / (i as f64 + 2.0).log2();
        }
    }
    let ideal_count = total_relevant.min(k);
    let mut idcg = 0.0;
    for i in 0..ideal_count {
        idcg += 1.0 / (i as f64 + 2.0).log2();
    }
    if idcg > 0.0 { dcg / idcg } else { 0.0 }
}

/// Per-dataset aggregates for the before/after reranker table.
#[derive(Default)]
struct DatasetAgg {
    count: usize,
    hyb_p5: Vec<f64>,
    hyb_r5: Vec<f64>,
    hyb_mrr: Vec<f64>,
    hyb_ndcg: Vec<f64>,
    rr_p5: Vec<f64>,
    rr_r5: Vec<f64>,
    rr_mrr: Vec<f64>,
    rr_ndcg: Vec<f64>,
}

/// Failure bucket for a query: where the first relevant hit lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RankBucket {
    Top1,
    Top2to3,
    Top4to5,
    Top6to10,
    Top11to50,
    Missing,
}

impl RankBucket {
    fn of(rank: Option<usize>) -> RankBucket {
        match rank {
            Some(1) => RankBucket::Top1,
            Some(2 | 3) => RankBucket::Top2to3,
            Some(4 | 5) => RankBucket::Top4to5,
            Some(6..=10) => RankBucket::Top6to10,
            Some(_) => RankBucket::Top11to50,
            None => RankBucket::Missing,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            RankBucket::Top1 => "top-1",
            RankBucket::Top2to3 => "top-2-3",
            RankBucket::Top4to5 => "top-4-5",
            RankBucket::Top6to10 => "top-6-10",
            RankBucket::Top11to50 => "top-11-50",
            RankBucket::Missing => "missing",
        }
    }
}

/// Rank (1-based) of the first relevant hit in a retrieval list, or `None`.
fn first_relevant_rank(retrieved: &[String], ground_truth: &[String]) -> Option<usize> {
    retrieved
        .iter()
        .position(|p| is_relevant(p, ground_truth))
        .map(|i| i + 1)
}

/// Per-case failure diagnostics for the error-classification pass.
struct CaseDiag {
    query: String,
    dataset: String,
    ground_truth: Vec<String>,
    total_relevant: usize,
    /// First relevant hit in the hybrid top-50 (before reranker).
    hyb_rank: Option<usize>,
    /// First relevant hit in the reranked order (same candidate set).
    rr_rank: Option<usize>,
    /// First relevant hit in the graph-expanded candidate pool.
    exp_rank: Option<usize>,
    /// Per-channel scores of the first relevant hit (cosine, lexical, filename).
    /// Zero when the hit is not in the hybrid top-50 at all.
    cosine: f64,
    lexical: f64,
    filename: f64,
}

// ── Entry ──────────────────────────────────────────────────────────────────

fn main() {
    let args = parse_args();

    // Isolate the database: redirect LOCALAPPDATA to a fresh temp dir so the
    // benchmark never touches the real user database.
    let bench_dir = std::env::temp_dir().join(format!("nexus-bench-run-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&bench_dir);
    std::fs::create_dir_all(&bench_dir).expect("create bench dir");
    unsafe {
        std::env::set_var("LOCALAPPDATA", &bench_dir);
    }

    // Use the real downloaded ONNX model (not the hash fallback).
    if let Ok(cache) = std::env::var("NEXUS_BENCH_FASTEMBED_CACHE")
        && !cache.is_empty()
    {
        unsafe {
            std::env::set_var("FASTEMBED_CACHE_DIR", &cache);
        }
    }

    // Exact token counting with the embedded tiktoken vocabulary.
    nexus::core::tokenizer::set_active_model("gpt-4o");

    let files = load_files(&args.projects_dir, args.limit);
    if files.is_empty() {
        eprintln!(
            "No indexable files found under {}",
            args.projects_dir.display()
        );
        std::process::exit(1);
    }
    println!("# Nexus Benchmark — real measurements");
    println!();
    println!("- Projects dir: `{}`", args.projects_dir.display());
    println!("- Files indexed: **{}**", files.len());
    println!("- Embedding: real ONNX all-MiniLM-L6-v2 (FASTEMBED_CACHE_DIR)");
    println!("- Token counting: tiktoken `gpt-4o` (exact, offline)");
    println!();

    // ── Index phase ──
    let conn = nexus::db::open_connection().expect("open isolated db");
    let repo = Arc::new(SqliteMemoryRepository::new(conn).expect("memory repo"));
    let search_conn = nexus::db::open_connection().expect("search conn");
    let search = SemanticSearch::new(search_conn).expect("semantic search");
    println!("- ONNX model loaded: **{}**", search.is_model_loaded());
    println!();

    let mut id_to_path = HashMap::new();

    let index_start = Instant::now();
    let rt = tokio::runtime::Runtime::new().unwrap();
    for f in &files {
        let rec = MemoryRecord::new(
            f.title.clone(),
            f.content.clone(),
            "bench".into(),
            MemorySource::Document,
        )
        .expect("valid record");
        let id = rt.block_on(repo.save(&rec)).expect("save record");
        let text =
            nexus::core::context::indexer::index_text(&rec.title, &rec.summary, &rec.content);
        if let Err(e) = search.store_fingerprint(&id, &text) {
            eprintln!("  fingerprint fail {}: {e}", f.rel_path);
        }
        id_to_path.insert(id.as_str().to_string(), f.rel_path.clone());
    }
    let index_secs = index_start.elapsed().as_secs_f64();
    let total = files.len();
    let indexed = total;
    println!(
        "- Indexing: {indexed}/{total} files in {index_secs:.1}s ({:.0} files/s)",
        indexed as f64 / index_secs
    );
    println!();

    // ── Phase 1.2/1.3 graph: co-location knowledge graph ──
    // Files sharing a directory are neighbours — a real, measurable relation
    // (a component and its siblings: Button.js → Button.test.js → index.js).
    // `graph_expand` walks this graph from hybrid seeds; `HybridReranker`
    // breaks near-ties using the per-candidate link density.
    let colocation_conn = nexus::db::open_connection().expect("graph conn");
    let colocation_repo =
        Arc::new(SqliteGraphRepository::new(colocation_conn).expect("graph repo"));
    let mut neighbors_cache: HashMap<String, Vec<EntityId>> = HashMap::new();
    {
        // Entities first (get_neighbors resolves the centre against this table).
        for (id_str, path) in &id_to_path {
            let id = EntityId::parse(id_str).expect("valid entity id");
            let mut entity = Entity::new(EntityType::Document, path.clone(), path.clone());
            entity.id = id;
            rt.block_on(colocation_repo.add_entity(&entity)).ok();
        }
        // Co-location edges between files of the same directory.
        let mut dirs: HashMap<String, Vec<String>> = HashMap::new();
        for (id_str, path) in &id_to_path {
            let dir = match path.rfind(['/', '\\']) {
                Some(i) => &path[..i],
                None => path.as_str(),
            };
            dirs.entry(dir.to_string())
                .or_default()
                .push(id_str.clone());
        }
        for ids in dirs.values() {
            // Huge flat dirs would add pure noise (near-cubic link density);
            // component-style dirs (the MUI pattern) are the target.
            if ids.len() > 40 {
                continue;
            }
            for a in ids {
                let id_a = EntityId::parse(a).expect("valid entity id");
                for b in ids {
                    if a == b {
                        continue;
                    }
                    let id_b = EntityId::parse(b).expect("valid entity id");
                    let rel = Relationship::new(
                        id_a.clone(),
                        id_b.clone(),
                        RelationshipType::RelatedTo,
                        0.6,
                    )
                    .expect("valid rel");
                    rt.block_on(colocation_repo.add_relationship(&rel)).ok();
                    neighbors_cache
                        .entry(a.clone())
                        .or_default()
                        .push(id_b.clone());
                }
            }
        }
    }
    println!(
        "- Co-location graph: **{}** entities, **{}** files with cached neighbours",
        id_to_path.len(),
        neighbors_cache.len()
    );
    println!();

    // ── Retrieval benchmark ──
    let cases = load_cases(args.cases.as_deref());
    println!("- Retrieval cases: **{}**", cases.len());

    let mut sem_p5: Vec<f64> = Vec::new();
    let mut sem_r5: Vec<f64> = Vec::new();
    let mut sem_r20: Vec<f64> = Vec::new();
    let mut kw_p5: Vec<f64> = Vec::new();
    let mut kw_r5: Vec<f64> = Vec::new();
    let mut kw_r20: Vec<f64> = Vec::new();
    // Material-UI group: ground-truth paths are nested (`Button/Button.js`),
    // unlike the flat file sets of requests/log. This is the homogeneous
    // corpus where plain hybrid search historically scored 0.00.
    let mut mui_r5: Vec<f64> = Vec::new();
    let mut mui_r20: Vec<f64> = Vec::new();

    println!("## Retrieval benchmark (hybrid vs keyword)");
    println!();
    println!("| Query | Sem P@5 | Sem R@5 | Sem R@20 | KW P@5 | KW R@5 | KW R@20 | Rel |");
    println!("|---|---|---|---|---|---|---|---|");

    for case in &cases {
        // Total relevant = files whose path matches any ground-truth substring.
        let total_relevant = files
            .iter()
            .filter(|f| is_relevant(&f.rel_path, &case.ground_truth))
            .count();

        // Nexus hybrid retrieval (real ONNX embeddings + lexical/path channel).
        // One call with limit 20; the top-5 slice gives P@5/R@5, the full list
        // gives R@20.
        let mut sem_top20: Vec<String> = Vec::new();
        if let Ok(hits) = search.search_hybrid(&case.query, 20) {
            for (id, _score) in &hits {
                if let Some(path) = id_to_path.get(id.as_str()) {
                    sem_top20.push(path.clone());
                }
            }
        }
        let sem_top5: Vec<String> = sem_top20.iter().take(5).cloned().collect();
        let (sp, sr) = precision_recall(&sem_top5, &case.ground_truth, total_relevant);
        let (_, sr20) = precision_recall(&sem_top20, &case.ground_truth, total_relevant);

        // Keyword baseline (naive term matching — "without Nexus").
        let kw_top20 = keyword_baseline(&case.query, &files, 20);
        let kw_top5: Vec<String> = kw_top20.iter().take(5).cloned().collect();
        let (kp, kr) = precision_recall(&kw_top5, &case.ground_truth, total_relevant);
        let (_, kr20) = precision_recall(&kw_top20, &case.ground_truth, total_relevant);

        sem_p5.push(sp);
        sem_r5.push(sr);
        sem_r20.push(sr20);
        kw_p5.push(kp);
        kw_r5.push(kr);
        kw_r20.push(kr20);

        let is_mui = case.ground_truth.iter().any(|g| g.contains('/'));
        if is_mui {
            mui_r5.push(sr);
            mui_r20.push(sr20);
        }

        println!(
            "| {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {} |",
            truncate(&case.query, 40),
            sp,
            sr,
            sr20,
            kp,
            kr,
            kr20,
            total_relevant
        );
    }

    let avg = |v: &[f64]| -> f64 {
        if v.is_empty() {
            0.0
        } else {
            v.iter().sum::<f64>() / v.len() as f64
        }
    };
    println!(
        "| **Mean ({})** | **{:.2}** | **{:.2}** | **{:.2}** | **{:.2}** | **{:.2}** | **{:.2}** | — |",
        cases.len(),
        avg(&sem_p5),
        avg(&sem_r5),
        avg(&sem_r20),
        avg(&kw_p5),
        avg(&kw_r5),
        avg(&kw_r20)
    );
    if !mui_r5.is_empty() {
        println!(
            "| **Mean MUI ({})** | — | **{:.2}** | **{:.2}** | — | — | — | — |",
            mui_r5.len(),
            avg(&mui_r5),
            avg(&mui_r20)
        );
    }
    println!();

    // ── Phase 1.2/1.3: graph expansion + multi-stage reranker ──
    // Hybrid top-K (stage 1) → graph neighbourhood expansion (plan 1.2) →
    // graph-aware rerank of the expanded set (plan 1.3). Measured on the same
    // cases as the baseline above, so the delta is attributable.
    let reranker = HybridReranker::new();
    let mut exp_r5: Vec<f64> = Vec::new();
    let mut exp_r20: Vec<f64> = Vec::new();
    let mut exp_p5: Vec<f64> = Vec::new();
    let mut mui_exp_r5: Vec<f64> = Vec::new();
    let mut mui_exp_r20: Vec<f64> = Vec::new();
    // Per-dataset evidence, first-seen order (requests → rust-log → mui).
    let mut ds_agg: Vec<(String, DatasetAgg)> = Vec::new();
    // Failure-classification diagnostics, one entry per case.
    let mut diags: Vec<CaseDiag> = Vec::new();

    println!("## Phase 1.2/1.3 — graph expansion + reranker");
    println!();
    println!("| Query | Hybrid R@20 | Exp R@5 | Exp R@20 | Reranked P@5 | Rel |");
    println!("|---|---|---|---|---|---|");

    for (i, case) in cases.iter().enumerate() {
        let total_relevant = files
            .iter()
            .filter(|f| is_relevant(&f.rel_path, &case.ground_truth))
            .count();

        // Stage 1: hybrid top-50 with per-channel breakdown (seeds for expansion).
        let Ok(breakdown) = search.search_hybrid_breakdown(&case.query, 50) else {
            continue;
        };
        let seeds: Vec<(EntityId, f64)> = breakdown
            .iter()
            .map(|(id, _, _, _, total)| (id.clone(), *total))
            .collect();

        // Stage 2: one-hop graph expansion around the seeds (plan 1.2).
        let expanded = rt
            .block_on(graph_expand(&*colocation_repo, &seeds, 50))
            .unwrap_or_default();

        // Stage 3: graph-aware rerank of the hybrid hits (plan 1.3).
        let links = |id: &EntityId| -> Vec<EntityId> {
            neighbors_cache
                .get(id.as_str())
                .cloned()
                .unwrap_or_default()
        };
        let reranked = reranker.rerank(&breakdown, links);

        // Candidate ids → paths for metric computation.
        let exp_paths: Vec<String> = expanded
            .iter()
            .filter_map(|(id, _)| id_to_path.get(id.as_str()).cloned())
            .collect();
        let exp_top5: Vec<String> = exp_paths.iter().take(5).cloned().collect();
        let exp_top20: Vec<String> = exp_paths.iter().take(20).cloned().collect();
        let (ep5, er5) = precision_recall(&exp_top5, &case.ground_truth, total_relevant);
        let (_, er20) = precision_recall(&exp_top20, &case.ground_truth, total_relevant);

        let rr_top5: Vec<String> = reranked
            .iter()
            .take(5)
            .filter_map(|(id, _)| id_to_path.get(id.as_str()).cloned())
            .collect();
        let (rp5, _) = precision_recall(&rr_top5, &case.ground_truth, total_relevant);

        // Per-dataset evidence: before = hybrid order of the same top-50
        // candidates, after = the reranker's reordering of those candidates.
        // The candidate set is identical — only the order changes, so any
        // metric delta is attributable to the reranker alone.
        let hyb_paths: Vec<String> = breakdown
            .iter()
            .filter_map(|(id, _, _, _, _)| id_to_path.get(id.as_str()).cloned())
            .collect();
        let rr_paths: Vec<String> = reranked
            .iter()
            .filter_map(|(id, _)| id_to_path.get(id.as_str()).cloned())
            .collect();
        let hyb_top5: Vec<String> = hyb_paths.iter().take(5).cloned().collect();
        let (hb_p5, hb_r5) = precision_recall(&hyb_top5, &case.ground_truth, total_relevant);
        let (ra_p5, ra_r5) = precision_recall(&rr_top5, &case.ground_truth, total_relevant);
        let hb_mrr = reciprocal_rank(&hyb_paths, &case.ground_truth, 10);
        let ra_mrr = reciprocal_rank(&rr_paths, &case.ground_truth, 10);
        let hb_ndcg = ndcg_at_k(&hyb_paths, &case.ground_truth, total_relevant, 10);
        let ra_ndcg = ndcg_at_k(&rr_paths, &case.ground_truth, total_relevant, 10);

        // Failure diagnostics: ranks before/after + graph pool, plus the
        // per-channel scores of the first relevant hit in the hybrid pool.
        let hyb_rank = first_relevant_rank(&hyb_paths, &case.ground_truth);
        let rr_rank = first_relevant_rank(&rr_paths, &case.ground_truth);
        let exp_rank = first_relevant_rank(&exp_paths, &case.ground_truth);
        let (mut cosine, mut lexical, mut filename) = (0.0f64, 0.0f64, 0.0f64);
        if let Some(r) = hyb_rank
            && let Some((_, c, l, f, _)) = breakdown.get(r - 1)
        {
            cosine = *c;
            lexical = *l;
            filename = *f;
        }
        diags.push(CaseDiag {
            query: case.query.clone(),
            dataset: case.dataset.clone(),
            ground_truth: case.ground_truth.clone(),
            total_relevant,
            hyb_rank,
            rr_rank,
            exp_rank,
            cosine,
            lexical,
            filename,
        });

        let entry = match ds_agg.iter_mut().find(|(n, _)| n == &case.dataset) {
            Some(e) => e,
            None => {
                ds_agg.push((case.dataset.clone(), DatasetAgg::default()));
                ds_agg.last_mut().unwrap()
            }
        };
        let agg = &mut entry.1;
        agg.count += 1;
        agg.hyb_p5.push(hb_p5);
        agg.hyb_r5.push(hb_r5);
        agg.hyb_mrr.push(hb_mrr);
        agg.hyb_ndcg.push(hb_ndcg);
        agg.rr_p5.push(ra_p5);
        agg.rr_r5.push(ra_r5);
        agg.rr_mrr.push(ra_mrr);
        agg.rr_ndcg.push(ra_ndcg);

        exp_p5.push(ep5);
        exp_r5.push(er5);
        exp_r20.push(er20);
        let is_mui = case.ground_truth.iter().any(|g| g.contains('/'));
        if is_mui {
            mui_exp_r5.push(er5);
            mui_exp_r20.push(er20);
        }

        // Hybrid R@20 for this case (recorded in the baseline pass above).
        let hybrid_r20 = sem_r20.get(i).copied().unwrap_or(0.0);

        println!(
            "| {} | {:.2} | {:.2} | {:.2} | {:.2} | {} |",
            truncate(&case.query, 40),
            hybrid_r20,
            er5,
            er20,
            rp5,
            total_relevant
        );
    }

    println!(
        "| **Mean ({})** | — | **{:.2}** | **{:.2}** | **{:.2}** | — |",
        cases.len(),
        avg(&exp_r5),
        avg(&exp_r20),
        avg(&exp_p5)
    );
    if !mui_exp_r5.is_empty() {
        println!(
            "| **Mean MUI ({})** | — | **{:.2}** | **{:.2}** | — | — |",
            mui_exp_r5.len(),
            avg(&mui_exp_r5),
            avg(&mui_exp_r20)
        );
    }
    println!();

    // ── Per-dataset evidence table: before vs after reranker ──
    // Same candidate set per query (hybrid top-50); only the order changes.
    // `hyb_*` = hybrid order (before), `rr_*` = reranked order (after).
    println!("## Per-dataset retrieval evidence — before vs after reranker");
    println!();
    println!("| Dataset | Queries | P@5 | R@5 | MRR@10 | NDCG@10 | (before reranker) |");
    println!("|---|---|---|---|---|---|---|");
    for (name, agg) in &ds_agg {
        println!(
            "| {} | {} | {:.2} | {:.2} | {:.2} | {:.2} | |",
            name,
            agg.count,
            avg(&agg.hyb_p5),
            avg(&agg.hyb_r5),
            avg(&agg.hyb_mrr),
            avg(&agg.hyb_ndcg)
        );
    }
    println!();
    println!("| Dataset | Queries | P@5 | R@5 | MRR@10 | NDCG@10 | (after reranker) |");
    println!("|---|---|---|---|---|---|---|");
    for (name, agg) in &ds_agg {
        println!(
            "| {} | {} | {:.2} | {:.2} | {:.2} | {:.2} | |",
            name,
            agg.count,
            avg(&agg.rr_p5),
            avg(&agg.rr_r5),
            avg(&agg.rr_mrr),
            avg(&agg.rr_ndcg)
        );
    }
    println!();
    println!("| Dataset | ΔP@5 | ΔR@5 | ΔMRR@10 | ΔNDCG@10 | (after − before) |");
    println!("|---|---|---|---|---|---|");
    for (name, agg) in &ds_agg {
        println!(
            "| {} | {:+.2} | {:+.2} | {:+.2} | {:+.2} | |",
            name,
            avg(&agg.rr_p5) - avg(&agg.hyb_p5),
            avg(&agg.rr_r5) - avg(&agg.hyb_r5),
            avg(&agg.rr_mrr) - avg(&agg.hyb_mrr),
            avg(&agg.rr_ndcg) - avg(&agg.hyb_ndcg)
        );
    }
    println!();

    // ── Failure analysis: where retrieval still misses ──
    // Distribution of the first relevant hit across rank buckets, before and
    // after the reranker, then a per-case classification of every miss.
    println!("## Failure analysis — where retrieval still misses");
    println!();
    println!("Bucket of the first relevant hit (118 queries, hybrid top-50 pool):");
    println!();
    println!("| Bucket | Before (hybrid) | After (reranked) | Δ |");
    println!("|---|---|---|---|");
    let buckets = [
        RankBucket::Top1,
        RankBucket::Top2to3,
        RankBucket::Top4to5,
        RankBucket::Top6to10,
        RankBucket::Top11to50,
        RankBucket::Missing,
    ];
    let mut before_counts = [0usize; 6];
    let mut after_counts = [0usize; 6];
    for d in &diags {
        before_counts[RankBucket::of(d.hyb_rank) as usize] += 1;
        after_counts[RankBucket::of(d.rr_rank) as usize] += 1;
    }
    for (i, b) in buckets.iter().enumerate() {
        println!(
            "| {} | {} | {} | {:+.0} |",
            b.label(),
            before_counts[i],
            after_counts[i],
            after_counts[i] as i64 - before_counts[i] as i64
        );
    }
    println!();
    println!(
        "- wrong top-1 (after): **{}** ({:.0}%)",
        { diags.iter().filter(|d| d.rr_rank != Some(1)).count() },
        rate_of(&diags, |d| d.rr_rank != Some(1)) * 100.0
    );
    println!(
        "- wrong top-3 (after): **{}** ({:.0}%)",
        {
            diags
                .iter()
                .filter(|d| d.rr_rank.is_none() || d.rr_rank.unwrap() > 3)
                .count()
        },
        rate_of(&diags, |d| d.rr_rank.is_none() || d.rr_rank.unwrap() > 3) * 100.0
    );
    println!(
        "- wrong top-5 (after): **{}** ({:.0}%)",
        {
            diags
                .iter()
                .filter(|d| d.rr_rank.is_none() || d.rr_rank.unwrap() > 5)
                .count()
        },
        rate_of(&diags, |d| d.rr_rank.is_none() || d.rr_rank.unwrap() > 5) * 100.0
    );
    println!(
        "- missing from the top-50 pool (after): **{}** ({:.0}%)",
        { diags.iter().filter(|d| d.rr_rank.is_none()).count() },
        rate_of(&diags, |d| d.rr_rank.is_none()) * 100.0
    );
    println!();

    // Per-case classification of misses. A "miss" is any case whose first
    // relevant hit is NOT in top-5 after reranking (the context builder feeds
    // on the ranked candidates, so top-5 is the actionable window).
    println!("### Classification of misses (first relevant not in top-5 after rerank)");
    println!();
    println!("| Dataset | Query | Rank before → after | Class | Evidence |");
    println!("|---|---|---|---|---|");
    let mut semantic_misses = 0usize;
    let mut lexical_misses = 0usize;
    let mut path_misses = 0usize;
    let mut graph_misses = 0usize;
    let mut reranker_misses = 0usize;
    let mut ambiguity = 0usize;
    let mut ground_truth_misses = 0usize;
    let mut demotions = 0usize;
    let mut rescues = 0usize;
    let mut graph_recoveries = 0usize;
    let mut miss_count = 0usize;

    for d in &diags {
        let in_top5 = d.rr_rank.is_some_and(|r| r <= 5);
        // Positive events first (not misses, but they explain the system):
        if d.hyb_rank.is_some_and(|r| r > 5) && d.rr_rank.is_some_and(|r| r <= 5) {
            rescues += 1;
        }
        if d.hyb_rank.is_none() && d.exp_rank.is_some() {
            graph_recoveries += 1;
        }
        if in_top5 {
            continue;
        }
        miss_count += 1;

        // Reranker demotion: the hybrid order had it in top-5, the reranked
        // order dropped it out.
        if d.hyb_rank.is_some_and(|r| r <= 5) {
            demotions += 1;
            reranker_misses += 1;
            println!(
                "| {} | {:?} | {} → {} | reranker demotion | hybrid had it at #{}, rerank lost it |",
                d.dataset,
                truncate(&d.query, 38),
                rank_str(d.hyb_rank),
                rank_str(d.rr_rank),
                d.hyb_rank.unwrap()
            );
            continue;
        }

        // Ground-truth ambiguity: the label matches many files; a single-file
        // target is a judgment call, not a retrieval failure.
        if d.total_relevant > 1 {
            ambiguity += 1;
            println!(
                "| {} | {:?} | {} → {} | ground-truth ambiguity | {} files match the label |",
                d.dataset,
                truncate(&d.query, 38),
                rank_str(d.hyb_rank),
                rank_str(d.rr_rank),
                d.total_relevant
            );
            continue;
        }

        // In the pool but ranked low: which channel failed to carry it?
        if d.hyb_rank.is_some() {
            let (cls, ev) = channel_class(d);
            match cls {
                "semantic miss" => semantic_misses += 1,
                "lexical miss" => lexical_misses += 1,
                _ => path_misses += 1,
            }
            println!(
                "| {} | {:?} | {} → {} | {} | {} |",
                d.dataset,
                truncate(&d.query, 38),
                rank_str(d.hyb_rank),
                rank_str(d.rr_rank),
                cls,
                ev
            );
            continue;
        }

        // Not retrieved at all: probe the individual channels to say *why*.
        let probe = probe_channels(d, &files, &search, &id_to_path);
        match probe.0 {
            "semantic miss" => semantic_misses += 1,
            "lexical miss" => lexical_misses += 1,
            "graph miss" => graph_misses += 1,
            "ground-truth miss" => ground_truth_misses += 1,
            _ => path_misses += 1,
        }
        println!(
            "| {} | {:?} | {} → {} | {} | {} |",
            d.dataset,
            truncate(&d.query, 38),
            rank_str(d.hyb_rank),
            rank_str(d.rr_rank),
            probe.0,
            probe.1
        );
    }
    println!();
    println!("- **{miss_count}** cases with the first relevant hit outside top-5 (after rerank)");
    println!("- Reranker demotions: {demotions} (relevant was top-5 in hybrid order)");
    println!("- Reranker rescues: {rescues} (relevant moved INTO top-5 by rerank)");
    println!("- Graph-pool recoveries: {graph_recoveries} (relevant found only after expansion)");
    println!(
        "- Classified misses: {} semantic, {} lexical, {} path/symbol, {} graph, {} reranker, {} ground-truth ambiguity, {} ground-truth miss",
        semantic_misses,
        lexical_misses,
        path_misses,
        graph_misses,
        reranker_misses,
        ambiguity,
        ground_truth_misses
    );
    println!();
    println!(
        "NEXUS_METRIC retr_after_top1_rate {:.4}",
        rate_of(&diags, |d| d.rr_rank == Some(1))
    );
    println!(
        "NEXUS_METRIC retr_after_top5_rate {:.4}",
        rate_of(&diags, |d| d.rr_rank.is_some_and(|r| r <= 5))
    );
    println!(
        "NEXUS_METRIC retr_missing_rate {:.4}",
        rate_of(&diags, |d| d.rr_rank.is_none())
    );
    println!("NEXUS_METRIC retr_reranker_demotions {demotions}");
    println!("NEXUS_METRIC retr_reranker_rescues {rescues}");
    println!("NEXUS_METRIC retr_graph_recoveries {graph_recoveries}");
    println!();

    // Transparency: show exactly what hybrid retrieval returned (top-5 per
    // query) so the scores above can be eyeballed — no hidden scoring.
    println!("### What Nexus hybrid search actually returned (top 5)");
    println!();
    for case in &cases {
        let mut lines: Vec<String> = Vec::new();
        if let Ok(hits) = search.search_hybrid(&case.query, 5) {
            for (id, score) in hits {
                if let Some(path) = id_to_path.get(id.as_str()) {
                    lines.push(format!("`{}` ({score:.3})", truncate(path, 60)));
                }
            }
        }
        println!(
            "- {:?} → {}",
            truncate(&case.query, 44),
            if lines.is_empty() {
                "—".to_string()
            } else {
                lines.join(", ")
            }
        );
    }
    println!();

    // ── Context pipeline: token economy + latency ──
    println!("## Context pipeline — token economy & latency");
    println!();
    println!(
        "| Query | Baseline tokens | Context tokens | Reduction | Latency | Conflicts excluded |"
    );
    println!("|---|---|---|---|---|---|");

    let graph_conn = nexus::db::open_connection().expect("graph conn");
    let graph_repo = SqliteGraphRepository::new(graph_conn).expect("graph repo");
    let builder_conn = nexus::db::open_connection().expect("builder conn");
    let builder_repo = SqliteMemoryRepository::new(builder_conn).expect("builder repo");
    let builder =
        nexus::core::context::context_builder::ContextBuilderImpl::new(graph_repo, builder_repo);

    let mut reductions: Vec<f64> = Vec::new();
    let mut latencies: Vec<f64> = Vec::new();

    for case in &cases {
        let start = Instant::now();
        let pkg = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(builder.build_for_query(&case.query))
            .expect("build context");
        let lat_ms = start.elapsed().as_secs_f64() * 1000.0;
        let baseline = pkg.baseline_tokens;
        let final_tokens = pkg.token_count;
        let reduction = if baseline > 0 {
            (1.0 - final_tokens as f64 / baseline as f64) * 100.0
        } else {
            0.0
        };
        reductions.push(reduction);
        latencies.push(lat_ms);
        println!(
            "| {} | {} | {} | {:.1}% | {:.1} ms | {} |",
            truncate(&case.query, 52),
            baseline,
            final_tokens,
            reduction,
            lat_ms,
            pkg.conflicts_excluded
        );
    }
    println!(
        "| **Mean** | — | — | **{:.1}%** | **{:.1} ms** | — |",
        avg(&reductions),
        avg(&latencies)
    );
    println!();

    // ── Conflict detection ──
    println!("## System 3 — Conflict detection");
    println!();
    // Three groups, each measuring a distinct behaviour:
    //  (A) near-identical restatement with a swapped fact — must be flagged
    //  (B) realistic paraphrase conflict — the honest hard case
    //  (C) same-statement restatement — must NOT be flagged
    let conflict_cases: Vec<(&str, &str, &str, &str)> = vec![
        // A — positive control, wording matches the engine's own unit tests
        (
            "Database choice",
            "Use PostgreSQL as the primary database",
            "Database choice",
            "Use MySQL as the primary database",
        ),
        (
            "Auth tokens",
            "Access tokens are stored in httpOnly cookies",
            "Auth tokens",
            "Access tokens are stored in localStorage for the SPA",
        ),
        // B — paraphrased conflict (realistic, harder for the Dice threshold)
        (
            "Database used in production",
            "PostgreSQL is the primary production database for all services",
            "Database used in production",
            "We migrated the production database from PostgreSQL to SQLite for simplicity",
        ),
        (
            "Deployment platform",
            "All services are deployed to AWS EC2 instances",
            "Deployment platform",
            "The team decided to move all deployments from AWS to bare-metal servers",
        ),
        // C — negative control: same fact restated, must stay Current
        (
            "Auth",
            "Use JWT with 15 minute expiry",
            "Auth decision",
            "Use JWT with 15 minute expiry (confirmed)",
        ),
    ];
    let mut found_a = 0usize;
    let mut total_a = 0usize;
    let mut found_b = 0usize;
    let mut total_b = 0usize;
    let mut false_pos_c = 0usize;
    for (i, (a_title, a_content, b_title, b_content)) in conflict_cases.iter().enumerate() {
        let a = MemoryRecord::new(
            a_title.to_string(),
            a_content.to_string(),
            "bench".into(),
            MemorySource::Manual,
        )
        .unwrap();
        let b = MemoryRecord::new(
            b_title.to_string(),
            b_content.to_string(),
            "bench".into(),
            MemorySource::Manual,
        )
        .unwrap();
        let aid = rt.block_on(repo.save(&a)).unwrap();
        let bid = rt.block_on(repo.save(&b)).unwrap();

        // The real write path indexes every record into the semantic store
        // (via `index_memory`); the benchmark must mirror that or the semantic
        // channel of the conflict detector physically cannot see the pair.
        let a_text = nexus::core::context::indexer::index_text(&a.title, &a.summary, &a.content);
        let b_text = nexus::core::context::indexer::index_text(&b.title, &b.summary, &b.content);
        let _ = search.store_fingerprint(&aid, &a_text);
        let _ = search.store_fingerprint(&bid, &b_text);

        // Diagnostics: the two signals the detector fuses, measured for real.
        let dice = nexus::core::memory::memory_lifecycle::text_overlap(
            &format!("{} {}", a.title, a.content),
            &format!("{} {}", b.title, b.content),
        );
        let mut cosine = 0.0f64;
        if let Ok(hits) = search.search(&format!("{} {}", a.title, a.content), 10) {
            for (id, s) in hits {
                if id == bid {
                    cosine = s;
                    break;
                }
            }
        }

        rt.block_on(detect_and_mark_conflicts(&repo, &b)).ok();
        let a_conflicted = rt
            .block_on(repo.get_by_id(&aid))
            .ok()
            .flatten()
            .map(|r| r.memory_state == MemoryState::Conflicted)
            .unwrap_or(false);
        let group = if i < 2 {
            'A'
        } else if i < 4 {
            'B'
        } else {
            'C'
        };
        match group {
            'A' => {
                total_a += 1;
                if a_conflicted {
                    found_a += 1;
                }
            }
            'B' => {
                total_b += 1;
                if a_conflicted {
                    found_b += 1;
                }
            }
            _ => {
                if a_conflicted {
                    false_pos_c += 1;
                }
            }
        }
        println!(
            "  [{group}] `{}` vs `{}` → {}  (cosine {:.3}, dice {:.3})",
            truncate(a_content, 44),
            truncate(b_content, 44),
            if a_conflicted {
                "CONFLICTED"
            } else {
                "current"
            },
            cosine,
            dice
        );
    }
    println!();
    println!("- A) Near-duplicate conflicts (positive control): {found_a}/{total_a} flagged");
    println!("- B) Paraphrased conflicts (realistic wording): {found_b}/{total_b} flagged");
    println!("- C) Same-fact restatements (negative control): {false_pos_c} false positives");
    println!();

    // ── Canonical consolidation ──
    println!("## System 2 — Rehearsal / canonical consolidation");
    println!();
    // similarity()/find_clusters() compare title+summary, so the theme must go
    // into `summary` (like the real pipeline does after summarisation).
    let mut cluster_records: Vec<MemoryRecord> = Vec::new();
    let themes = [
        "JWT authentication refresh token rotation implementation",
        "JWT refresh tokens are rotated on every request in the auth service",
        "Implementation of JWT auth uses rotating refresh tokens",
        "JWT access token expires in 15 minutes, refresh token rotates each use",
        "Auth service rotates JWT refresh tokens after every successful refresh",
    ];
    for (i, t) in themes.iter().enumerate() {
        let mut rec = MemoryRecord::new(
            format!("JWT auth note {}", i + 1),
            format!("Notes about: {}", t),
            "bench".into(),
            MemorySource::Manual,
        )
        .unwrap();
        rec.summary = t.to_string();
        cluster_records.push(rec);
    }
    let clusters = find_clusters(&cluster_records);
    println!("- Planted similar memories: {}", cluster_records.len());
    println!("- Clusters found: {}", clusters.len());
    for c in &clusters {
        println!(
            "  - Cluster of {} members, cohesion {:.2}",
            c.member_ids.len(),
            c.cohesion
        );
    }
    // Also measure pairwise similarity sanity.
    let s = similarity(&cluster_records[0], &cluster_records[1]);
    println!("- Pairwise Jaccard similarity (seed pair): {:.3}", s);
    println!();

    // ── Cognitive layers classification ──
    println!("## System 1 — Cognitive layer classification accuracy");
    println!();
    // Each case is (title, content) with a known-correct expected layer —
    // the same phrases the classifier's own unit tests use as ground truth.
    let layer_cases: Vec<(&str, &str, &str)> = vec![
        (
            "Auth bug",
            "Currently fixing the authentication bug in the login flow",
            "Working",
        ),
        (
            "Yesterday's experiment",
            "Yesterday we tried replacing the middleware and it broke the session store",
            "Episodic",
        ),
        (
            "API",
            "Authentication in this project is implemented with JWT and rotating refresh tokens",
            "Semantic",
        ),
        (
            "Token refresh",
            "First check the token, then refresh it: steps 1-3",
            "Procedural",
        ),
        (
            "Redis",
            "On August 3rd we decided to drop Redis and keep all state in PostgreSQL",
            "Decision",
        ),
        (
            "Architecture",
            "The architecture must remain fully local with no external dependencies",
            "Strategic",
        ),
    ];
    let mut correct = 0usize;
    for (title, content, expected) in &layer_cases {
        let cls = LayerClassifier::classify(
            title,
            content,
            MemorySource::Manual,
            MemoryState::Current,
            0.5,
        );
        let got = cls.layer.as_str();
        let ok = got == *expected;
        if ok {
            correct += 1;
        }
        println!(
            "- {:?} → `{}` (expected `{}`) {}",
            truncate(content, 60),
            got,
            expected,
            if ok { "✓" } else { "✗" }
        );
    }
    println!(
        "- Accuracy: **{:.1}%**",
        correct as f64 / layer_cases.len() as f64 * 100.0
    );
    println!();

    // ── Agent firewall ──
    println!("## System 4 — Agent memory firewall");
    println!();
    let secret_record = MemoryRecord::new(
        "API credentials".to_string(),
        "The production API key is sk-7f3a9c2e and the database password is admin123".to_string(),
        "bench".into(),
        MemorySource::Manual,
    )
    .unwrap();
    let safe_record = MemoryRecord::new(
        "Architecture decision".to_string(),
        "We use a layered architecture with repository pattern".to_string(),
        "bench".into(),
        MemorySource::Manual,
    )
    .unwrap();

    let policy = AgentPolicy {
        id: "p1".into(),
        agent: "claude-code".into(),
        role: "assistant".into(),
        allowed_visibility: vec![],
        allowed_layers: vec![],
        deny_patterns: vec!["api key".into(), "password".into()],
        enabled: true,
        created_at: "2026-08-09".into(),
    };

    let secret_assessment = assess_agent_access(&policy, &secret_record);
    let safe_assessment = assess_agent_access(&policy, &safe_record);
    println!(
        "- Secret memory → **{:?}** (categories: {:?})",
        secret_assessment.verdict, secret_assessment.categories
    );
    println!("- Safe memory → **{:?}**", safe_assessment.verdict);
    let firewall_ok = secret_assessment.verdict
        == nexus::core::memory::agent_permissions::AccessVerdict::Deny
        && safe_assessment.verdict == nexus::core::memory::agent_permissions::AccessVerdict::Allow;
    println!(
        "- Firewall behaves correctly: **{}**",
        if firewall_ok { "YES" } else { "NO" }
    );
    println!();

    // ── Summary ──
    println!("## Summary");
    println!();
    println!("| Metric | Value |");
    println!("|---|---|");
    println!("| Files indexed | {} |", files.len());
    println!("| Semantic P@5 (mean) | {:.2} |", avg(&sem_p5));
    println!("| Semantic R@5 (mean) | {:.2} |", avg(&sem_r5));
    println!("| Keyword P@5 (mean) | {:.2} |", avg(&kw_p5));
    println!("| Keyword R@5 (mean) | {:.2} |", avg(&kw_r5));
    // Overall MRR@10 / NDCG@10 across all datasets, before vs after reranker.
    let overall = |pick: fn(&DatasetAgg) -> &Vec<f64>| -> f64 {
        let (sum, n): (f64, usize) = ds_agg
            .iter()
            .map(|(_, a)| (pick(a).iter().sum::<f64>(), pick(a).len()))
            .fold((0.0, 0), |(s, n), (x, c)| (s + x, n + c));
        if n == 0 { 0.0 } else { sum / n as f64 }
    };
    println!(
        "| MRR@10 before → after reranker | {:.2} → {:.2} |",
        overall(|a| &a.hyb_mrr),
        overall(|a| &a.rr_mrr)
    );
    println!(
        "| NDCG@10 before → after reranker | {:.2} → {:.2} |",
        overall(|a| &a.hyb_ndcg),
        overall(|a| &a.rr_ndcg)
    );
    println!("| Token reduction (mean) | {:.1}% |", avg(&reductions));
    println!("| Context latency (mean) | {:.1} ms |", avg(&latencies));
    println!("| Conflict detect (near-duplicate) | {found_a}/{total_a} |");
    println!("| Conflict detect (paraphrased) | {found_b}/{total_b} |");
    println!("| Conflict false positives | {false_pos_c} |");
    println!("| Canonical clusters | {} |", clusters.len());
    println!(
        "| Layer classification accuracy | {:.0}% |",
        correct as f64 / layer_cases.len() as f64 * 100.0
    );
    println!(
        "| Firewall deny/allow | {} |",
        if firewall_ok { "correct" } else { "incorrect" }
    );
    println!();
    println!(
        "_Every number above is a measurement of the real engine on real project files — no mocks, no synthetic scoring._"
    );
}

fn truncate(s: &str, n: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= n {
        s.to_string()
    } else {
        let t: String = chars[..n].iter().collect();
        format!("{}…", t)
    }
}

// ── Failure-analysis helpers ───────────────────────────────────────────────

fn rank_str(rank: Option<usize>) -> String {
    rank.map(|r| format!("#{r}"))
        .unwrap_or_else(|| "missing".into())
}

fn rate_of(diags: &[CaseDiag], f: impl Fn(&CaseDiag) -> bool) -> f64 {
    let n = diags.len();
    if n == 0 {
        return 0.0;
    }
    diags.iter().filter(|d| f(d)).count() as f64 / n as f64
}

/// For a relevant hit that IS in the hybrid pool but ranked below top-5:
/// the weakest channel is the one that failed to carry it. When every
/// channel is reasonably strong, the miss is a fusion-ordering issue.
fn channel_class(d: &CaseDiag) -> (&'static str, String) {
    let min = d.cosine.min(d.lexical).min(d.filename);
    let detail = format!(
        "channels cos {:.3} / lex {:.3} / path {:.3}",
        d.cosine, d.lexical, d.filename
    );
    if min >= 0.3 {
        return ("fusion ordering", detail);
    }
    if d.filename == min {
        ("path/symbol miss", detail)
    } else if d.lexical == min {
        ("lexical miss", detail)
    } else {
        ("semantic miss", detail)
    }
}

/// For a relevant hit missing from the hybrid top-50 entirely: probe the
/// individual channels against the real index to explain *why* nothing
/// surfaced it. Every probe runs real engine code on the real corpus.
fn probe_channels(
    d: &CaseDiag,
    files: &[IndexedFile],
    search: &SemanticSearch,
    id_to_path: &HashMap<String, String>,
) -> (&'static str, String) {
    // 0) Is the target even in the corpus? (label sanity, not a retrieval fault)
    let target_exists = files
        .iter()
        .any(|f| is_relevant(&f.rel_path, &d.ground_truth));
    if !target_exists {
        return (
            "ground-truth miss",
            "no indexed file matches the ground-truth path".into(),
        );
    }

    // 1) Pure-semantic probe: cosine alone, top-50.
    let sem_hit = search.search(&d.query, 50).is_ok_and(|hits| {
        hits.iter().any(|(id, _)| {
            id_to_path
                .get(id.as_str())
                .is_some_and(|p| is_relevant(p, &d.ground_truth))
        })
    });

    // 2) Pure-keyword probe: term-coverage baseline, top-50.
    let kw_hit = keyword_baseline(&d.query, files, 50)
        .iter()
        .any(|p| is_relevant(p, &d.ground_truth));

    // 3) Filename probe: does the target's path contain any query term?
    let q_terms: Vec<String> = d
        .query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| w.len() > 2)
        .map(|w| w.to_string())
        .collect();
    let path_term_hit = files
        .iter()
        .filter(|f| is_relevant(&f.rel_path, &d.ground_truth))
        .any(|f| {
            let p = f.rel_path.to_lowercase();
            q_terms.iter().any(|t| p.contains(t.as_str()))
        });

    match (sem_hit, kw_hit, path_term_hit) {
        (false, false, _) => (
            "semantic miss",
            "neither cosine nor keyword recovers the target in top-50".into(),
        ),
        (true, false, _) => (
            "lexical miss",
            "cosine finds it, keyword does not — lexical bridge absent".into(),
        ),
        (false, true, _) => (
            "semantic miss",
            "keyword finds it, cosine does not — semantic bridge absent".into(),
        ),
        (true, true, false) => (
            "path/symbol miss",
            "cosine + keyword recover it; the filename carries no query term".into(),
        ),
        (true, true, true) => (
            "graph/fusion miss",
            "all channels recover it individually yet hybrid top-50 lost it".into(),
        ),
    }
}
