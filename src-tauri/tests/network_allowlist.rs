//! Privacy mandate 9.3 — "0 unexpected outbound network connections".
//!
//! This is a *lock*: it walks the tree and refuses anything that would let the
//! app dial a host that is not in the documented allowlist. Adding a new
//! network primitive, a new updater endpoint, a remote `<link>` in `index.html`
//! or a programmatic `fetch`/`WebSocket` in the frontend fails these tests
//! until the endpoint is consciously vetted and documented in
//! `docs/NETWORK_PRIVACY.md`.
//!
//! Allowlist (the only hosts Nexus is ever supposed to contact):
//!   - `github.com`      — auto-updater (plan 7.2): release manifests and
//!     installers, `src-tauri/src/infra/updater.rs`.
//!   - `huggingface.co`  — fastembed ONNX model download on first semantic
//!     search, cached under `.fastembed_cache`.
//!
//! Not enforced here (by design): `opencode.ai` / `example.com` appear in
//! `src/core/mcp_register.rs` as embedded JSON-schema `$schema` strings — pure
//! metadata placed inside generated documents, never fetched by any client.

use std::path::{Path, PathBuf};

/// Update endpoints must live on this host.
const ALLOWED_UPDATE_HOSTS: &[&str] = &["github.com"];

/// Network primitives that must never appear in Rust crate sources. Any
/// occurrence means a new code path that can open a socket or fire an HTTP
/// client, bypassing the allowlist review.
const FORBIDDEN_RUST_PRIMITIVES: &[&str] = &[
    "TcpStream",
    "UdpSocket",
    "TcpListener",
    "reqwest::",
    "ureq::",
    "hyper::",
    "ws://",
    "wss://",
    "WebSocket::connect",
];

/// HTTP client crates that must not re-enter the direct dependency tree
/// (removed for 9.3 — reqwest was declared but never used).
const FORBIDDEN_DIRECT_DEPS: &[&str] = &["reqwest", "ureq", "hyper"];

/// Programmatic network entry points that must not appear in the frontend.
/// A static `<a href>` link is user-initiated and acceptable; anything here
/// fires a request on its own.
const FORBIDDEN_FRONTEND_PATTERNS: &[&str] =
    &["fetch(", "axios", "new WebSocket(", "XMLHttpRequest"];

fn manifest_dir() -> &'static str {
    env!("CARGO_MANIFEST_DIR")
}

/// Recursively collect all files under `dir` with the given extensions.
fn collect_files(dir: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, exts, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str())
            && exts.contains(&ext)
        {
            out.push(path);
        }
    }
}

fn file_contents(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn extract_https_hosts(text: &str) -> Vec<String> {
    // `https://host` — capture host (letters, digits, dots, hyphens).
    let mut hosts = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"https://") {
            let start = i + "https://".len();
            let mut end = start;
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'.' || bytes[end] == b'-')
            {
                end += 1;
            }
            let host = &text[start..end];
            if !host.is_empty() {
                hosts.push(host.to_ascii_lowercase());
            }
            i = end;
        } else {
            i += 1;
        }
    }
    hosts
}

