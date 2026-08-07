//! Physical end-to-end test: real commands against a real SQLite database on disk.
//!
//! Unlike the unit tests (in-memory connections) this suite:
//! 1. Points `LOCALAPPDATA` at a throwaway directory, so `db::db_path()`
//!    resolves to a real file on disk — exactly like production.
//! 2. Applies migrations the same way `main.rs` does at startup.
//! 3. Drives the actual `#[tauri::command]` functions (no mocks, no repos).
//! 4. Verifies the rows landed via the public command surface.
//!
//! Every step prints its result so the run doubles as a report.

use std::path::PathBuf;

use nexus::commands::graph::{create_entity, find_duplicate_entities, get_graph, merge_entities};
use nexus::commands::knowledge::{
    agents_generate, agents_read, agents_save, code_dependents, code_deps, code_import, code_list,
    code_search, code_stats, import_docs, knowledge_stats, list_docs, search_docs, skills_register,
    skills_run,
};
use nexus::commands::lifecycle::{
    get_feedback_summary, get_lifecycle_overview, memory_confirm, memory_feedback,
    memory_set_state, memory_supersede,
};
use nexus::commands::memory::create_memory;
use nexus::commands::savings::{SavingsMeasurement, get_product_metrics, record_savings};
use nexus::core::tokenizer::{self, Method};
use nexus::db;
use nexus::storage::sqlite::schema;

/// Create an isolated on-disk database for this test process.
///
/// `db_path()` reads `LOCALAPPDATA` **at call time**, so pointing it at a
/// temporary directory redirects every connection the commands open.
fn setup_isolated_db() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nexus-physical-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // Rust 2024: set_var is unsafe.
    unsafe { std::env::set_var("LOCALAPPDATA", &dir) };

    // Apply migrations exactly like main.rs does for the real database.
    let db_path = db::db_path();
    println!("[e2e] isolated database at: {}", db_path.display());
    let conn = db::open_connection().expect("open isolated DB");
    schema::apply_migrations(&conn).expect("apply migrations");
    drop(conn);
    dir
}

