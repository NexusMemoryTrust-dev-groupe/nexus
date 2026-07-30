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

### 3. Context building done in command layer (not full ContextService)

**Decision**: `commands/context.rs` builds context directly by calling memory search + graph search, skipping the full `ContextService` pipeline (intent detection, ranker, compressor).

**Why**: MVP simplicity. The full pipeline requires wiring `IntentDetector`, `Ranker`, `Compressor`, `ContextCache` — which is overkill for a first integration pass. The command does a simplified version: search memories by query, search graph by query, merge results.

**Future**: Replace with `ContextService::build_context()` once all dependencies are stable.

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
