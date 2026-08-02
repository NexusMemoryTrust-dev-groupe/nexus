//! Registers Nexus as an MCP server inside the user's OpenCode config.
//!
//! Why this exists
//! ---------------
//! The whole premise of Nexus is "your AI reads your memory through our MCP
//! server". The binary has always supported `--mcp`, but nothing ever wrote it
//! into OpenCode's config, so every user had to hand-edit JSON before the
//! product did anything. This module closes that gap.
//!
//! Schema (verified against <https://opencode.ai/config.json>, opencode 1.17.9)
//! ```json
//! { "mcp": { "nexus": { "type": "local", "command": ["<exe>", "--mcp"], "enabled": true } } }
//! ```
//! Note the key is `mcp` — *not* `mcpServers`, which is the Claude Desktop
//! spelling and is silently ignored by OpenCode.
//!
//! The config file allows comments and trailing commas (`allowComments` /
//! `allowTrailingCommas` in the schema), so a hand-written config may not be
//! strict JSON. We therefore never fail destructively: if the existing file
//! cannot be parsed we back it up and refuse to overwrite unless asked.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::core::result::{AppError, Result};

/// Key under `mcp` that identifies our server.
pub const SERVER_KEY: &str = "nexus";

/// Outcome of a registration attempt — surfaced to the setup wizard.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum RegistrationOutcome {
    /// Config did not mention Nexus; we added it.
    Added { config_path: String },
    /// Nexus was present but pointed somewhere else (e.g. after reinstalling
    /// to another drive); we corrected the command.
    Updated {
        config_path: String,
        previous_command: Vec<String>,
    },
    /// Already correct — nothing written.
    AlreadyCurrent { config_path: String },
}

impl RegistrationOutcome {
    pub fn config_path(&self) -> &str {
        match self {
            Self::Added { config_path }
            | Self::Updated { config_path, .. }
            | Self::AlreadyCurrent { config_path } => config_path,
        }
    }

    /// True when the config file was modified.
    pub fn changed(&self) -> bool {
        !matches!(self, Self::AlreadyCurrent { .. })
    }

    /// Stable machine-readable identifier for the outcome.
    ///
    /// The wizard uses this as a translation key, so it must not carry
    /// human-readable prose: the UI decides the wording per locale.
    pub fn state_id(&self) -> &'static str {
        match self {
            Self::Added { .. } => "added",
            Self::Updated { .. } => "updated",
            Self::AlreadyCurrent { .. } => "alreadyCurrent",
        }
    }
}

/// Directory holding the global OpenCode config.
///
/// OpenCode follows the XDG layout on every platform, including Windows, where
/// it resolves to `%USERPROFILE%\.config\opencode`. `XDG_CONFIG_HOME` wins when
/// set so power users keep control.
pub fn config_dir() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
        && !xdg.trim().is_empty()
    {
        return Ok(PathBuf::from(xdg).join("opencode"));
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| {
            AppError::Configuration(
                "Cannot locate the home directory (USERPROFILE/HOME unset)".into(),
            )
        })?;
    Ok(PathBuf::from(home).join(".config").join("opencode"))
}

/// Full path to `opencode.json`.
pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("opencode.json"))
}

/// Absolute path of the currently running executable.
pub fn current_exe() -> Result<PathBuf> {
    std::env::current_exe()
        .map_err(|e| AppError::Io(format!("Cannot resolve the Nexus executable path: {}", e)))
}

/// The `command` array OpenCode should invoke to reach our MCP server.
pub fn mcp_command(exe: &Path) -> Vec<String> {
    vec![exe.to_string_lossy().to_string(), "--mcp".to_string()]
}

/// Strip `//` and `/* */` comments so a hand-written config still parses.
///
/// Quote-aware: a `//` inside a JSON string (very common in Windows paths that
/// use forward slashes, or in URLs) must not be treated as a comment.
fn strip_json_comments(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let mut in_string = false;
    let mut escaped = false;

    while i < bytes.len() {
        let b = bytes[i];

        if in_string {
            // Copy whole UTF-8 sequences. `b as char` would reinterpret each
            // byte as Latin-1 and corrupt non-ASCII text such as a Cyrillic
            // user profile path.
            let ch_len = utf8_len(b);
            out.push_str(&input[i..i + ch_len]);
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            i += ch_len;
            continue;
        }

        match b {
            b'"' => {
                in_string = true;
                out.push('"');
                i += 1;
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
            }
            _ => {
                // Multi-byte UTF-8 sequences are copied byte-for-byte; pushing
                // through a byte buffer keeps them intact.
                let ch_len = utf8_len(b);
                out.push_str(&input[i..i + ch_len]);
                i += ch_len;
            }
        }
    }
    out
}

