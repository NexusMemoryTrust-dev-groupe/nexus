//! Diagnostics commands — health report for the UI (Production Readiness 0.5).
//!
//! The UI needs the same view the `nexus doctor` CLI has, plus a portable
//! export for support tickets. Both are intentionally PII-free: the report
//! contains check names, statuses and aggregate counts — never memory titles,
//! contents or paths.

use chrono::Utc;
use serde::Serialize;

use crate::core::doctor::{CheckStatus, DoctorReport, run_doctor};

/// One check, as the UI renders it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCheck {
    pub name: String,
    /// "ok" | "warning" | "error"
    pub status: String,
    pub message: String,
}

/// The full diagnostics snapshot for the UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsReport {
    pub run_at: String,
    pub healthy: bool,
    pub checks: Vec<DiagnosticCheck>,
}

impl From<DoctorReport> for DiagnosticsReport {
    fn from(report: DoctorReport) -> Self {
        Self {
            run_at: report.run_at,
            healthy: report.healthy,
            checks: report
                .checks
                .into_iter()
                .map(|c| DiagnosticCheck {
                    name: c.name,
                    status: match c.status {
                        CheckStatus::Ok => "ok",
                        CheckStatus::Warning => "warning",
                        CheckStatus::Error => "error",
                    }
                    .to_string(),
                    message: c.message,
                })
                .collect(),
        }
    }
}

/// Export payload — a ready-to-share Markdown report, no personal data.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsExport {
    pub content: String,
    pub filename: String,
}

fn render_markdown(report: &DiagnosticsReport) -> String {
    let mut out = String::new();
    out.push_str("# Nexus Diagnostic Report\n\n");
    out.push_str(&format!("- Generated: {}\n", report.run_at));
    out.push_str(&format!(
        "- Overall: {}\n\n",
        if report.healthy {
            "HEALTHY"
        } else {
            "NEEDS ATTENTION"
        }
    ));
    out.push_str("| Check | Status | Detail |\n|---|---|---|\n");
    for check in &report.checks {
        // Escape pipe characters so the table stays well-formed.
        let detail = check.message.replace('|', "\\|");
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            check.name, check.status, detail
        ));
    }
    out.push_str("\n*No personal data is included in this report.*\n");
    out
}

/// Run the full doctor battery and return the snapshot for the UI.
#[tauri::command]
pub fn get_diagnostics_report() -> Result<DiagnosticsReport, String> {
    let report = run_doctor();
    Ok(report.into())
}

/// Render the diagnostics report as portable Markdown (no PII) for export.
#[tauri::command]
pub fn export_diagnostics_report() -> Result<DiagnosticsExport, String> {
    let report = run_doctor();
    let content = render_markdown(&DiagnosticsReport::from(report));
    let stamp = Utc::now().format("%Y%m%d-%H%M%S");
    Ok(DiagnosticsExport {
        content,
        filename: format!("nexus-diagnostics-{stamp}.md"),
    })
}
