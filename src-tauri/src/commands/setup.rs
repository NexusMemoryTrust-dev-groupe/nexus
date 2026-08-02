//! First-run setup: detect prerequisites, install what is missing, and register
//! Nexus with OpenCode so any AI can reach the memory graph.
//!
//! Why this exists
//! ---------------
//! Everything here used to live in a PowerShell script the user had to find and
//! run by hand, and the MCP registration step did not exist at all — the whole
//! premise of the product ("your AI works through our MCP server") required the
//! user to hand-write a JSON config. These commands let the UI drive the same
//! work with no terminal.
//!
//! Every check below reports what was actually observed. Nothing is assumed:
//! if we cannot prove a component works, it is reported as missing rather than
//! optimistically marked present.

use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::Duration;

/// How long to wait on a probe subprocess before giving up.
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

/// How long to allow `npm install -g` to run.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(300);

// ── Status model ────────────────────────────────────────────────────────────

/// State of a single prerequisite.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    /// Stable identifier the UI keys off: `node`, `opencode`, `apiKey`, `mcp`, `model`.
    pub id: String,
    /// True when the component is present and usable.
    pub ok: bool,
    /// Short human-readable state, already localised by the frontend via `id`.
    pub detail: String,
    /// Version or path when we could determine one.
    pub version: Option<String>,
    /// True when Nexus can fix this itself (enables the "Install" button).
    pub fixable: bool,
}

impl CheckResult {
    fn ok(id: &str, detail: impl Into<String>, version: Option<String>) -> Self {
        Self { id: id.into(), ok: true, detail: detail.into(), version, fixable: false }
    }

    fn missing(id: &str, detail: impl Into<String>, fixable: bool) -> Self {
        Self { id: id.into(), ok: false, detail: detail.into(), version: None, fixable }
    }
}

/// Full setup picture handed to the wizard.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatus {
    pub checks: Vec<CheckResult>,
    /// True only when every check passed — the wizard's "you're done" condition.
    pub ready: bool,
    /// Where OpenCode's config lives, so the UI can show it verbatim.
    pub opencode_config_path: String,
    /// Where Nexus keeps its database.
    pub database_path: String,
    /// Absolute path of the running Nexus executable (what gets registered).
    pub executable_path: String,
    /// Token counting method currently in effect: `exact` or `estimated`.
    pub token_method: String,
}

// ── Process helpers ─────────────────────────────────────────────────────────

/// Run a command with a timeout, returning `(success, stdout+stderr)`.
///
/// A plain `output()` can hang forever if the child waits on input; every probe
/// here goes through this wrapper so a wedged subprocess cannot freeze the UI.
fn run_with_timeout(program: &str, args: &[&str], timeout: Duration) -> (bool, String) {
    use std::sync::mpsc;

    let program = program.to_string();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let mut cmd = Command::new(&program);
        cmd.args(&args);
        cmd.stdin(std::process::Stdio::null());

        #[cfg(windows)]
        {
            // CREATE_NO_WINDOW: probing must not flash console windows over the UI.
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }

        let result = cmd.output();
        let _ = tx.send(result);
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(out)) => {
            let mut text = String::from_utf8_lossy(&out.stdout).to_string();
            let err = String::from_utf8_lossy(&out.stderr);
            if !err.trim().is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&err);
            }
            (out.status.success(), text.trim().to_string())
        }
        Ok(Err(e)) => (false, e.to_string()),
        Err(_) => (false, format!("timed out after {}s", timeout.as_secs())),
    }
}

/// Resolve a Windows executable through `where`, returning the first hit.
fn which(program: &str) -> Option<String> {
    let (ok, out) = run_with_timeout("where", &[program], PROBE_TIMEOUT);
    if !ok {
        return None;
    }
    out.lines().map(str::trim).find(|l| !l.is_empty()).map(str::to_string)
}

/// Locate `npm`. On Windows the executable is `npm.cmd`, which must be invoked
/// through `cmd /c` — spawning it directly fails with "program not found".
fn npm_available() -> bool {
    which("npm.cmd").is_some() || which("npm").is_some()
}

// ── Individual checks ───────────────────────────────────────────────────────

fn check_node() -> CheckResult {
    let (ok, out) = run_with_timeout("node", &["--version"], PROBE_TIMEOUT);
    if ok && out.starts_with('v') {
        // OpenCode needs a modern Node; anything older cannot install it.
        let major: u32 = out
            .trim_start_matches('v')
            .split('.')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if major >= 18 {
            return CheckResult::ok("node", "installed", Some(out));
        }
        return CheckResult {
            id: "node".into(),
            ok: false,
            detail: "tooOld".into(),
            version: Some(out),
            // We deliberately do not auto-upgrade Node: replacing a system
            // runtime behind the user's back can break unrelated tooling.
            fixable: false,
        };
    }
    CheckResult::missing("node", "notFound", false)
}

