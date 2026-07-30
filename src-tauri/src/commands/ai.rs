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

/// Find the opencode binary path.
fn find_opencode_binary() -> Result<String, String> {
    let candidates = if cfg!(target_os = "windows") {
        vec![
            "C:\\Users\\User\\AppData\\Roaming\\npm\\node_modules\\opencode-ai\\bin\\opencode.exe",
            "C:\\Users\\User\\AppData\\Roaming\\npm\\opencode.cmd",
        ]
    } else {
        vec![
            "/usr/local/bin/opencode",
            "/usr/bin/opencode",
        ]
    };

    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return Ok(path.to_string());
        }
    }

    let which_cmd = if cfg!(target_os = "windows") { "where" } else { "which" };
    if let Ok(output) = Command::new(which_cmd).arg("opencode").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let first_line = stdout.lines().next().unwrap_or("").trim();
            if !first_line.is_empty() {
                return Ok(first_line.to_string());
            }
        }
    }

    Err("opencode binary not found. Install with: npm install -g opencode-ai".to_string())
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
            .map(|l| parse_model(l))
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
    let resolved_model = model.filter(|m| !m.is_empty()).unwrap_or_else(get_configured_model);
    let prompt = build_prompt(&messages);

    let result = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let mut child = Command::new(&binary)
            .args([
                "run",
                "--model", &resolved_model,
                "--format", "json",
                "--thinking",
                &prompt,
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start opencode: {}", e))?;

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
                            let _ = app.emit("ai-thinking-chunk", serde_json::json!({
                                "chunk": text,
                                "full_thinking": thinking_text,
                            }));
                        }
                    }
                    "text" => {
                        // Actual response text chunk
                        if let Some(text) = part.text {
                            full_text.push_str(&text);
                            let _ = app.emit("ai-text-chunk", serde_json::json!({
                                "chunk": text,
                                "full_text": full_text,
                            }));
                        }
                    }
                    _ => {}
                }
            }
        }

        // Wait for process to finish
        let _ = child.wait();

        if full_text.trim().is_empty() && thinking_text.trim().is_empty() {
            let stderr_output = child.wait_with_output()
                .map(|o| String::from_utf8_lossy(&o.stderr).trim().to_string())
                .unwrap_or_default();
            return Err(format!("Empty response from AI. {}", stderr_output));
        }

        // If no text but there was thinking, use thinking as response
        let response = if full_text.trim().is_empty() {
            thinking_text.trim().to_string()
        } else {
            full_text.trim().to_string()
        };

        // Emit final event
        let _ = app.emit("ai-stream-finish", serde_json::json!({
            "full_text": response,
            "had_thinking": !thinking_text.is_empty(),
        }));

        Ok(response)
    })
    .await
    .map_err(|_| "AI request timed out (5 min limit).".to_string())?
    .map_err(|e| format!("AI error: {}", e))?;

    Ok(result)
}

/// Non-streaming chat — uses configured model unless overridden. 5 min timeout.
#[tauri::command]
pub async fn ai_chat(messages: Vec<ChatMessage>, model: Option<String>) -> Result<String, String> {
    let binary = find_opencode_binary()?;
    let resolved_model = model.filter(|m| !m.is_empty()).unwrap_or_else(get_configured_model);
    let prompt = build_prompt(&messages);

    let result = tokio::time::timeout(Duration::from_secs(300), tokio::task::spawn_blocking(move || -> Result<String, String> {
        let output = Command::new(&binary)
            .args(["run", "--model", &resolved_model, &prompt])
            .output()
            .map_err(|e| format!("Failed to run opencode: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let msg = if !stderr.is_empty() {
                stderr.trim().to_string()
            } else {
                stdout.trim().to_string()
            };
            return Err(format!("opencode error: {}", msg));
        }

        let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if result.is_empty() {
            return Err("Empty response from AI".to_string());
        }

        Ok(result)
    }))
    .await
    .map_err(|_| "AI request timed out (5 min limit). Try a shorter question or switch model.".to_string())?
    .map_err(|e| format!("AI error: {}", e))?;

    Ok(result?)
}

/// Health check — uses configured model unless overridden.
#[tauri::command]
pub async fn ai_health_check(model: Option<String>) -> Result<String, String> {
    let binary = find_opencode_binary()?;
    let resolved_model = model.filter(|m| !m.is_empty()).unwrap_or_else(get_configured_model);

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
