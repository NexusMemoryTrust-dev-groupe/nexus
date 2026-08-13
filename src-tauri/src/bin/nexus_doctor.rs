//! Nexus Doctor CLI — Production Readiness Gate 0.4.
//!
//! Runs the full battery of health checks against the live database and prints
//! a structured report. Exit code is 0 when healthy, 1 when any check errored.
//!
//! Run:  cargo run --bin nexus_doctor
//!       cargo run --bin nexus_doctor -- --json     (machine-readable report)
//!
//! Environment isolation: LOCALAPPDATA/HOME are read by `db::db_path()`; point
//! them at a temp dir to check a throwaway database instead of the user's.

use nexus::core::doctor::{CheckStatus, run_doctor};

fn main() {
    let json = std::env::args().any(|a| a == "--json");
    let report = run_doctor();

    if json {
        match serde_json::to_string_pretty(&report) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("failed to serialize doctor report: {e}");
                std::process::exit(2);
            }
        }
    } else {
        println!("Nexus Doctor — {}", report.run_at);
        println!("{}", "─".repeat(64));
        for check in &report.checks {
            let mark = match check.status {
                CheckStatus::Ok => "  OK",
                CheckStatus::Warning => "WARN",
                CheckStatus::Error => "FAIL",
            };
            println!("[{mark}] {:<16} {}", check.name, check.message);
        }
        println!("{}", "─".repeat(64));
        let errors = report.error_count();
        println!(
            "Result: {} — {} checks, {} error(s)",
            if report.healthy() {
                "HEALTHY"
            } else {
                "DEGRADED"
            },
            report.checks.len(),
            errors
        );
        if errors > 0 {
            std::process::exit(1);
        }
    }
}