fn check_opencode() -> CheckResult {
    let Some(path) = crate::commands::ai::opencode_path() else {
        // Fixable: `npm install -g opencode-ai` needs Node, which is checked separately.
        return CheckResult::missing("opencode", "notFound", npm_available());
    };

    let (ok, out) = run_with_timeout(&path, &["--version"], PROBE_TIMEOUT);
    if ok && !out.is_empty() {
        let version = out.lines().next().unwrap_or("").trim().to_string();
        return CheckResult::ok("opencode", "installed", Some(version));
    }
    // Present on disk but not runnable — a broken install, so offer a reinstall.
    CheckResult::missing("opencode", "notRunnable", npm_available())
}

/// Does OpenCode have any usable credentials?
///
/// We ask OpenCode itself rather than guessing at files: `models` only lists
/// providers it can actually reach, so a non-empty list proves working access.
fn check_api_key() -> CheckResult {
    let Some(path) = crate::commands::ai::opencode_path() else {
        return CheckResult::missing("apiKey", "opencodeMissing", false);
    };

    let (ok, out) = run_with_timeout(&path, &["models"], PROBE_TIMEOUT);
    let models: Vec<&str> = out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && l.contains('/'))
        .collect();

    if ok && !models.is_empty() {
        return CheckResult::ok("apiKey", "configured", Some(format!("{}", models.len())));
    }
    CheckResult::missing("apiKey", "notConfigured", false)
}

fn check_mcp() -> CheckResult {
    match crate::core::mcp_register::status() {
        crate::core::mcp_register::Registration::Current { .. } => {
            CheckResult::ok("mcp", "registered", None)
        }
        crate::core::mcp_register::Registration::Stale { previous, .. } => CheckResult {
            id: "mcp".into(),
            ok: false,
            detail: "stalePath".into(),
            version: Some(previous),
            fixable: true,
        },
        crate::core::mcp_register::Registration::Absent { .. } => {
            CheckResult::missing("mcp", "notRegistered", true)
        }
    }
}

fn check_model() -> CheckResult {
    match crate::commands::config::get_config_sync("ai.model".to_string()) {
        Ok(Some(v)) if !v.trim().is_empty() => CheckResult::ok("model", "selected", Some(v)),
        _ => CheckResult::missing("model", "notSelected", true),
    }
}

// ── Commands ────────────────────────────────────────────────────────────────

/// Inspect every prerequisite. Safe to call repeatedly; performs no changes.
#[tauri::command]
pub async fn setup_status() -> Result<SetupStatus, String> {
    tokio::task::spawn_blocking(|| {
        let checks = vec![
            check_node(),
            check_opencode(),
            check_api_key(),
            check_mcp(),
            check_model(),
        ];
        let ready = checks.iter().all(|c| c.ok);

        SetupStatus {
            checks,
            ready,
            opencode_config_path: crate::core::mcp_register::config_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|e| format!("unavailable: {}", e)),
            database_path: crate::db::db_path().display().to_string(),
            executable_path: std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            token_method: crate::core::tokenizer::method().as_str().to_string(),
        }
    })
    .await
    .map_err(|e| format!("Setup probe failed: {}", e))
}

/// Install the OpenCode CLI via npm.
///
/// Returns the installed version on success. Reports npm's own output verbatim
/// on failure so the user sees the real reason rather than a generic message.
#[tauri::command]
pub async fn install_opencode() -> Result<String, String> {
    tokio::task::spawn_blocking(|| {
        if !npm_available() {
            return Err(
                "npm was not found. Install Node.js 18 or newer, then try again.".to_string(),
            );
        }

        // npm on Windows is a .cmd shim, so it has to go through the shell.
        let (ok, out) = run_with_timeout(
            "cmd",
            &["/c", "npm", "install", "-g", "opencode-ai"],
            INSTALL_TIMEOUT,
        );
        if !ok {
            return Err(format!("npm install failed:\n{}", out));
        }

        // Verify rather than trust the exit code: a zero exit with a broken
        // install would otherwise be reported as success.
        match crate::commands::ai::opencode_path() {
            Some(path) => {
                let (v_ok, version) = run_with_timeout(&path, &["--version"], PROBE_TIMEOUT);
                if v_ok && !version.is_empty() {
                    Ok(version.lines().next().unwrap_or("").trim().to_string())
                } else {
                    Err("OpenCode installed but does not run. Try reopening Nexus.".to_string())
                }
            }
            None => Err(
                "OpenCode installed but was not found on PATH. Restart Nexus and retry."
                    .to_string(),
            ),
        }
    })
    .await
    .map_err(|e| format!("Install task failed: {}", e))?
}

