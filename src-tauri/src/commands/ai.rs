use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::process::Command;
use std::time::Duration;
use tauri::Emitter;

const DEFAULT_MODEL: &str = "opencode/deepseek-v4-flash-free";

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub is_free: bool,
}

/// Build a single prompt string from system rules + conversation messages.
fn build_prompt(messages: &[ChatMessage]) -> String {
    crate::commands::ai_prompt::build_full_prompt(messages)
}

/// Relative location of the real executable inside a global npm install.
///
/// npm puts a `opencode.cmd` shim in the prefix directory and the actual binary
/// one level down, inside the package itself.
#[cfg(target_os = "windows")]
const NPM_EXE_SUFFIX: &str = "node_modules\\opencode-ai\\bin\\opencode.exe";

/// Map a discovered path to something that can be spawned directly.
///
/// `.cmd`/`.bat` shims are replaced by the executable they wrap. Everything
/// else passes through untouched.
///
/// Why not just run the shim: since the fix for CVE-2024-24576 Rust refuses to
/// pass arguments to a batch file when they contain characters it cannot safely
/// escape. Every real chat prompt contains such characters (newlines, quotes),
/// so `Command::spawn` fails with "batch file arguments are invalid". That is
/// exactly the error seen in the Co-Pilot, and it explains why listing models
/// still worked: `models` is a single tidy argument with nothing to escape.
///
/// The tempting workaround — `cmd.exe /C <shim> <args>` — is worse than the
/// bug. It hands user-authored prompt text to the command interpreter, where
/// `&` or `|` would be read as operators rather than data. Resolving to the
/// executable keeps Rust's argument handling, and its protection, intact.
fn resolve_to_exe(path: &str) -> Option<String> {
    let lower = path.to_lowercase();
    if !(lower.ends_with(".cmd") || lower.ends_with(".bat")) {
        return Some(path.to_string());
    }

    #[cfg(target_os = "windows")]
    {
        let dir = std::path::Path::new(path).parent()?;
        let exe = dir.join(NPM_EXE_SUFFIX);
        if exe.is_file() {
            return Some(exe.to_string_lossy().to_string());
        }
    }

    None
}

/// Find a directly spawnable opencode binary.
///
/// Candidates are probed in order and each one is passed through
/// [`resolve_to_exe`], so a `.cmd` shim still leads to a usable path.
fn find_opencode_binary() -> Result<String, String> {
    let mut candidates: Vec<String> = Vec::new();

    if cfg!(target_os = "windows") {
        for prefix in [
            std::env::var("APPDATA").map(|v| format!("{}\\npm", v)),
            std::env::var("USERPROFILE").map(|v| format!("{}\\AppData\\Roaming\\npm", v)),
        ]
        .into_iter()
        .flatten()
        {
            candidates.push(format!(
                "{}\\node_modules\\opencode-ai\\bin\\opencode.exe",
                prefix
            ));
            candidates.push(format!("{}\\opencode.cmd", prefix));
        }
    } else {
        candidates.push("/usr/local/bin/opencode".to_string());
        candidates.push("/usr/bin/opencode".to_string());
    }

    for path in &candidates {
        if std::path::Path::new(path).exists()
            && let Some(exe) = resolve_to_exe(path)
        {
            return Ok(exe);
        }
    }

    // Last resort: ask the OS where the command lives. `where` can return
    // several lines; a real executable is preferred over a shim.
    let which_cmd = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    if let Ok(output) = Command::new(which_cmd).arg("opencode").output()
        && output.status.success()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let found: Vec<&str> = stdout
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let ordered = found
            .iter()
            .filter(|l| l.to_lowercase().ends_with(".exe"))
            .chain(found.iter());
        for path in ordered {
            if let Some(exe) = resolve_to_exe(path) {
                return Ok(exe);
            }
        }
    }

    Err("opencode binary not found. Install with: npm install -g opencode-ai".to_string())
}

/// Public probe used by the setup wizard: the OpenCode binary path, or `None`.
///
/// Thin wrapper so the wizard can report presence without duplicating the
/// discovery logic (npm prefix, user profile, then `where`/`which`).
pub fn opencode_path() -> Option<String> {
    find_opencode_binary().ok()
}

/// Read the configured model from DB, falling back to DEFAULT_MODEL.
fn get_configured_model() -> String {
    match crate::commands::config::get_config_sync("ai.model".to_string()) {
        Ok(Some(v)) if !v.is_empty() => v,
        _ => DEFAULT_MODEL.to_string(),
    }
}

/// Parse model ID into provider + name + is_free flag.
fn parse_model(id: &str) -> ModelInfo {
    let parts: Vec<&str> = id.splitn(2, '/').collect();
    let provider = if parts.len() > 1 { parts[0] } else { "unknown" };
    let name = if parts.len() > 1 { parts[1] } else { id };
    let is_free = id.contains("-free") || id.contains("free");
    ModelInfo {
        id: id.to_string(),
        name: name.to_string(),
        provider: provider.to_string(),
        is_free,
    }
}

// ─────────────────────────────────────────────────────
// JSON streaming event types from opencode --format json
// ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OcEvent {
    #[serde(rename = "type")]
    event_type: String,
    part: Option<OcPart>,
}