/// Byte length of a UTF-8 sequence from its leading byte.
fn utf8_len(lead: u8) -> usize {
    match lead {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

/// Remove trailing commas before `}` / `]`, which the schema tolerates.
fn strip_trailing_commas(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escaped = false;
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            // Same UTF-8 caveat as in `strip_json_comments`.
            let ch_len = utf8_len(b);
            out.push_str(&input[i..i + ch_len]);
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            i += ch_len;
            continue;
        }
        if b == b'"' {
            in_string = true;
            out.push('"');
            i += 1;
            continue;
        }
        if b == b',' {
            // Look ahead past whitespace for a closing bracket.
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                j += 1;
            }
            if j < bytes.len() && (bytes[j] == b'}' || bytes[j] == b']') {
                i += 1; // drop the comma
                continue;
            }
        }
        let ch_len = utf8_len(b);
        out.push_str(&input[i..i + ch_len]);
        i += ch_len;
    }
    out
}

/// Parse an OpenCode config, tolerating comments and trailing commas.
pub fn parse_config(text: &str) -> std::result::Result<Value, String> {
    if text.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    let cleaned = strip_trailing_commas(&strip_json_comments(text));
    serde_json::from_str(&cleaned).map_err(|e| e.to_string())
}

/// Build the MCP entry for Nexus.
fn nexus_entry(exe: &Path) -> Value {
    json!({
        "type": "local",
        "command": mcp_command(exe),
        "enabled": true,
    })
}

/// Read `command` out of an existing entry, if present.
fn entry_command(entry: &Value) -> Vec<String> {
    entry
        .get("command")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Apply the Nexus entry to a parsed config document.
///
/// Split out from disk I/O so the merge rules are unit-testable: existing
/// unrelated servers, providers and settings must survive untouched.
pub fn apply_registration(
    root: &mut Value,
    exe: &Path,
) -> std::result::Result<Option<Vec<String>>, String> {
    if !root.is_object() {
        if root.is_null() {
            *root = Value::Object(Map::new());
        } else {
            return Err("opencode.json must contain a JSON object at the top level".into());
        }
    }
    let obj = root.as_object_mut().expect("checked above");

    // Keep the schema reference so editors keep autocompleting.
    obj.entry("$schema")
        .or_insert_with(|| Value::String("https://opencode.ai/config.json".into()));

    let mcp = obj
        .entry("mcp")
        .or_insert_with(|| Value::Object(Map::new()));
    if !mcp.is_object() {
        return Err("the \"mcp\" field in opencode.json is not an object".into());
    }
    let mcp_obj = mcp.as_object_mut().expect("checked above");

    let desired = nexus_entry(exe);
    match mcp_obj.get(SERVER_KEY) {
        Some(existing) if *existing == desired => Ok(None),
        Some(existing) => {
            let previous = entry_command(existing);
            mcp_obj.insert(SERVER_KEY.to_string(), desired);
            Ok(Some(previous))
        }
        None => {
            mcp_obj.insert(SERVER_KEY.to_string(), desired);
            Ok(Some(Vec::new()))
        }
    }
}

/// Write `value` to `path` atomically (temp file + rename).
fn write_atomic(path: &Path, value: &Value) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Io("Config path has no parent directory".into()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| AppError::Io(format!("Cannot create {}: {}", parent.display(), e)))?;

    let mut text =
        serde_json::to_string_pretty(value).map_err(|e| AppError::Serialization(e.to_string()))?;
    text.push('\n');

    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text.as_bytes())
        .map_err(|e| AppError::Io(format!("Cannot write {}: {}", tmp.display(), e)))?;
    // Windows rename fails if the destination exists, so replace explicitly.
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|e| AppError::Io(format!("Cannot replace {}: {}", path.display(), e)))?;
    }
    std::fs::rename(&tmp, path)
        .map_err(|e| AppError::Io(format!("Cannot finalise {}: {}", path.display(), e)))?;
    Ok(())
}

/// Copy the current config aside before modifying it.
fn backup(path: &Path) -> Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let dest = path.with_file_name(format!("opencode.json.nexus-backup-{}", stamp));
    std::fs::copy(path, &dest)
        .map_err(|e| AppError::Io(format!("Cannot back up {}: {}", path.display(), e)))?;
    Ok(Some(dest))
}