/// Register (or refresh) Nexus as an MCP server in OpenCode's config.
///
/// Idempotent: running it twice reports `alreadyCurrent` and leaves the file
/// untouched. Any pre-existing config is backed up before being rewritten.
#[tauri::command]
pub async fn register_mcp() -> Result<serde_json::Value, String> {
    tokio::task::spawn_blocking(|| {
        crate::core::mcp_register::register()
            .map(|outcome| {
                serde_json::json!({
                    "state": outcome.state_id(),
                    "configPath": outcome.config_path(),
                    "changed": outcome.changed(),
                })
            })
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Registration task failed: {}", e))?
}

/// Persist the OpenCode API key.
///
/// Stored through OpenCode's own credential command so it lands wherever that
/// version expects it, and mirrored into our config table only as a presence
/// flag — never the secret itself.
#[tauri::command]
pub async fn save_api_key(key: String) -> Result<String, String> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("The API key is empty.".to_string());
    }

    tokio::task::spawn_blocking(move || {
        let Some(path) = crate::commands::ai::opencode_path() else {
            return Err("OpenCode is not installed yet. Install it first.".to_string());
        };

        // `providers` (alias `auth`) owns credential storage in OpenCode 1.x.
        let (ok, out) = run_with_timeout(
            &path,
            &["providers", "login", "--api-key", &key],
            PROBE_TIMEOUT,
        );
        if !ok {
            return Err(format!(
                "OpenCode rejected the key:\n{}\n\nYou can also run: opencode providers login",
                out
            ));
        }
        Ok("saved".to_string())
    })
    .await
    .map_err(|e| format!("Save task failed: {}", e))?
}

/// Persist the chosen model.
#[tauri::command]
pub async fn select_model(model: String) -> Result<(), String> {
    let model = model.trim().to_string();
    if model.is_empty() {
        return Err("No model selected.".to_string());
    }
    crate::commands::config::set_config("ai.model".to_string(), model).await
}

/// Mark the wizard as finished so it does not reappear on every launch.
#[tauri::command]
pub async fn complete_setup() -> Result<(), String> {
    crate::commands::config::set_config(
        "setup.completed_version".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
    )
    .await
}

/// Should the wizard be shown? True until setup completes for this version.
#[tauri::command]
pub async fn setup_needed() -> Result<bool, String> {
    let done = crate::commands::config::get_config_sync("setup.completed_version".to_string())?;
    Ok(match done {
        Some(v) => v.trim() != env!("CARGO_PKG_VERSION"),
        None => true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_result_is_not_fixable() {
        let c = CheckResult::ok("node", "installed", Some("v20.0.0".into()));
        assert!(c.ok);
        assert!(!c.fixable);
        assert_eq!(c.version.as_deref(), Some("v20.0.0"));
    }

    #[test]
    fn missing_result_carries_fixability() {
        let c = CheckResult::missing("opencode", "notFound", true);
        assert!(!c.ok);
        assert!(c.fixable);
        assert!(c.version.is_none());
    }

    #[test]
    fn timeout_is_reported_not_hung() {
        // A slow command must come back as a failure rather than blocking the
        // wizard. `ping` is used deliberately: `cmd /c pause` returns instantly
        // once stdin is null (which it is, so console windows never appear), so
        // it would not exercise the timeout at all.
        let started = std::time::Instant::now();
        let (ok, out) = run_with_timeout(
            "ping",
            &["-n", "10", "127.0.0.1"],
            Duration::from_millis(300),
        );
        assert!(!ok, "a command exceeding its budget must fail: {out}");
        assert!(out.contains("timed out"), "unexpected output: {out}");
        // Proves we returned on the timeout instead of waiting for the process.
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "run_with_timeout blocked for {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn which_finds_a_known_system_binary() {
        // `cmd` exists on every Windows install; proves the resolver works.
        assert!(which("cmd").is_some());
    }

    #[test]
    fn which_returns_none_for_nonsense() {
        assert!(which("definitely-not-a-real-program-xyz").is_none());
    }

    #[test]
    fn status_serialises_with_camel_case_keys() {
        // The frontend reads camelCase; a rename would silently break the wizard.
        let s = SetupStatus {
            checks: vec![CheckResult::ok("node", "installed", None)],
            ready: true,
            opencode_config_path: "c".into(),
            database_path: "d".into(),
            executable_path: "e".into(),
            token_method: "exact".into(),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"opencodeConfigPath\""));
        assert!(json.contains("\"tokenMethod\""));
        assert!(json.contains("\"ready\":true"));
    }

    #[test]
    fn node_check_reports_a_verdict_either_way() {
        // Must never panic regardless of what is installed on the machine.
        let c = check_node();
        assert_eq!(c.id, "node");
    }

    #[test]
    fn mcp_check_reports_a_verdict_either_way() {
        let c = check_mcp();
        assert_eq!(c.id, "mcp");
    }
}
