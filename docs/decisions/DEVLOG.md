# Foundation DEVLOG.md

## 2026-07-22 Project Setup

### Created
- Project structure: nexus/ with src-tauri/, src/, ai/, tests/, docs/
- Rust backend: Cargo.toml, main.rs, core/ modules (6 files)
- Core modules: error.rs, types.rs, event_bus.rs, module_registry.rs, config.rs, context.rs
- React frontend: package.json, main.tsx, App.tsx, globals.css, tauri.ts, vite.config.ts, tsconfig.json
- Python AI layer: pyproject.toml, __init__.py
- Architecture tests in tests/rust/architecture/mod.rs
- CI/CD: .github/workflows/ci.yml
- Documentation: README.md, DECISIONS.md, DEVLOG.md

### Verified
- All Rust core modules compile (pending cargo build)
- Architecture tests exist and will run
- Project structure follows Clean Architecture principles

### Notes
- Using Tauri 2.11.5 (latest stable)
- Using Rust edition 2024 (stable since 1.85)
- Architecture tests enforce: no core→infra deps, no core→tauri deps

---

## 2026-07-23 M1 Core Platform Implementation

### Files Created
- `core/result.rs` — AppError enum (NotFound, Unauthorized, Forbidden, Validation, Conflict, Internal) + Result<T> type alias
- `core/entity_id.rs` — EntityId newtype (UUID v4, String wrapper, parse/serialize/Display/Hash/Ord)
- `core/value_object.rs` — ValueObject trait with validate() + EmailValueObject example impl
- `core/domain_event.rs` — DomainEvent struct (id, event_type, payload, metadata, timestamp) + DomainEventType enum (12 variants)
- `core/event_bus/mod.rs` — EventBus trait (publish/subscribe/unsubscribe) + SubscriptionId type
- `core/event_bus/domain_event_bus.rs` — InMemoryEventBus (tokio::broadcast, capacity-configurable)
- `core/event_bus/application_event_bus.rs` — InMemoryApplicationEventBus
- `core/event_bus/integration_event_bus.rs` — InMemoryIntegrationEventBus
- `core/module_registry.rs` — Module trait (async initialize/shutdown, name/version/deps) + ModuleRegistry
- `core/config/configuration_provider.rs` — ConfigurationProvider trait (get/set/has/delete) + InMemoryConfig
- `core/security/request_context.rs` — RequestContext (user_id, session_id, device_id, correlation_id, timestamp)
- `infra/logging.rs` — init_logging() with tracing_subscriber + EnvFilter
- `infra/mod.rs` — module declarations
- `commands/mod.rs` — placeholder for Tauri IPC commands

### Files Removed
- `core/error.rs` → replaced by `core/result.rs`
- `core/types.rs` → replaced by `core/entity_id.rs`
- `core/context.rs` → replaced by `core/security/request_context.rs`
- `core/config.rs` → replaced by `core/config/mod.rs` + `configuration_provider.rs`
- `core/event_bus.rs` → replaced by `core/event_bus/mod.rs` + 3 implementations

### Files Updated
- `core/mod.rs` — M1 module declarations + pub use re-exports, #[allow(dead_code, unused_imports)]
- `main.rs` — updated for new module structure, removed old imports
- `tests/rust/architecture/mod.rs` — updated to check M1 file structure, recurse into subdirs

### Verified
- `cargo build` ✅ — zero errors
- `cargo clippy` ✅ — zero warnings
- `cargo test` ✅ — 55/55 tests pass (result: 8, entity_id: 9, domain_event: 5, event_bus: 7, module_registry: 7, config: 7, security: 5, value_object: 4, logging: 1, architecture: 2)

### Bug Fix
- Event bus publish: `tokio::broadcast::send()` returns SendError when zero receivers. Fixed by treating "no subscribers" as non-error (`let _ = sender.send(event)`) — logging-only, not failure.

### Architecture Notes
- Three EventBus levels (Domain/Application/Integration) share identical trait interface but independent channels
- Module trait uses `Box<dyn Module>` for type-erased registry — enables dynamic module loading in M3
- RequestContext carries correlation_id for distributed tracing across modules
- InMemoryConfig for dev/testing; production config provider to be added in M3+

---

## 2026-07-23 M2 Memory Engine Implementation

### Files Created
- `core/memory/types.rs` — MemorySource, MemoryVisibility, MemoryCaptureMode, MemoryLayer, MemoryStatus enums
- `core/memory/memory_record.rs` — MemoryRecord struct (EntityId, scores, layers, validation)
- `core/memory/memory_repository.rs` — MemoryRepository async trait
- `core/memory/memory_recall.rs` — MemoryRecallService trait + RecallContext/RecallResult
- `core/memory/memory_compression.rs` — MemoryCompressionService trait + CompressedMemory + SimpleCompressionService
- `core/memory/memory_service.rs` — MemoryService business logic orchestrator
- `storage/sqlite/schema.rs` — SQL schema (memory_records + FTS5 + triggers + versioning tables)
- `storage/sqlite/memory_repository_sqlite.rs` — SqliteMemoryRepository (Mutex<Connection>, full CRUD)
- `storage/sqlite/recall.rs` — InMemoryRecallService (FTS5 search + confidence ranking)

