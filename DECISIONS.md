# MVP Integration — DECISIONS.md

## Architecture Decisions

### 1. One SQLite connection per Tauri command

**Decision**: Each Tauri command opens its own `rusqlite::Connection` via `open_repo()`.

**Why**: `rusqlite::Connection` is `!Clone` and not `Send + Sync`. A shared connection would require a mutex, creating a bottleneck for concurrent Tauri IPC calls.

**Trade-off**: More file descriptors (4 connections). Acceptable for single-user desktop app.

---

### 2. Event bus on Tauri async runtime (not separate runtime)

**Decision**: `InMemoryEventBus` is initialized as `Arc<InMemoryEventBus>`, managed via `.manage()`, and subscribed to in `.setup()`.

**Why**: Earlier attempt used a separate `tokio::runtime::Runtime` which caused lifetime issues and would not share the broadcast channel with Tauri's runtime.

**Implementation**: `tauri::async_runtime::spawn` inside `.setup()` callback ensures the subscription runs on Tauri's own tokio runtime.

---

### 3. Context building through ContextService (расширенный пайплайн)

**Decision**: `commands/context.rs` вызывает `ContextService::build_context()` → `ContextBuilderImpl` (полный пайплайн: intent → seed → expand → inject → compress → rank). Пакет дополнен провенансом (`provenance.rs`) и baseline-токенами (`tokenizer.rs`).

**Why**: MVP-фаза начиналась с упрощённой версии в command-слое (поиск памяти + поиск графа). Расширение: когда все зависимости (IntentDetector, Ranker, Compressor, ContextCache, MemoryInjector) стали стабильны, команда переключена на полный `ContextService` — тот же command-слой, расширенная внутренняя реализация.

**Future**: Новые шаги пайплайна (семантический intent, гибридный recall) — расширения существующих шагов, а не замена command-слоя.

---

### 4. Event bus M2→M28 via versioning_listener (not inline in command)

**Decision**: A dedicated `versioning_listener.rs` subscribes to `MemoryRecordCreated` events and creates automatic commits asynchronously.

**Why**: 
- Separation of concerns: memory command doesn't need to know about versioning.
- Async: commit creation is spawned via `tokio::spawn`, so it doesn't block the memory creation response.
- Testable: listener has its own unit tests with a mock `CommitService`.

**Trade-off**: Auto-commit runs fire-and-forget. If it fails, the error is logged but the memory creation still succeeds. This is intentional — versioning should not block core operations.

---

### 5. Frontend CSS class-based styling (not Tailwind utilities)

**Decision**: All M9 components use CSS class names (`sidebar-item`, `card`, `badge`, `ai-panel`, etc.) defined in `globals.css`, not inline Tailwind utility classes.

**Why**: The raytsystem design system is defined as CSS custom properties and semantic class names. This ensures visual consistency and makes the design system a single source of truth.

**Trade-off**: More verbose JSX (`className="card badge-memory"` instead of `className="p-4 bg-white shadow rounded"`), but the design system is maintainable and themeable.

---

### 6. No connection pooling

**Decision**: No SQLite connection pool. Each command opens and closes its own connection.

**Why**: WAL mode allows concurrent readers. A single-user desktop app doesn't have enough concurrent writes to need a pool. Adds complexity (deadpool/sqlite-pool crate) for no measurable benefit at this scale.

---

### 7. Canonical consolidation (System 3) — merged memories, never deleted

**Decision**: `core/memory/canonical_consolidation.rs` clusters similar memories by Jaccard similarity of token sets (threshold 0.40) and builds a canonical memory per cluster: title from the most important member, summary merges unique tokens, importance/confidence get a per-repeat boost (`0.05`/`0.04`, capped at 1.0). Originals are marked `Merged` + `superseded_by_id` and set to private — **nothing is deleted**.

**Why**: The flagman requirement was «nothing is lost». Merging in place (instead of deleting) keeps provenance and lets the user inspect what fed the canonical.

**Implementation notes**: 
- Idempotency: `exists_cluster` compares the sorted JSON array of member ids, so re-running consolidation never duplicates canons.
- Sources use `MemorySource::Compressed`; the canonical derives from `MemoryRecord::new(title, content, author, source)`.
- Persistence: `V27_canonical_memories.sql` + `storage/sqlite/canonical_repository.rs`.

---

### 8. Agent-level firewall permissions (System 4) — deny by default

**Decision**: `core/memory/agent_permissions.rs` classifies memories into categories (secrets/personal/architecture/code/decisions/documentation) and sensitivity levels (public/project/restricted/private). `assess_agent_access(agent, memory)` decides Allow/Deny per agent policy.

**Why**: One global firewall rule cannot express «the researcher may see project architecture but never secrets». Agent-level policies map naturally onto MCP: the caller identifies itself, Nexus decides what it may see.

**Implementation notes**:
- Safety default: a disabled/missing policy is **Deny**; empty `allowed_visibility`/`allowed_layers` lists mean «everything allowed».
- CLI format: `/agent-policy add <agent> [vis] [layers] : <deny-patterns>` parsed via `split_once(':')`.
- Persistence: `V28_agent_policies.sql` + CRUD in `SqliteFirewallRepository` (`save_policy`, `list_policies`, `get_policy_for_agent`, `delete_policy`).

---

### 9. Context chain recording (System 5) — why did AI say this?

**Decision**: `core/flight/context_chain.rs` records the full pipeline of every answer: 10 `ChainStage`s (request → seeds → expand → inject → compress → answer), `ContextSeed`s with kind/weight/tokens, final answer confidence and total tokens. `render_why` produces an ASCII breakdown («Why did AI say this?») with per-category share bars; `render_stages` renders the pipeline chronology.

**Why**: The flight recorder stores *what happened* (records/sessions). The context chain answers *why the answer came out this way* — the debug path from an answer back to the memory seeds that shaped it.

**Implementation notes**:
- `ContextKind` derives `Ord` for deterministic BTreeMap aggregation in `context_breakdown`; seeds/stages derive `Deserialize` so the repository can round-trip them as JSON columns.
- Persistence: `V29_context_chains.sql` + `save_context_chain` / `get_context_chain` / `recent_context_chains` / `count_context_chains` in `SqliteFlightRepository` (upsert on id).
- Surface: MCP `nexus_why` / `nexus_context_chain_record` / `nexus_context_chain_recent`; copilot `/why <chain-id>` and `/context-chain recent|record|get`.

---

### 10. Knowledge Map (System 9) — AI Universe rings

**Decision**: `core/knowledge/knowledge_map.rs` renders the graph around an entity as four concentric rings — Mission (directly relevant), Relevant (2nd degree), Supporting (3rd+ degree), Historical (superseded/old) — via BFS with layer bucketing (`ring_for_layer`).

**Why**: A flat neighbor list doesn't show *why* an entity matters. Ring structure communicates distance and role at a glance, matching the flagman's «AI Universe» picture.

**Implementation notes**: traversal needs `GraphNeighborhood` + `GraphTraversal` traits in scope (E0599 trap); `KnowledgeMapDto` carries the four ring arrays plus a rendered text form.
