//! Physical MCP stdio test: spawns the real `nexus --mcp` binary and drives it
//! over the actual JSON-RPC line protocol, exactly like an AI client would.
//!
//! This is the strongest possible proof the MCP server works: a real process,
//! a real on-disk database (isolated via LOCALAPPDATA), real tool definitions
//! and real tool calls — no unit-test shortcuts.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::Value;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

struct McpClient {
    stdin: ChildStdin,
    rx: mpsc::Receiver<String>,
    next_id: u64,
}

impl McpClient {
    fn spawn(db_dir: &std::path::Path) -> (Child, Self) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_nexus"))
            .arg("--mcp")
            .env("LOCALAPPDATA", db_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn nexus --mcp");

        let stdin = child.stdin.take().expect("child stdin");
        let stdout = child.stdout.take().expect("child stdout");

        // Reader thread: every JSON-RPC response is one line on stdout.
        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break, // EOF or error -> server gone
                    Ok(_) => {
                        if !line.trim().is_empty() {
                            let _ = tx.send(line.trim().to_string());
                        }
                    }
                }
            }
        });

        let client = McpClient {
            stdin,
            rx,
            next_id: 1,
        };
        (child, client)
    }

    /// Send one request, block for the response (with timeout).
    fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        writeln!(self.stdin, "{}", req).expect("write request");
        self.stdin.flush().expect("flush stdin");

        let line = self
            .rx
            .recv_timeout(REQUEST_TIMEOUT)
            .unwrap_or_else(|_| panic!("timed out waiting for response to {method} (id {id})"));
        let resp: Value = serde_json::from_str(&line).expect("parse JSON-RPC response");
        assert_eq!(resp["id"], id, "response id mismatch for {method}: {line}");
        if let Some(err) = resp.get("error") {
            panic!("JSON-RPC error for {method}: {err}");
        }
        resp["result"].clone()
    }

    /// Call a tool; returns (text, isError).
    fn call_tool(&mut self, name: &str, arguments: Value) -> (String, bool) {
        let result = self.call(
            "tools/call",
            serde_json::json!({ "name": name, "arguments": arguments }),
        );
        let content = result["content"].as_array().expect("content array");
        let text = content
            .iter()
            .filter_map(|c| c["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let is_error = result["isError"].as_bool().unwrap_or(false);
        (text, is_error)
    }
}

/// The text payload of a successful tool call is:
/// `message\n\n<pretty-printed JSON data>`.
/// Extract the trailing JSON document, if any.
fn data_from_text(text: &str) -> Option<Value> {
    let idx = text.find("\n\n")?;
    let json = &text[idx + 2..];
    serde_json::from_str(json).ok()
}

fn memory_id_from_text(text: &str) -> String {
    let data = data_from_text(text).expect("memory create data JSON");
    data["id"].as_str().expect("memory id in data").to_string()
}

#[test]
fn mcp_stdio_full_flow() {
    // Isolated on-disk DB for the spawned server process.
    let dir = std::env::temp_dir().join(format!("nexus-mcp-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    println!("[mcp] isolated DB dir: {}", dir.display());

    let (mut child, mut client) = McpClient::spawn(&dir);

    // ── handshake ────────────────────────────────────────────────────
    let init = client.call(
        "initialize",
        serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "e2e", "version": "1.0.0" },
        }),
    );
    assert_eq!(init["serverInfo"]["name"], "nexus-mcp-server");
    println!(
        "[mcp] initialize -> server {} v{}",
        init["serverInfo"]["name"], init["serverInfo"]["version"]
    );

    // ── tools/list: must advertise all 95 tools ─────────────────────
    let list = client.call("tools/list", serde_json::json!({}));
    let tools = list["tools"].as_array().expect("tools array");
    println!("[mcp] tools/list -> {} tools", tools.len());
    assert_eq!(tools.len(), 95, "exactly 95 tools must be advertised");

    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    for expected in [
        "nexus_memory_set_state",
        "nexus_memory_confirm",
        "nexus_memory_feedback",
        "nexus_memory_supersede",
        "nexus_lifecycle_overview",
        "nexus_find_duplicates",
        "nexus_merge_entities",
        "nexus_product_metrics",
        "nexus_savings_record",
        "nexus_docs_import",
        "nexus_docs_list",
        "nexus_docs_search",
        "nexus_agents_read",
        "nexus_agents_generate",
        "nexus_skills_list",
        "nexus_skills_run",
        "nexus_code_import",
        "nexus_code_list",
        "nexus_code_search",
        "nexus_code_deps",
        "nexus_code_dependents",
        "nexus_radar_snapshot",
        "nexus_team_add_member",
        "nexus_team_list_members",
        "nexus_team_update_member",
        "nexus_team_remove_member",
        "nexus_team_overview",
        "nexus_audit_trail",
        "nexus_audit_add_event",
        "nexus_audit_alternative",
    ] {
        assert!(names.contains(&expected), "missing tool {expected}");
    }

    // ── lifecycle tools over the real protocol ──────────────────────
    let (text, err) = client.call_tool(
        "nexus_create_memory",
        serde_json::json!({
            "title": "MCP e2e memory",
            "content": "Created through the real MCP protocol",
        }),
    );
    assert!(!err, "create_memory must succeed: {text}");
    let mem_id = memory_id_from_text(&text);
    println!("[mcp] nexus_create_memory -> id={mem_id}");

    let (text, err) = client.call_tool(
        "nexus_memory_set_state",
        serde_json::json!({ "id": mem_id, "state": "UserConfirmed" }),
    );
    assert!(!err, "set_state: {text}");
    assert!(text.contains("UserConfirmed"), "state applied: {text}");
    println!("[mcp] nexus_memory_set_state -> {text}");

    let (text, err) = client.call_tool(
        "nexus_memory_confirm",
        serde_json::json!({ "id": mem_id, "by": "e2e" }),
    );
    assert!(!err, "confirm: {text}");
    println!("[mcp] nexus_memory_confirm -> {text}");

    let (text, err) = client.call_tool(
        "nexus_memory_feedback",
        serde_json::json!({ "id": mem_id, "kind": "useful" }),
    );
    assert!(!err, "feedback: {text}");
    assert!(text.contains("Useful: 1"), "feedback counted: {text}");
    println!("[mcp] nexus_memory_feedback -> {text}");

    let (text, err) = client.call_tool(
        "nexus_memory_supersede",
        serde_json::json!({
            "old_id": mem_id,
            "new_title": "MCP e2e memory v2",
            "new_content": "Superseded through the real MCP protocol",
        }),
    );
    assert!(!err, "supersede: {text}");
    println!("[mcp] nexus_memory_supersede -> {text}");

    let (text, err) = client.call_tool("nexus_lifecycle_overview", serde_json::json!({}));
    assert!(!err, "lifecycle overview: {text}");
    let overview = data_from_text(&text).expect("overview JSON");
    assert!(
        overview["total"].as_u64().unwrap_or(0) >= 2,
        "2+ memories: {text}"
    );
    assert!(
        overview["superseded"].as_u64().unwrap_or(0) >= 1,
        "superseded counted: {text}"
    );
    println!("[mcp] nexus_lifecycle_overview -> {text}");

    // ── entity resolution tools ─────────────────────────────────────
    for title in ["Nexus", "Nexus MCP", "Nexus Server"] {
        let (text, err) = client.call_tool(
            "nexus_create_entity",
            serde_json::json!({
                "entity_type": "Technology",
                "title": title,
                "description": format!("{title} component"),
            }),
        );
        assert!(!err, "create_entity {title}: {text}");
    }
    println!("[mcp] nexus_create_entity x3 -> done");

    let (text, err) = client.call_tool("nexus_find_duplicates", serde_json::json!({}));
    assert!(!err, "find_duplicates: {text}");
    let groups = data_from_text(&text).expect("duplicate groups JSON");
    let groups_arr = groups.as_array().expect("groups array");
    println!(
        "[mcp] nexus_find_duplicates -> {} group(s): {text}",
        groups_arr.len()
    );
    assert!(!groups_arr.is_empty(), "Nexus variants must form a group");

    let first = &groups_arr[0];
    let best_id = first["bestId"].as_str().expect("bestId").to_string();
    let duplicates: Vec<String> = first["entities"]
        .as_array()
        .expect("entities array")
        .iter()
        .filter_map(|e| {
            let id = e["entityId"].as_str()?;
            (id != best_id).then(|| id.to_string())
        })
        .collect();
    assert!(
        !duplicates.is_empty(),
        "group must have mergeable candidates"
    );

    let (text, err) = client.call_tool(
        "nexus_merge_entities",
        serde_json::json!({ "primary": best_id, "duplicates": duplicates }),
    );
    assert!(!err, "merge_entities: {text}");
    println!("[mcp] nexus_merge_entities -> {text}");

    // ── product metrics tools ───────────────────────────────────────
    let (text, err) = client.call_tool(
        "nexus_savings_record",
        serde_json::json!({
            "baseline_tokens": 48_000,
            "context_tokens": 11_800,
            "entities_count": 3,
            "memories_count": 2,
            "relationships_count": 1,
            "candidate_entities": 5,
            "candidate_memories": 4,
            "query": "MCP e2e query",
            "intent_type": "question",
            "latency_ms": 120,
            "precision": 0.9,
            "used_fragments": 9,
            "irrelevant_fragments": 5,
            "manual_context": 0,
        }),
    );
    assert!(!err, "savings_record: {text}");
    println!("[mcp] nexus_savings_record -> {text}");

    let (text, err) = client.call_tool("nexus_product_metrics", serde_json::json!({}));
    assert!(!err, "product_metrics: {text}");
    let pm = data_from_text(&text).expect("product metrics JSON");
    let interactions = pm["total_interactions"].as_u64().unwrap_or(0);
    let tokens = pm["total_tokens_saved"].as_u64().unwrap_or(0);
    assert_eq!(interactions, 1, "one recorded interaction: {text}");
    assert_eq!(tokens, 48_000 - 11_800, "measured token saving: {text}");
    println!("[mcp] nexus_product_metrics -> {text}");

    // ── RAG pipeline (context builder) ──────────────────────────────
    // The context builder is the RAG system: it assembles a token-counted
    // package (entities, memories, relationships) for a query and records a
    // measured savings event. The e2e DB holds the merged "Nexus Server"
    // entity, so a "Nexus" query must seed it through the graph.
    let (text, err) = client.call_tool(
        "nexus_build_context",
        serde_json::json!({ "query": "Nexus Server" }),
    );
    assert!(!err, "build_context: {text}");
    let pkg = data_from_text(&text).expect("context package JSON");
    assert!(
        pkg["token_count"].as_u64().unwrap_or(0) > 0,
        "context package carries tokens: {text}"
    );
    let entities = pkg["entities"].as_u64().unwrap_or(0);
    let memories = pkg["memory_records"].as_u64().unwrap_or(0);
    assert!(
        entities > 0 || memories > 0,
        "context package found entities ({entities}) or memories ({memories}): {text}"
    );
    assert!(
        pkg["token_count"].as_u64().unwrap_or(0) >= entities + memories,
        "token count covers content: {text}"
    );
    println!("[mcp] nexus_build_context -> {text}");

    let (text, err) = client.call_tool("nexus_product_metrics", serde_json::json!({}));
    assert!(!err, "product_metrics after RAG: {text}");
    let pm2 = data_from_text(&text).expect("product metrics JSON after RAG");
    assert_eq!(
        pm2["total_interactions"].as_u64().unwrap_or(0),
        2,
        "RAG interaction recorded: {text}"
    );
    println!("[mcp] nexus_product_metrics (after RAG) -> {text}");

    // ── Copilot command ─────────────────────────────────────────────
    let (text, err) = client.call_tool(
        "nexus_copilot_command",
        serde_json::json!({ "command": "/skills" }),
    );
    assert!(!err, "copilot /skills: {text}");
    assert!(
        text.contains("audit-trail"),
        "copilot lists seeded skills: {text}"
    );
    println!("[mcp] nexus_copilot_command /skills -> {text}");

    let (text, err) = client.call_tool(
        "nexus_copilot_command",
        serde_json::json!({ "command": format!("/audit {mem_id}") }),
    );
    assert!(!err, "copilot /audit: {text}");
    println!("[mcp] nexus_copilot_command /audit -> {text}");

    // ── project knowledge base tools (RAG / AGENTS.md / skills) ────
    let docs_dir = std::env::temp_dir().join(format!("nexus-mcp-docs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&docs_dir);
    std::fs::create_dir_all(&docs_dir).unwrap();
    std::fs::write(
        docs_dir.join("ARCHITECTURE.md"),
        "# Architecture\n\nNexus stores memories in SQLite and embeds them with ONNX models.",
    )
    .unwrap();

    let (text, err) = client.call_tool(
        "nexus_docs_import",
        serde_json::json!({ "folder_path": docs_dir.to_string_lossy() }),
    );
    assert!(!err, "docs_import: {text}");
    assert!(text.contains("scanned 1"), "one doc scanned: {text}");
    println!("[mcp] nexus_docs_import -> {text}");

    let (text, err) = client.call_tool(
        "nexus_docs_search",
        serde_json::json!({ "query": "SQLite", "limit": 5 }),
    );
    assert!(!err, "docs_search: {text}");
    assert!(
        text.contains("1 document(s)"),
        "SQLite found in ARCHITECTURE.md: {text}"
    );
    println!("[mcp] nexus_docs_search -> {text}");

    let (text, err) = client.call_tool("nexus_agents_generate", serde_json::json!({}));
    assert!(!err, "agents_generate: {text}");
    assert!(
        text.contains("AGENTS.md generated"),
        "generated file: {text}"
    );
    println!("[mcp] nexus_agents_generate -> {text}");

    let (text, err) = client.call_tool("nexus_agents_read", serde_json::json!({}));
    assert!(!err, "agents_read: {text}");
    assert!(
        text.contains("AGENTS.md"),
        "read back the generated file: {text}"
    );
    println!("[mcp] nexus_agents_read -> {text}");

    let (text, err) = client.call_tool("nexus_skills_list", serde_json::json!({}));
    assert!(!err, "skills_list: {text}");
    assert!(
        text.contains("audit-trail"),
        "default skills seeded: {text}"
    );
    println!("[mcp] nexus_skills_list -> {text}");

    let (text, err) = client.call_tool(
        "nexus_skills_run",
        serde_json::json!({ "name": "radar-scan", "args": ["2"] }),
    );
    assert!(!err, "skills_run: {text}");
    assert!(text.contains("RADAR"), "skill output: {text}");
    println!("[mcp] nexus_skills_run -> {text}");

    // ── code graph tools ────────────────────────────────────────────
    let code_dir = std::env::temp_dir().join(format!("nexus-mcp-code-{}", std::process::id()));
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

    let (text, err) = client.call_tool(
        "nexus_code_import",
        serde_json::json!({ "folder_path": code_dir.to_string_lossy() }),
    );
    assert!(!err, "code_import: {text}");
    assert!(text.contains("scanned 2"), "two code files scanned: {text}");
    println!("[mcp] nexus_code_import -> {text}");

    let (text, err) = client.call_tool(
        "nexus_code_search",
        serde_json::json!({ "query": "App", "limit": 5 }),
    );
    assert!(!err, "code_search: {text}");
    assert!(text.contains("1 symbol(s)"), "struct App found: {text}");
    println!("[mcp] nexus_code_search -> {text}");

    let main_path = code_dir.join("src").join("main.rs");
    let (text, err) = client.call_tool(
        "nexus_code_deps",
        serde_json::json!({ "path": main_path.to_string_lossy() }),
    );
    assert!(!err, "code_deps: {text}");
    assert!(text.contains("std"), "main.rs imports std: {text}");
    println!("[mcp] nexus_code_deps -> {text}");

    let (text, err) = client.call_tool("nexus_code_list", serde_json::json!({}));
    assert!(!err, "code_list: {text}");
    assert!(text.contains("2 source files"), "two files listed: {text}");
    println!("[mcp] nexus_code_list -> {text}");

    // ── memory radar tools (proactive recall) ───────────────────────
    let (text, err) = client.call_tool("nexus_radar_snapshot", serde_json::json!({}));
    assert!(!err, "radar_snapshot: {text}");
    assert!(
        text.contains("attention score"),
        "radar reports attention: {text}"
    );
    let radar = data_from_text(&text).expect("radar JSON");
    assert!(
        radar["counts"]["total"].as_u64().unwrap_or(0) >= 2,
        "radar sees the e2e memories: {text}"
    );
    assert!(
        radar["attention_score"].as_u64().is_some(),
        "attention score present: {text}"
    );
    println!("[mcp] nexus_radar_snapshot -> {text}");

    let (text, err) = client.call_tool(
        "nexus_radar_snapshot",
        serde_json::json!({ "markSeen": true }),
    );
    assert!(!err, "radar_snapshot markSeen: {text}");
    println!("[mcp] nexus_radar_snapshot(markSeen=true) -> {text}");

    // ── team memory tools (shared trusted layer) ────────────────────
    let (text, err) = client.call_tool(
        "nexus_team_add_member",
        serde_json::json!({ "name": "e2e-mate", "role": "member" }),
    );
    assert!(!err, "team_add_member: {text}");
    assert!(text.contains("e2e-mate"), "member added: {text}");
    println!("[mcp] nexus_team_add_member -> {text}");

    let (text, err) = client.call_tool("nexus_team_list_members", serde_json::json!({}));
    assert!(!err, "team_list_members: {text}");
    assert!(text.contains("e2e-mate"), "roster lists member: {text}");
    println!("[mcp] nexus_team_list_members -> {text}");

    let (text, err) = client.call_tool("nexus_team_overview", serde_json::json!({}));
    assert!(!err, "team_overview: {text}");
    assert!(
        text.contains("confirmed decision") || text.contains("0 confirmed"),
        "overview reports decisions: {text}"
    );
    println!("[mcp] nexus_team_overview -> {text}");

    // ── audit memory tools (decision chain / compliance) ─────────────
    let (text, err) = client.call_tool(
        "nexus_audit_add_event",
        serde_json::json!({
            "memoryId": mem_id,
            "eventType": "Confirmed",
            "actor": "e2e",
            "detail": "verified through the real MCP protocol",
        }),
    );
    assert!(!err, "audit_add_event: {text}");
    assert!(text.contains("recorded"), "event recorded: {text}");
    println!("[mcp] nexus_audit_add_event -> {text}");

    let (text, err) = client.call_tool(
        "nexus_audit_alternative",
        serde_json::json!({
            "memoryId": mem_id,
            "title": "MySQL",
            "reason": "license costs",
            "actor": "e2e",
        }),
    );
    assert!(!err, "audit_alternative: {text}");
    assert!(text.contains("recorded"), "alternative recorded: {text}");
    println!("[mcp] nexus_audit_alternative -> {text}");

    let (text, err) = client.call_tool(
        "nexus_audit_trail",
        serde_json::json!({ "memoryId": mem_id }),
    );
    assert!(!err, "audit_trail: {text}");
    assert!(text.contains("Audit trail for"), "trail header: {text}");
    assert!(
        text.contains("alternative(s)"),
        "trail reports alternatives: {text}"
    );
    println!("[mcp] nexus_audit_trail -> {text}");

    // ── graceful shutdown: EOF on stdin stops the server ────────────
    drop(client.stdin);
    let status = child.wait().expect("server process exits after stdin EOF");
    println!("[mcp] server exited with status {status}");

    println!("\n[mcp] ALL PHYSICAL MCP CHECKS PASSED");
}