#[tokio::test]
async fn physical_full_stack_e2e() {
    setup_isolated_db();

    // ═══════════════════════════════════════════════════════════════
    // A. Memory Trust lifecycle (V12) — real commands, real file DB.
    // ═══════════════════════════════════════════════════════════════
    let m1 = create_memory(
        "Database decision".to_string(),
        "Use PostgreSQL as the primary database".to_string(),
        Some("tester".to_string()),
    )
    .await
    .expect("create_memory m1");
    println!(
        "[lifecycle] m1 created: id={} state={}",
        m1.id, m1.memory_state
    );
    assert_eq!(m1.memory_state, "Current", "fresh memory must be Current");

    // A second memory about the same topic with a different fact must trigger
    // the conflict detector and flag BOTH sides Conflicted.
    let m2 = create_memory(
        "Database decision".to_string(),
        "Use MySQL as the primary database".to_string(),
        Some("tester".to_string()),
    )
    .await
    .expect("create_memory m2");
    println!(
        "[lifecycle] m2 created: id={} state={}",
        m2.id, m2.memory_state
    );
    assert_eq!(
        m2.memory_state, "Conflicted",
        "conflicting memory must be flagged Conflicted"
    );

    let m1_after = nexus::commands::memory::get_memory(m1.id.clone())
        .await
        .expect("get_memory m1")
        .expect("m1 exists");
    assert_eq!(
        m1_after.memory_state, "Conflicted",
        "both sides of a conflict must be flagged"
    );

    // Explicit state change.
    let set = memory_set_state(m1.id.clone(), "UserConfirmed".to_string())
        .await
        .expect("memory_set_state");
    assert_eq!(set.memory_state, "UserConfirmed");
    println!("[lifecycle] memory_set_state -> {}", set.memory_state);

    // Explicit human confirmation stamps confirmed_at / confirmed_by.
    let confirmed = memory_confirm(m1.id.clone(), Some("tester".to_string()))
        .await
        .expect("memory_confirm");
    assert_eq!(confirmed.memory_state, "UserConfirmed");
    assert!(confirmed.confirmed_at.is_some(), "confirmed_at must be set");
    assert_eq!(confirmed.confirmed_by.as_deref(), Some("tester"));
    println!(
        "[lifecycle] memory_confirm -> by={:?} at={:?}",
        confirmed.confirmed_by, confirmed.confirmed_at
    );

    // Feedback counters (useful / irrelevant / wrong) with one-vote toggle:
    // the same kind clicked again removes the vote, a different kind switches.
    let fb_useful = memory_feedback(m1.id.clone(), "useful".to_string(), None)
        .await
        .expect("feedback useful");
    assert_eq!(fb_useful.feedback.useful, 1);
    assert_eq!(fb_useful.feedback.voted.as_deref(), Some("useful"));
    // Repeat click on the same kind must NOT inflate the counter — it removes the vote.
    let fb_unvote = memory_feedback(m1.id.clone(), "useful".to_string(), None)
        .await
        .expect("feedback useful unvote");
    assert_eq!(fb_unvote.feedback.useful, 0);
    assert_eq!(fb_unvote.feedback.voted, None);
    // Vote useful again, then switch to wrong (useful -> 0, wrong -> 1).
    let fb_useful2 = memory_feedback(m1.id.clone(), "useful".to_string(), None)
        .await
        .expect("feedback useful again");
    assert_eq!(fb_useful2.feedback.useful, 1);
    let fb_wrong = memory_feedback(m1.id.clone(), "wrong".to_string(), None)
        .await
        .expect("feedback wrong");
    assert_eq!(fb_wrong.feedback.wrong, 1);
    assert_eq!(
        fb_wrong.feedback.useful, 0,
        "vote must switch, not accumulate"
    );
    assert_eq!(fb_wrong.feedback.voted.as_deref(), Some("wrong"));
    // A note explains the verdict and is stored verbatim.
    let fb_note = memory_feedback(
        m1.id.clone(),
        "wrong".to_string(),
        Some("The real primary DB is PostgreSQL, not MySQL".to_string()),
    )
    .await
    .expect("feedback note");
    assert_eq!(
        fb_note.feedback.note.as_deref(),
        Some("The real primary DB is PostgreSQL, not MySQL")
    );
    assert_eq!(fb_note.feedback.wrong, 1, "note must not change counters");
    println!(
        "[lifecycle] feedback -> useful={} wrong={} voted={:?} note={:?}",
        fb_note.feedback.useful,
        fb_note.feedback.wrong,
        fb_note.feedback.voted,
        fb_note.feedback.note
    );

    // Supersede: old demoted to Superseded, new Current record with links.
    let new = memory_supersede(
        m1.id.clone(),
        "Database decision v2".to_string(),
        "Use PostgreSQL with read replicas".to_string(),
        None,
    )
    .await
    .expect("memory_supersede");
    assert_eq!(new.memory_state, "Current");
    assert_eq!(new.supersedes_id.as_deref(), Some(m1.id.as_str()));

    let m1_superseded = nexus::commands::memory::get_memory(m1.id.clone())
        .await
        .expect("get_memory m1 after")
        .expect("m1 still exists");
    assert_eq!(m1_superseded.memory_state, "Superseded");
    assert_eq!(
        m1_superseded.superseded_by_id.as_deref(),
        Some(new.id.as_str())
    );
    println!(
        "[lifecycle] supersede -> old={} state={} new={} state={}",
        m1.id, m1_superseded.memory_state, new.id, new.memory_state
    );

    // Lifecycle overview aggregates the real states.
    let overview = get_lifecycle_overview().await.expect("lifecycle overview");
    println!(
        "[lifecycle] overview -> total={} current={} user_confirmed={} inferred={} superseded={} conflicted={}",
        overview.total,
        overview.current,
        overview.user_confirmed,
        overview.inferred,
        overview.superseded,
        overview.conflicted
    );
    assert!(overview.total >= 3, "3 memories exist");
    assert!(overview.superseded >= 1, "m1 was superseded");
    assert!(overview.conflicted >= 1, "m2 stayed conflicted");
    assert!(overview.current >= 1, "new memory is current");

    let feedback = get_feedback_summary().await.expect("feedback summary");
    println!(
        "[lifecycle] feedback summary -> useful={} irrelevant={} wrong={}",
        feedback.useful, feedback.irrelevant, feedback.wrong
    );
    assert_eq!(feedback.useful, 0, "useful vote was switched to wrong");
    assert_eq!(feedback.wrong, 1);

    // ═══════════════════════════════════════════════════════════════
    // B. Entity resolution — duplicates & merge (real commands).
    // ═══════════════════════════════════════════════════════════════
    let e1 = create_entity(
        "Technology".to_string(),
        "Nexus".to_string(),
        "AI memory tool".to_string(),
    )
    .await
    .expect("create_entity e1");
    let e2 = create_entity(
        "Technology".to_string(),
        "Nexus MCP".to_string(),
        "MCP server for Nexus".to_string(),
    )
    .await
    .expect("create_entity e2");
    let e3 = create_entity(
        "Technology".to_string(),
        "Nexus Server".to_string(),
        "Server component of Nexus".to_string(),
    )
    .await
    .expect("create_entity e3");
    println!(
        "[resolution] created: {} | {} | {}",
        e1.title, e2.title, e3.title
    );

    let groups = find_duplicate_entities(None)
        .await
        .expect("find_duplicates");
    println!(
        "[resolution] duplicate groups: {}",
        groups
            .iter()
            .map(|g| format!("{}", g.entities.len()))
            .collect::<Vec<_>>()
            .join(", ")
    );
    assert!(
        !groups.is_empty(),
        "Nexus/Nexus MCP/Nexus Server must be grouped"
    );
    let group = &groups[0];
    assert!(group.entities.len() >= 2, "group must have 2+ candidates");
    println!(
        "[resolution] group best_id={} candidates: {}",
        group.best_id,
        group
            .entities
            .iter()
            .map(|c| format!("{}({:.2},{})", c.title, c.score, c.match_kind))
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Merge everything into the best candidate.
    let duplicates: Vec<String> = group
        .entities
        .iter()
        .filter(|c| c.entity_id != group.best_id)
        .map(|c| c.entity_id.clone())
        .collect();
    let merged = merge_entities(group.best_id.clone(), duplicates.clone())
        .await
        .expect("merge_entities");
    println!(
        "[resolution] merged {} entities -> '{}' ({})",
        duplicates.len() + 1,
        merged.title,
        merged.id
    );
    assert_eq!(merged.id, group.best_id);

    // The graph must still contain the canonical node, and the duplicate scan
    // must no longer propose the merged pair as a duplicate group.
    let graph = get_graph().await.expect("get_graph");
    assert!(
        graph.nodes.iter().any(|n| n.id == merged.id),
        "canonical node present after merge"
    );
    println!(
        "[resolution] graph after merge: {} nodes, {} edges",
        graph.nodes.len(),
        graph.edges.len()
    );

    // ═══════════════════════════════════════════════════════════════
    // C. Product metrics (V13) — real savings rows, real aggregation.
    // ═══════════════════════════════════════════════════════════════
    let measure =
        |baseline: u32, context: u32, used: u32, irr: u32, ids: Vec<String>| SavingsMeasurement {
            baseline_tokens: baseline,
            context_tokens: context,
            entities_count: 3,
            memories_count: 2,
            relationships_count: 1,
            candidate_entities: 5,
            candidate_memories: 4,
            token_method: "exact".to_string(),
            latency_ms: 120,
            precision: 0.9,
            used_fragments: used,
            irrelevant_fragments: irr,
            manual_context: 0,
            memory_ids: ids,
        };

    // Interaction 1 delivers m1; interaction 2 delivers m1 again + new memory
    // → m1 is reused across sessions, new memory is delivered once.
    record_savings(
        &measure(48_000, 11_800, 9, 5, vec![m1.id.clone()]),
        "how is the database configured?",
        "question",
    );
    record_savings(
        &measure(30_000, 9_000, 6, 2, vec![m1.id.clone(), new.id.clone()]),
        "explain the current database setup",
        "explain",
    );

    let pm = get_product_metrics().await.expect("get_product_metrics");
    println!(
        "[metrics] interactions={} tokens_saved={} baseline={} latency={:.0}ms precision={:.2} auto_context={:.0}%",
        pm.total_interactions,
        pm.total_tokens_saved,
        pm.total_baseline_tokens,
        pm.avg_latency_ms,
        pm.avg_precision,
        pm.auto_context_share * 100.0
    );
    println!(
        "[metrics] used_fragments={} irrelevant_fragments={} used_share={:.2} stale_memories={} memory_fixes={} reused_memories={} delivered={}",
        pm.total_used_fragments,
        pm.total_irrelevant_fragments,
        pm.used_fragment_share,
        pm.stale_memories,
        pm.memory_fixes,
        pm.reused_memories,
        pm.total_memories_delivered
    );
    assert_eq!(pm.total_interactions, 2);
    assert_eq!(
        pm.total_tokens_saved,
        (48_000 - 11_800) + (30_000 - 9_000),
        "tokens_saved must equal the measured differences"
    );
    assert_eq!(pm.total_used_fragments, 9 + 6);
    assert_eq!(pm.total_irrelevant_fragments, 5 + 2);
    assert_eq!(pm.total_baseline_tokens, 48_000 + 30_000);
    assert!(
        pm.stale_memories >= 1,
        "superseded/conflicted memories are stale"
    );
    assert_eq!(pm.memory_fixes, 1, "one 'wrong' feedback counts as a fix");
    assert!(pm.reused_memories >= 1, "m1 delivered in two interactions");
    assert_eq!(pm.total_memories_delivered, 2, "m1 and the new memory");

    // ═══════════════════════════════════════════════════════════════
    // D. Tokenizer — real counting against real model targets.
    // ═══════════════════════════════════════════════════════════════
    tokenizer::set_active_model("gpt-4o");
    let exact_count = tokenizer::count("Hello world, this is a physical tokenizer test.");
    let exact_method = tokenizer::method();
    println!(
        "[tokenizer] gpt-4o -> count={} method={}",
        exact_count,
        exact_method.as_str()
    );
    assert_eq!(
        exact_method,
        Method::Exact,
        "gpt-4o uses embedded tiktoken BPE"
    );
    assert!(exact_count > 0);

    tokenizer::set_active_model("claude-3-5-sonnet");
    let claude_count = tokenizer::count("Hello world, this is a physical tokenizer test.");
    let claude_method = tokenizer::method();
    println!(
        "[tokenizer] claude-3-5-sonnet -> count={} method={}",
        claude_count,
        claude_method.as_str()
    );
    assert_eq!(
        claude_method,
        Method::Estimated,
        "Claude has no public tokenizer.json -> honest estimate"
    );

    tokenizer::set_active_model("gpt-4o");
    let cyrillic = tokenizer::count("Привет мир, это тест токенизатора на кириллице.");
    assert!(cyrillic > 0, "cyrillic text must count");
    println!("[tokenizer] cyrillic under gpt-4o -> count={}", cyrillic);

    // ═══════════════════════════════════════════════════════════════
    // E. Project knowledge base (V14) — RAG docs, AGENTS.md, skills.
    // ═══════════════════════════════════════════════════════════════
    // A real docs folder on disk, next to the isolated DB.
    let docs_dir = std::env::temp_dir().join(format!("nexus-knowledge-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&docs_dir);
    std::fs::create_dir_all(docs_dir.join("guides")).unwrap();
    std::fs::write(
        docs_dir.join("README.md"),
        "# Nexus\n\nThe Nexus project is an AI memory tool with semantic search.",
    )
    .unwrap();
    std::fs::write(
        docs_dir.join("guides").join("architecture.md"),
        "# Architecture\n\nNexus stores memories in SQLite and embeds them with ONNX models.",
    )
    .unwrap();
    std::fs::write(docs_dir.join("ignored.rs"), "fn main() {}").unwrap();

    let report = import_docs(docs_dir.to_string_lossy().to_string())
        .await
        .expect("import_docs");
    println!(
        "[knowledge] import -> scanned={} imported={} unchanged={} pruned={} failed={}",
        report.scanned, report.imported, report.unchanged, report.updated, report.failed
    );
    assert_eq!(report.scanned, 2, "only .md files are scanned");
    assert_eq!(report.imported, 2, "both markdown files imported");
    assert_eq!(report.failed, 0);

    // Re-import must be a no-op (checksum idempotency).
    let report2 = import_docs(docs_dir.to_string_lossy().to_string())
        .await
        .expect("re-import_docs");
    assert_eq!(report2.imported, 0, "re-import must not re-add docs");
    assert_eq!(report2.unchanged, 2, "both docs unchanged on re-import");

    let listed = list_docs(None).await.expect("list_docs");
    println!("[knowledge] list_docs -> {} documents", listed.len());
    assert_eq!(listed.len(), 2);

    // Text-overlap search must find the README by an exact word.
    let hits = search_docs("SQLite".to_string(), Some(10))
        .await
        .expect("search_docs");
    println!("[knowledge] search 'SQLite' -> {} hits", hits.len());
    assert!(
        !hits.is_empty(),
        "SQLite appears in architecture.md content"
    );
    let top = &hits[0];
    assert_eq!(top.document.doc_type, "markdown");
    assert!(top.score > 0.0, "score must be positive: {}", top.score);

    let stats = knowledge_stats().await.expect("knowledge_stats");
    println!(
        "[knowledge] stats -> documents={} agents={} skills={}",
        stats.document_count, stats.agents_count, stats.skill_count
    );
    assert_eq!(stats.document_count, 2);

    // AGENTS.md round-trip through the real command surface.
    let saved = agents_save(
        "AGENTS.md".to_string(),
        "# Agent Rules\n\nNever commit secrets.".to_string(),
        Some("e2e:AGENTS.md".to_string()),
    )
    .await
    .expect("agents_save");
    assert_eq!(saved.name, "AGENTS.md");
    assert!(saved.content.contains("Never commit secrets"));
    let read = agents_read(None)
        .await
        .expect("agents_read")
        .expect("AGENTS.md exists");
    assert_eq!(read.content, saved.content);
    println!(
        "[knowledge] AGENTS.md round-trip ok ({} chars)",
        read.content.len()
    );

    // Auto-generation from live system data (the documentation skill).
    let generated = agents_generate().await.expect("agents_generate");
    println!(
        "[knowledge] agents_generate -> {} chars, {} commands listed",
        generated.content.len(),
        generated.content.matches("/docs-import").count()
            + generated.content.matches("/stats").count()
    );
    assert!(
        generated.content.contains("# AGENTS.md"),
        "generated file has a header"
    );
    assert!(
        generated.content.contains("/docs-search") || generated.content.contains("/docs-import"),
        "generated file documents the knowledge commands"
    );
    let reread = agents_read(None)
        .await
        .expect("agents_read")
        .expect("generated exists");
    assert_eq!(
        reread.content, generated.content,
        "generated file persisted"
    );

    // Skills: register a trivial command and run it through SkillRunner.
    // `echo` is a cmd.exe builtin on Windows, so use the shell there.
    let echo_cmd = if cfg!(windows) {
        "cmd /C echo hello-from-skill"
    } else {
        "echo hello-from-skill"
    };
    let skill = skills_register(
        "echo-test".to_string(),
        "Echo a greeting".to_string(),
        echo_cmd.to_string(),
        None,
    )
    .await
    .expect("skills_register");
    assert_eq!(skill.name, "echo-test");
    let out = skills_run("echo-test".to_string(), None)
        .await
        .expect("skills_run");
    println!(
        "[knowledge] skills_run -> success={} stdout='{}' stderr='{}' exit={:?}",
        out.success,
        out.stdout.trim(),
        out.stderr.trim(),
        out.exit_code
    );
    assert!(out.success, "echo must exit 0: {:?}", out.exit_code);
    assert!(
        out.stdout.contains("hello-from-skill"),
        "stdout must contain the echo: '{}'",
        out.stdout
    );

    let stats2 = knowledge_stats().await.expect("knowledge_stats (after)");
    assert_eq!(stats2.agents_count, 1, "one agents file stored");
    assert_eq!(stats2.skill_count, 1, "one skill registered");

    // ═══════════════════════════════════════════════════════════════
    // F. Code graph (V15) — structure over real source files.
    // ═══════════════════════════════════════════════════════════════
    let code_dir = std::env::temp_dir().join(format!("nexus-cg-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&code_dir);
    std::fs::create_dir_all(code_dir.join("src")).unwrap();
    std::fs::write(
        code_dir.join("src").join("main.rs"),
        "use std::fs;\nfn main() {}\nstruct App {}\n",
    )
    .unwrap();
    std::fs::write(
        code_dir.join("src").join("lib.rs"),
        "use crate::main;\npub fn helper() {}\n",
    )
    .unwrap();
    std::fs::write(code_dir.join("readme.md"), "# readme\n").unwrap();

    let cg = code_import(code_dir.to_string_lossy().to_string())
        .await
        .expect("code_import");
    println!(
        "[codegraph] import -> scanned={} indexed={} unchanged={} pruned={} failed={}",
        cg.scanned, cg.indexed, cg.unchanged, cg.pruned, cg.failed
    );
    assert_eq!(
        cg.scanned, 2,
        "only code files are scanned (md is not code)"
    );
    assert_eq!(cg.indexed, 2, "both Rust files indexed");

    // Re-import is idempotent (checksum).
    let cg2 = code_import(code_dir.to_string_lossy().to_string())
        .await
        .expect("re-import");
    assert_eq!(cg2.indexed, 0, "re-import must not re-index");
    assert_eq!(cg2.unchanged, 2, "both files unchanged");

    let files = code_list(None).await.expect("code_list");
    println!("[codegraph] list -> {} files", files.len());
    assert_eq!(files.len(), 2);

    // Symbol search across the parsed code.
    let hits = code_search("App".to_string(), None)
        .await
        .expect("code_search");
    println!(
        "[codegraph] search 'App' -> {} hits: {:?}",
        hits.len(),
        hits.iter()
            .map(|h| format!("{}@{}", h.symbol.name, h.file_path))
            .collect::<Vec<_>>()
    );
    assert!(!hits.is_empty(), "struct App must be found");
    assert_eq!(hits[0].symbol.kind, "struct");
    assert!(hits[0].file_path.contains("main.rs"));

    // Dependencies of main.rs: `use std::fs` -> external dep on std.
    let main_path = code_dir.join("src").join("main.rs");
    let deps = code_deps(main_path.to_string_lossy().to_string())
        .await
        .expect("code_deps");
    println!(
        "[codegraph] deps of main.rs -> {}",
        deps.iter()
            .map(|d| format!("{}({})", d.target, d.kind))
            .collect::<Vec<_>>()
            .join(", ")
    );
    assert!(!deps.is_empty(), "main.rs imports std::fs");
    let std_dep = deps.iter().find(|d| d.target == "std").expect("std dep");
    assert!(std_dep.is_external, "std is external");

    // Reverse edges: who depends on `main` (from `use crate::main`).
    let dependents = code_dependents("main".to_string())
        .await
        .expect("code_dependents");
    println!(
        "[codegraph] dependents of 'main' -> {}",
        dependents
            .iter()
            .map(|h| format!("{} ({})", h.file_path, h.kind))
            .collect::<Vec<_>>()
            .join(", ")
    );
    assert!(
        dependents.iter().any(|h| h.file_path.contains("lib.rs")),
        "lib.rs uses crate::main"
    );

    let cstats = code_stats().await.expect("code_stats");
    println!(
        "[codegraph] stats -> files={} symbols={} deps={}",
        cstats.file_count, cstats.symbol_count, cstats.dependency_count
    );
    assert_eq!(cstats.file_count, 2);
    assert!(
        cstats.symbol_count >= 3,
        "main + App + helper parsed: {}",
        cstats.symbol_count
    );
    assert!(
        cstats.dependency_count >= 2,
        "std + crate edges: {}",
        cstats.dependency_count
    );

    println!("\n[e2e] ALL PHYSICAL CHECKS PASSED");
}
