//! Nexus Conflict Engine v2 Benchmark — deterministic CI gate.
//!
//! Loads `benchmarks/conflict/cases.json`, runs the pure `classify` verdict
//! over every pair of memories and reports, per failure class (paraphrase /
//! negation / numbers / architecture / lexical):
//!
//! - detection rate = correctly-flagged contradictions / all contradictions
//! - false-positive rate = wrongly-flagged non-conflicts / all non-conflicts
//!
//! Gate (plan 2.5): detection ≥ 95%, FP < 2%. Exit code 0/1 drives the CI
//! check. No database, no embedding model — fully deterministic and fast.
//!
//! Run:  cargo run --bin nexus_conflict_bench -- [path/to/cases.json]

use std::path::PathBuf;

use nexus::core::memory::conflict::verdict::{PairVerdict, classify, compare_claims};
use nexus::core::memory::memory_record::MemoryRecord;
use nexus::core::memory::types::MemorySource;

const DETECTION_TARGET: f64 = 0.95;
const FP_TARGET: f64 = 0.02;

#[derive(Debug, serde::Deserialize)]
struct ConflictCase {
    id: String,
    class: String,
    a_title: String,
    a_content: String,
    b_title: String,
    b_content: String,
    expected: String,
    semantic: Option<f64>,
    /// Materialize an explicit `supersedes_id` link for `expected ==
    /// "superseded"`. Defaults to true (the lifecycle link case); set to
    /// false for temporal cases that must be caught by the year/version
    /// channel alone (plan 2.3).
    #[serde(default = "default_link")]
    link: bool,
    /// Version of record B. Defaults to 1; used by the temporal version
    /// channel.
    #[serde(default)]
    b_version: u32,
}

fn default_link() -> bool {
    true
}

#[derive(Debug, serde::Deserialize)]
struct CaseFile {
    cases: Vec<ConflictCase>,
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../benchmarks/conflict/cases.json")
        });

    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(e) => {
            eprintln!("cannot read {}: {e}", path.display());
            std::process::exit(2);
        }
    };
    let file: CaseFile = match serde_json::from_str(&raw) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("cannot parse {}: {e}", path.display());
            std::process::exit(2);
        }
    };

    let mut total_contradictions = 0usize;
    let mut detected_contradictions = 0usize;
    let mut total_non_conflicts = 0usize;
    let mut false_positives = 0usize;

    // Per-class detection for the report (plan 2.5 breakdown).
    let mut class_stats: std::collections::BTreeMap<String, (usize, usize)> =
        std::collections::BTreeMap::new(); // class -> (detected, total)

    println!("## Conflict Engine v2 benchmark");
    println!();
    println!("| Case | Class | Verdict | Expected | OK |");
    println!("|---|---|---|---|---|");

    for case in &file.cases {
        let a = MemoryRecord::new(
            case.a_title.clone(),
            case.a_content.clone(),
            "bench".into(),
            MemorySource::Manual,
        )
        .expect("valid case title/content");
        let mut b = MemoryRecord::new(
            case.b_title.clone(),
            case.b_content.clone(),
            "bench".into(),
            MemorySource::Manual,
        )
        .expect("valid case title/content");

        // Materialize the explicit supersession link from the expected verdict
        // (the engine reads the lifecycle fields on the records themselves),
        // unless the case opts out so the temporal channel is tested purely
        // through year/version signals (plan 2.3).
        if case.expected == "superseded" && case.link {
            b.supersedes_id = Some(a.id.as_str().to_string());
        }
        b.version = case.b_version.max(1);

        let verdict = classify(&a, &b, case.semantic);
        let ok = verdict.as_str() == case.expected;

        if case.expected == "contradicted" {
            total_contradictions += 1;
            if verdict == PairVerdict::Contradicted {
                detected_contradictions += 1;
            }
            let stat = class_stats.entry(case.class.clone()).or_insert((0, 0));
            stat.1 += 1;
            if verdict == PairVerdict::Contradicted {
                stat.0 += 1;
            }
        } else {
            total_non_conflicts += 1;
            if verdict == PairVerdict::Contradicted {
                false_positives += 1;
            }
        }

        let _ = compare_claims(&a, &b); // exercised for side-effect-free signals
        println!(
            "| {} | {} | {} | {} | {} |",
            case.id,
            case.class,
            verdict.as_str(),
            case.expected,
            if ok { "✓" } else { "✗" }
        );
    }

    let detection = if total_contradictions == 0 {
        1.0
    } else {
        detected_contradictions as f64 / total_contradictions as f64
    };
    let fp = if total_non_conflicts == 0 {
        0.0
    } else {
        false_positives as f64 / total_non_conflicts as f64
    };

    println!();
    println!("| Metric | Value | Target |");
    println!("|---|---|---|");
    println!(
        "| Contradiction detection | **{:.1}%** ({}/{}) | ≥ {:.0}% |",
        detection * 100.0,
        detected_contradictions,
        total_contradictions,
        DETECTION_TARGET * 100.0
    );
    println!(
        "| False positives | **{:.1}%** ({}/{}) | < {:.0}% |",
        fp * 100.0,
        false_positives,
        total_non_conflicts,
        FP_TARGET * 100.0
    );
    println!();
    println!("### Detection by class");
    println!();
    println!("| Class | Detected | Total | Rate |");
    println!("|---|---|---|---|");
    for (class, (det, total)) in &class_stats {
        println!(
            "| {} | {} | {} | {:.1}% |",
            class,
            det,
            total,
            *det as f64 / *total as f64 * 100.0
        );
    }

    let pass = detection >= DETECTION_TARGET && fp < FP_TARGET;
    println!();
    println!("GATE: {}", if pass { "PASS" } else { "FAIL" });
    // Machine-readable output for the CI regression gate (plan 6.2).
    println!("NEXUS_METRIC conflict_detection_rate={detection:.4}");
    println!("NEXUS_METRIC conflict_fp_rate={fp:.4}");
    std::process::exit(if pass { 0 } else { 1 });
}