/// Register the running executable as the `nexus` MCP server.
///
/// Idempotent: running it repeatedly leaves a single, correct entry. Existing
/// MCP servers and unrelated settings are preserved. The previous file is
/// backed up whenever a change is made.
pub fn register() -> Result<RegistrationOutcome> {
    register_with_exe(&current_exe()?)
}

/// Same as [`register`], with an explicit executable path (used by tests and by
/// the installer, which knows the final install location).
pub fn register_with_exe(exe: &Path) -> Result<RegistrationOutcome> {
    let path = config_path()?;
    let display = path.to_string_lossy().to_string();

    let existing_text = match std::fs::read_to_string(&path) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(AppError::Io(format!(
                "Cannot read {}: {}",
                path.display(),
                e
            )));
        }
    };

    let mut root = match existing_text.as_deref() {
        None => Value::Object(Map::new()),
        Some(text) => parse_config(text).map_err(|e| {
            AppError::Configuration(format!(
                "{} is not valid JSON ({}). Fix or remove the file, then retry.",
                path.display(),
                e
            ))
        })?,
    };

    let changed = apply_registration(&mut root, exe).map_err(AppError::Configuration)?;

    match changed {
        None => Ok(RegistrationOutcome::AlreadyCurrent {
            config_path: display,
        }),
        Some(previous) => {
            backup(&path)?;
            write_atomic(&path, &root)?;
            if previous.is_empty() {
                Ok(RegistrationOutcome::Added {
                    config_path: display,
                })
            } else {
                Ok(RegistrationOutcome::Updated {
                    config_path: display,
                    previous_command: previous,
                })
            }
        }
    }
}

/// What the config says about us right now, without changing anything.
///
/// The setup wizard needs to *report* state before offering to fix it, so this
/// is deliberately read-only and never creates or rewrites the config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Registration {
    /// No entry for us at all (or no config file yet).
    Absent { config_path: String },
    /// An entry exists but points somewhere else — typically a previous install
    /// location left behind after the app moved to another drive.
    Stale {
        config_path: String,
        previous: String,
    },
    /// The entry already points at this executable.
    Current { config_path: String },
}

/// Inspect the current registration state for the running executable.
pub fn status() -> Registration {
    match current_exe() {
        Ok(exe) => status_for(&exe),
        // Without a known executable path we cannot claim the entry is ours.
        Err(_) => Registration::Absent {
            config_path: config_path()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
        },
    }
}

/// Inspect the registration state for a specific executable.
pub fn status_for(exe: &Path) -> Registration {
    let display = config_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let Ok(path) = config_path() else {
        return Registration::Absent {
            config_path: display,
        };
    };

    // A missing or unreadable file means "not registered", not an error: the
    // wizard's job is then simply to create it.
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Registration::Absent {
            config_path: display,
        };
    };

    let Ok(root) = parse_config(&text) else {
        return Registration::Absent {
            config_path: display,
        };
    };

    let entry = root.get("mcp").and_then(|m| m.get(SERVER_KEY));

    match entry {
        None => Registration::Absent {
            config_path: display,
        },
        Some(entry) => {
            let found = entry_command(entry);
            if found == mcp_command(exe) {
                Registration::Current {
                    config_path: display,
                }
            } else {
                Registration::Stale {
                    config_path: display,
                    previous: found.join(" "),
                }
            }
        }
    }
}