#[derive(Debug, Deserialize)]
struct OcPart {
    #[serde(rename = "type")]
    #[allow(dead_code)] // Used for JSON deserialization only
    part_type: Option<String>,
    text: Option<String>,
}

// ─────────────────────────────────────────────────────
// Commands
// ─────────────────────────────────────────────────────

/// List all available models via `opencode models` CLI.
#[tauri::command]
pub async fn ai_list_models(free_only: Option<bool>) -> Result<Vec<ModelInfo>, String> {
    let binary = find_opencode_binary()?;
    let want_free = free_only.unwrap_or(false);

    let result = tokio::task::spawn_blocking(move || -> Result<Vec<ModelInfo>, String> {
        let output = Command::new(&binary)
            .arg("models")
            .output()
            .map_err(|e| format!("Failed to run opencode models: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("opencode models error: {}", stderr.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let models: Vec<ModelInfo> = stdout
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(parse_model)
            .filter(|m| !want_free || m.is_free)
            .collect();

        Ok(models)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))??;

    Ok(result)
}

/// Streaming chat — emits thinking + text chunks via Tauri events.
/// Events emitted:
///   "ai-thinking-chunk"  → { chunk: String }
///   "ai-text-chunk"      → { chunk: String }
///   "ai-stream-finish"   → { full_text: String }
///   "ai-stream-error"    → { error: String }
#[tauri::command]
pub async fn ai_chat_stream(
    app: tauri::AppHandle,
    messages: Vec<ChatMessage>,
    model: Option<String>,
) -> Result<String, String> {
    let binary = find_opencode_binary()?;
    let resolved_model = model
        .filter(|m| !m.is_empty())
        .unwrap_or_else(get_configured_model);
    let prompt = build_prompt(&messages);

    let result = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let mut child = Command::new(&binary)
            .args([
                "run",
                "--model",
                &resolved_model,
                "--format",
                "json",
                "--thinking",
                &prompt,
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start opencode: {}", e))?;

        // Capture stderr BEFORE taking stdout to avoid data loss
        let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;
        let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
        let reader = BufReader::new(stdout);

        let mut full_text = String::new();
        let mut thinking_text = String::new();
        let timeout = Duration::from_secs(300); // 5 minutes
        let start = std::time::Instant::now();

        for line in reader.lines() {
            // Check timeout
            if start.elapsed() > timeout {
                let _ = child.kill();
                return Err("AI request timed out (5 min limit).".to_string());
            }

            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Parse JSON event
            let event: OcEvent = match serde_json::from_str(line) {
                Ok(e) => e,
                Err(_) => continue,
            };

            if let Some(part) = event.part {
                match event.event_type.as_str() {
                    "reasoning" => {
                        // Thinking/reasoning chunk from the model
                        if let Some(text) = part.text {
                            thinking_text.push_str(&text);
                            let _ = app.emit(
                                "ai-thinking-chunk",
                                serde_json::json!({
                                    "chunk": text,
                                    "full_thinking": thinking_text,
                                }),
                            );
                        }
                    }
                    "text" => {
                        // Actual response text chunk
                        if let Some(text) = part.text {
                            full_text.push_str(&text);
                            let _ = app.emit(
                                "ai-text-chunk",
                                serde_json::json!({
                                    "chunk": text,
                                    "full_text": full_text,
                                }),
                            );
                        }
                    }
                    _ => {}
                }
            }
        }

        // Wait for process to finish
        let _ = child.wait();

        // Read remaining stderr content
        let mut stderr_buf = String::new();
        use std::io::Read;
        let mut stderr_reader = std::io::BufReader::new(stderr);
        let _ = stderr_reader.read_to_string(&mut stderr_buf);

        if full_text.trim().is_empty() && thinking_text.trim().is_empty() {
            return Err(format!("Empty response from AI. {}", stderr_buf.trim()));
        }

        // If no text but there was thinking, use thinking as response
        let response = if full_text.trim().is_empty() {
            thinking_text.trim().to_string()
        } else {
            full_text.trim().to_string()
        };

        // Emit final event
        let _ = app.emit(
            "ai-stream-finish",
            serde_json::json!({
                "full_text": response,
                "had_thinking": !thinking_text.is_empty(),
            }),
        );

        Ok(response)
    })
    .await
    .map_err(|_| "AI request timed out (5 min limit).".to_string())?
    .map_err(|e| format!("AI error: {}", e))?;

    Ok(result)
}

/// Health check — uses configured model unless overridden.
#[tauri::command]
pub async fn ai_health_check(model: Option<String>) -> Result<String, String> {
    let binary = find_opencode_binary()?;
    let resolved_model = model
        .filter(|m| !m.is_empty())
        .unwrap_or_else(get_configured_model);

    let result = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let output = Command::new(&binary)
            .args(["run", "--model", &resolved_model, "Say 'ok'"])
            .output()
            .map_err(|e| format!("Failed to run opencode: {}", e))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !stdout.is_empty() {
                Ok(format!("OpenCode AI ({}) connected", resolved_model))
            } else {
                Ok("OpenCode AI: empty response".to_string())
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Ok(format!("OpenCode AI: {}", stderr))
        }
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))??;

    Ok(result)
}