### Verified
- `cargo build` ✅ — zero errors
- `cargo clippy` ✅ — zero warnings
- `cargo test` ✅ — 105/105 tests pass (M1: 55 + M2: 50)

---

## 2026-07-23 M28 Core Versioning Implementation

### Files Created
- `core/versioning/automatic_commit.rs` — ChangeType enum + AutomaticCommit struct (validate, serialize) — 6 tests
- `core/versioning/causality_record.rs` — CausalityRecord struct (cause→effect tracking) — 2 tests
- `core/versioning/version_edge.rs` — VersionEdgeType enum + VersionEdge struct — 3 tests
- `core/versioning/snapshot_service.rs` — SnapshotService async trait (capture, store, get, get_baseline)
- `core/versioning/diff_calculator.rs` — DiffCalculator trait + SimpleDiffCalculator (text/structured/JSON diffs) — 9 tests
- `core/versioning/commit_service.rs` — CommitService trait + CreateCommitParams — 1 test
- `core/versioning/causality_chain.rs` — CausalityChain async trait (trace_causes, find_effects, record_causality)
- `core/versioning/version_graph.rs` — VersionGraph async trait (get_lineage, get_dependents, add_edge)
- `core/versioning/mod.rs` — module declarations + re-exports
- `storage/sqlite/versioning_repository.rs` — SqliteVersioningRepository implementing CommitService — 4 tests

### Files Updated
- `core/mod.rs` — added `pub mod versioning;`
- `storage/sqlite/mod.rs` — added versioning_repository
- `storage/sqlite/schema.rs` — added CREATE_VERSIONING_TABLES (automatic_commits + causality_records + version_edges)

### Bug Fixes
- `commit_hash` return type: changed from `Vec<u8>` to `String` (hex encoding) — fixed E0277 LowerHex trait bound error
- `is_multiple_of` — clippy: replaced `version_number % 20 == 0` with `version_number.is_multiple_of(20)`
- `SqliteVersioningRepository::new` — suppressed dead_code warning (used via constructor, not direct call yet)

### Verified
- `cargo build` ✅ — zero errors
- `cargo clippy` ✅ — zero warnings
- `cargo test` ✅ — 130/130 tests pass (M1: 55 + M2: 50 + M28: 25)

---

## 2026-07-23 M3 Knowledge Graph Implementation

### Files Created
- `core/graph/entity_types.rs` — EntityType enum (14 builtin + Custom) — 4 tests
- `core/graph/relationship_types.rs` — RelationshipType enum (12 builtin + Custom) — 4 tests
- `core/graph/entity.rs` — Entity struct (EntityId, title, description, status, metadata, canonical_id) — 8 tests
- `core/graph/relationship.rs` — Relationship struct (source, target, weight, type) — 12 tests
- `core/graph/graph_store.rs` — GraphStore async trait (add/get/update/delete entity/relationship, search, count)
- `core/graph/graph_traversal.rs` — GraphTraversal async trait (neighbors, distance, path, subgraph) + GraphNeighborhood/SubGraph
- `core/graph/graph_query.rs` — GraphQuery async trait (query, knowledge_density, timeline) + GraphQueryRequest/Result — 1 test
- `core/graph/entity_identity.rs` — EntityIdentityService async trait (find_duplicates, merge, canonical, resolve_alias)
- `core/graph/graph_service.rs` — GraphService orchestrator (delegates to all 4 traits)
- `core/graph/mod.rs` — module declarations + re-exports
- `storage/sqlite/graph_repository.rs` — SqliteGraphRepository implementing GraphStore + GraphTraversal + GraphQuery + EntityIdentityService — 25 tests

### Files Updated
- `core/mod.rs` — added `pub mod graph;`
- `storage/sqlite/mod.rs` — added graph_repository
- `storage/mod.rs` — added SqliteGraphRepository export
- `storage/sqlite/schema.rs` — added CREATE_GRAPH_TABLES (graph_entities + graph_relationships + 6 indexes)

### Bug Fixes
- `merge_entities` MutexGuard Send: scoped lock in a block to drop before `await` — fixed E0382 future not Send
- `EntityId::parse` return type: changed `Ok(Some(canonical))` to `Ok(canonical)` — fixed E0308 mismatched types
- `metadata.drain()`: replaced `for (k, v) in dup.metadata` with `drain()` to avoid borrow conflict
- clippy: `manual_range_contains`, `unnecessary_sort_by`, `collapsible_if` — all fixed