/// Whether the config currently points at `exe`.
pub fn is_registered_for(exe: &Path) -> bool {
    let Ok(path) = config_path() else {
        return false;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(root) = parse_config(&text) else {
        return false;
    };
    root.get("mcp")
        .and_then(|m| m.get(SERVER_KEY))
        .map(|entry| entry_command(entry) == mcp_command(exe))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe() -> PathBuf {
        PathBuf::from(r"D:\Apps\Nexus\Nexus.exe")
    }

    #[test]
    fn adds_entry_to_empty_config() {
        let mut root = Value::Object(Map::new());
        let changed = apply_registration(&mut root, &exe()).unwrap();
        assert_eq!(changed, Some(Vec::new()), "fresh config counts as added");

        let entry = &root["mcp"][SERVER_KEY];
        assert_eq!(entry["type"], "local");
        assert_eq!(entry["enabled"], true);
        assert_eq!(
            entry["command"],
            json!([r"D:\Apps\Nexus\Nexus.exe", "--mcp"])
        );
    }

    #[test]
    fn uses_mcp_key_not_mcp_servers() {
        // Claude Desktop spells it "mcpServers"; OpenCode ignores that.
        let mut root = Value::Object(Map::new());
        apply_registration(&mut root, &exe()).unwrap();
        assert!(root.get("mcp").is_some());
        assert!(root.get("mcpServers").is_none());
    }

    #[test]
    fn second_run_is_a_no_op() {
        let mut root = Value::Object(Map::new());
        apply_registration(&mut root, &exe()).unwrap();
        let again = apply_registration(&mut root, &exe()).unwrap();
        assert_eq!(again, None, "idempotent");
    }

    #[test]
    fn updates_stale_path_and_reports_previous() {
        let mut root = json!({
            "mcp": {
                SERVER_KEY: {
                    "type": "local",
                    "command": [r"C:\Old\Nexus.exe", "--mcp"],
                    "enabled": true
                }
            }
        });
        let changed = apply_registration(&mut root, &exe()).unwrap();
        assert_eq!(
            changed,
            Some(vec![r"C:\Old\Nexus.exe".into(), "--mcp".into()])
        );
        assert_eq!(
            root["mcp"][SERVER_KEY]["command"][0],
            r"D:\Apps\Nexus\Nexus.exe"
        );
    }

    #[test]
    fn preserves_unrelated_servers_and_settings() {
        let mut root = json!({
            "model": "anthropic/claude-sonnet-5",
            "mcp": {
                "other-tool": { "type": "local", "command": ["other.exe"] }
            },
            "permission": { "bash": "ask" }
        });
        apply_registration(&mut root, &exe()).unwrap();

        assert_eq!(root["model"], "anthropic/claude-sonnet-5");
        assert_eq!(root["permission"]["bash"], "ask");
        assert_eq!(root["mcp"]["other-tool"]["command"][0], "other.exe");
        assert!(root["mcp"][SERVER_KEY].is_object());
    }

    #[test]
    fn injects_schema_reference_once() {
        let mut root = json!({ "$schema": "https://example.com/custom.json" });
        apply_registration(&mut root, &exe()).unwrap();
        assert_eq!(
            root["$schema"], "https://example.com/custom.json",
            "must not clobber a user's schema"
        );
    }

    #[test]
    fn rejects_non_object_mcp_field() {
        let mut root = json!({ "mcp": "nope" });
        assert!(apply_registration(&mut root, &exe()).is_err());
    }

    #[test]
    fn rejects_non_object_root() {
        let mut root = json!([1, 2, 3]);
        assert!(apply_registration(&mut root, &exe()).is_err());
    }

    // ── comment / trailing-comma tolerance ──

    #[test]
    fn parses_config_with_line_comments() {
        let text = r#"{
            // my settings
            "model": "x/y" // trailing note
        }"#;
        assert_eq!(parse_config(text).unwrap()["model"], "x/y");
    }

    #[test]
    fn parses_config_with_block_comments() {
        let text = r#"{ /* header */ "model": "x/y" }"#;
        assert_eq!(parse_config(text).unwrap()["model"], "x/y");
    }

    #[test]
    fn does_not_treat_url_slashes_as_comments() {
        let text = r#"{ "$schema": "https://opencode.ai/config.json" }"#;
        assert_eq!(
            parse_config(text).unwrap()["$schema"],
            "https://opencode.ai/config.json"
        );
    }

    #[test]
    fn keeps_windows_paths_with_escaped_backslashes() {
        let text = r#"{ "mcp": { "a": { "command": ["C:\\Tools\\a.exe"] } } }"#;
        let v = parse_config(text).unwrap();
        assert_eq!(v["mcp"]["a"]["command"][0], r"C:\Tools\a.exe");
    }

    #[test]
    fn tolerates_trailing_commas() {
        let text = r#"{ "a": 1, "b": [1, 2,], }"#;
        let v = parse_config(text).unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(v["b"], json!([1, 2]));
    }

    #[test]
    fn preserves_non_ascii_values() {
        let text = r#"{ "username": "Пользователь" }"#;
        assert_eq!(parse_config(text).unwrap()["username"], "Пользователь");
    }

    #[test]
    fn empty_file_is_an_empty_object() {
        assert_eq!(parse_config("   ").unwrap(), Value::Object(Map::new()));
    }

    #[test]
    fn command_carries_the_mcp_flag() {
        assert_eq!(mcp_command(&exe())[1], "--mcp");
    }

    #[test]
    fn outcome_reports_change_state() {
        let added = RegistrationOutcome::Added {
            config_path: "p".into(),
        };
        let current = RegistrationOutcome::AlreadyCurrent {
            config_path: "p".into(),
        };
        assert!(added.changed());
        assert!(!current.changed());
        assert_eq!(current.config_path(), "p");
    }
}