#[test]
fn updater_endpoints_only_github() {
    // The runtime update probe is the only place the app dials a host by
    // itself. Every URL it can construct must point at the Nexus GitHub repo.
    //
    // `src/infra/updater.rs` is scanned for every URL literal: all of them are
    // endpoint constructions (`channel_endpoints`, `RELEASES_BASE`). For
    // `tauri.conf.json` only the `plugins.updater.endpoints` array is vetted —
    // the `$schema` at the top of the file is IDE metadata about the file
    // itself (points at raw.githubusercontent.com) and is never contacted.
    let updater = file_contents(&Path::new(manifest_dir()).join("src/infra/updater.rs"));
    let conf = file_contents(&Path::new(manifest_dir()).join("tauri.conf.json"));

    let mut offending = Vec::new();
    for host in extract_https_hosts(&updater) {
        if !ALLOWED_UPDATE_HOSTS.contains(&host.as_str()) {
            offending.push(host);
        }
    }

    // Extract only the `plugins.updater.endpoints` array from tauri.conf.json.
    let value: serde_json::Value = serde_json::from_str(&conf)
        .unwrap_or_else(|e| panic!("tauri.conf.json is not valid JSON: {e}"));
    let endpoints = value
        .get("plugins")
        .and_then(|p| p.get("updater"))
        .and_then(|u| u.get("endpoints"))
        .and_then(|e| e.as_array());
    assert!(
        endpoints.is_some(),
        "tauri.conf.json must define plugins.updater.endpoints"
    );
    for endpoint in endpoints.unwrap() {
        let url = endpoint
            .as_str()
            .unwrap_or_else(|| panic!("endpoint must be a string: {endpoint}"));
        for host in extract_https_hosts(url) {
            if !ALLOWED_UPDATE_HOSTS.contains(&host.as_str()) {
                offending.push(format!("{host} (from {url})"));
            }
        }
    }

    assert!(
        offending.is_empty(),
        "update endpoints must only target github.com (plan 7.2), got: {offending:?}"
    );
}

#[test]
fn no_raw_network_primitives_in_rust_sources() {
    // Walk every .rs file of the crate and refuse any raw socket / HTTP client
    // primitive. Network I/O is only allowed through tauri-plugin-updater
    // (configured endpoints) and fastembed (model download), never hand-rolled.
    let mut files = Vec::new();
    collect_files(&Path::new(manifest_dir()).join("src"), &["rs"], &mut files);
    assert!(!files.is_empty(), "no Rust sources found under src/");

    let mut hits: Vec<(String, String)> = Vec::new();
    for file in &files {
        let content = file_contents(file);
        for prim in FORBIDDEN_RUST_PRIMITIVES {
            if content.contains(prim) {
                hits.push((
                    file.strip_prefix(manifest_dir())
                        .unwrap_or(file)
                        .display()
                        .to_string(),
                    prim.to_string(),
                ));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "forbidden network primitives found (9.3): {hits:?}"
    );
}

#[test]
fn no_remote_resources_in_index_html() {
    // The webview must not load fonts/styles/scripts from a CDN at startup —
    // that is an unexpected connection on every launch.
    let index = file_contents(
        &Path::new(manifest_dir())
            .parent()
            .unwrap()
            .join("index.html"),
    );
    assert!(
        !index.contains("https://"),
        "index.html must not reference remote resources, found:\n{}",
        index
    );
}

#[test]
fn no_programmatic_network_in_frontend() {
    // Frontend sources must not fire requests by themselves. User-initiated
    // static `<a href>` links are fine and not matched here.
    let root = Path::new(manifest_dir()).parent().unwrap();
    let mut files = Vec::new();
    collect_files(
        &root.join("src"),
        &["ts", "tsx", "js", "jsx", "html"],
        &mut files,
    );

    let mut hits: Vec<(String, String)> = Vec::new();
    for file in &files {
        let content = file_contents(file);
        for pat in FORBIDDEN_FRONTEND_PATTERNS {
            if content.contains(pat) {
                hits.push((
                    file.strip_prefix(root)
                        .unwrap_or(file)
                        .display()
                        .to_string(),
                    pat.to_string(),
                ));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "forbidden frontend network entry points found (9.3): {hits:?}"
    );
}

#[test]
fn no_direct_http_client_dependencies() {
    // Guard against re-adding a direct HTTP-client dependency: the app must
    // not carry an unused network stack (reqwest was removed in 9.3).
    let cargo = file_contents(&Path::new(manifest_dir()).join("Cargo.toml"));
    let deps_section = cargo
        .split("[dependencies]")
        .nth(1)
        .unwrap_or("")
        .split("[dev-dependencies]")
        .next()
        .unwrap_or("");
    let offending: Vec<&str> = FORBIDDEN_DIRECT_DEPS
        .iter()
        .filter(|d| deps_section.lines().any(|l| l.trim_start().starts_with(*d)))
        .copied()
        .collect();
    assert!(
        offending.is_empty(),
        "direct HTTP-client dependencies are forbidden (9.3): {offending:?}"
    );
}