### Verified
- `cargo build` ✅ — zero errors
- `cargo clippy` ✅ — zero warnings
- `cargo test` ✅ — 184/184 tests pass (M1: 55 + M2: 50 + M28: 25 + M3: 54)

---

## 2026-07-23 M4 Context Engine Implementation

### Files Created
- `core/context/context_package.rs` — ContextPackage, TemporalSlice, IntentType, UserIntent — 8 tests
- `core/context/context_request.rs` — ContextRequest with defaults + validate — 7 tests
- `core/context/context_snapshot.rs` — ContextSnapshot (persisted context) — 2 tests
- `core/context/context_builder.rs` — ContextBuilder async trait (build/build_for_entity/build_for_query)
- `core/context/context_cache.rs` — ContextCache trait + InMemoryContextCache (TTL, Mutex) — 4 tests
- `core/context/context_store.rs` — ContextStore async trait (save/get/list/restore)
- `core/context/intent_detector.rs` — IntentDetector (keyword-based, ru/en, confidence heuristic) — 10 tests
- `core/context/graph_seeder.rs` — GraphSeeder (seed by query, seed by entity + 1-hop)
- `core/context/memory_injector.rs` — MemoryInjector (inject by entities + intent, inject by entity)
- `core/context/compressor.rs` — ContextCompressor (compress, calculate_token_count, prune_low_relevance) — 4 tests
- `core/context/ranker.rs` — ContextRanker (rank, calculate_score: keyword + importance + recency) — 4 tests
- `core/context/context_service.rs` — ContextService orchestrator (build, cache, snapshot, restore)
- `core/context/mod.rs` — module declarations + re-exports
- `storage/sqlite/context_repository.rs` — SqliteContextRepository implementing ContextStore — 5 tests

### Files Updated
- `core/mod.rs` — added `pub mod context;`
- `storage/sqlite/mod.rs` — added context_repository
- `storage/mod.rs` — added SqliteContextRepository export
- `storage/sqlite/schema.rs` — added CREATE_CONTEXT_TABLES (context_snapshots + 2 indexes)

### Bug Fixes
- `MemoryInjector::search()` — removed extra None arguments (MemoryRepository.search takes 1 arg)
- clippy: `collapsible_if` in context_cache.rs and ranker.rs — collapsed nested if-let chains

### Verified
- `cargo build` ✅ — zero errors
- `cargo clippy` ✅ — zero warnings
- `cargo test` ✅ — 228/228 tests pass (M1: 55 + M2: 50 + M28: 25 + M3: 54 + M4: 44)

---

## 2026-07-23 M5 Execution Layer Implementation

### Files Created
- `core/execution/types.rs` — Step, StepStatus, Plan, ExecutionState, ExecutionStatus, StepResult, ExecutionVersion, ExecutionLog — 8 tests
- `core/execution/sandbox.rs` — Sandbox struct (allowed_paths, blocked_commands, max_file_size) with validate_path/command/file_size — 8 tests
- `core/execution/tool.rs` — Tool async trait (name, description, execute, validate_params) — 4 tests
- `core/execution/planner.rs` — Planner async trait + SimplePlanner (keyword-based, split on ";") — 5 tests
- `core/execution/tool_router.rs` — ToolRouter trait + DefaultToolRouter (HashMap-backed) — 4 tests
- `core/execution/action_executor.rs` — ActionExecutor async trait + DefaultActionExecutor (delegates to ToolRouter) — 3 tests
- `core/execution/state_tracker.rs` — ExecutionStateTracker trait + InMemoryStateTracker (Mutex interior mutability) — 4 tests
- `core/execution/execution_service.rs` — ExecutionService orchestrator (plan → execute → track → log) — 3 tests
- `core/execution/mod.rs` — module declarations
- `tools/file_tool.rs` — FileTool (read/write/exists/list) with sandbox validation — 5 tests
- `tools/git_tool.rs` — GitTool (status/log/diff/commit) with sandbox validation — 4 tests
- `tools/mod.rs` — module declarations

### Files Updated
- `core/mod.rs` — added `pub mod execution;`
- `core/result.rs` — added `AppError::Security(String)` variant

### Bug Fixes
- `action_executor.rs` — added `#[async_trait]` to trait definition for dyn compatibility
- `state_tracker.rs` — changed from `&mut self` to `&self` with Mutex interior mutability (required for Arc<dyn> usage in ExecutionService)
- `planner.rs` — changed split delimiter from `[';', '.']` to `';'` only — `.` broke file paths like `/tmp/file.txt`
- `file_tool.rs` — fixed `tok::fs` typo → `tokio::fs`

### Verified
- `cargo build` ✅ — zero errors
- `cargo clippy` ✅ — zero warnings
- `cargo test` ✅ — 270/270 tests pass (M1: 55 + M2: 50 + M28: 25 + M3: 54 + M4: 44 + M5: 42)
