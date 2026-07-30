# MVP Integration Report

## Module Connections

```
┌─────────────────────────────────────────────────────────┐
│                    Tauri IPC Layer                       │
│  commands/memory.rs │ commands/graph.rs │ commands/context.rs │
└────────┬────────────┴────────┬───────────┴────────┬──────┘
         │                    │                    │
         ▼                    ▼                    ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│  M2: Memory     │  │  M3: Graph      │  │  M4: Context    │
│  SqliteMemory   │  │  SqliteGraph    │  │  SqliteContext   │
│  Repository     │  │  Repository     │  │  Repository      │
└────────┬────────┘  └────────┬────────┘  └────────┬────────┘
         │                    │                    │
         └────────────────────┼────────────────────┘
                              │
                    ┌─────────▼──────────┐
                    │  SQLite (nexus.db)  │
                    │  WAL mode           │
                    └─────────┬──────────┘
                              │
                    ┌─────────▼──────────┐
                    │  M28: Versioning   │
                    │  SqliteVersioning  │
                    │  Repository        │
                    └─────────┬──────────┘
                              │
                    ┌─────────▼──────────┐
                    │  Event Bus         │
                    │  (tokio::broadcast)│
                    └─────────┬──────────┘
                              │
                    ┌─────────▼──────────┐
                    │  versioning_        │
                    │  listener.rs        │
                    │  M2→M28 bridge     │
                    └────────────────────┘
```

## Module Details

| Module | File | Repository | SQLite Tables | Tauri Commands |
|--------|------|------------|---------------|----------------|
| M2 | `commands/memory.rs` | `SqliteMemoryRepository` | `memory_records`, `memory_fts` | `get_memories`, `get_memory`, `create_memory`, `search_memories` |
| M3 | `commands/graph.rs` | `SqliteGraphRepository` | `graph_entities`, `graph_relationships` | `get_graph`, `get_entity`, `create_entity` |
| M4 | `commands/context.rs` | `SqliteContextRepository` | `context_snapshots` | `build_context` |
| M28 | `versioning_listener.rs` | `SqliteVersioningRepository` | `automatic_commits`, `version_edges` | (event-driven, no direct commands) |

## Event Flow

1. **User creates memory** → `create_memory` Tauri command
2. `SqliteMemoryRepository::save_record()` inserts into `memory_records` and `memory_fts`
3. **Event published** → `DomainEvent` with `MemoryRecordCreated` type
4. `versioning_listener` receives event
5. `SqliteVersioningRepository::create_automatic_commit()` inserts into `automatic_commits`
6. Auto-commit linked to original memory record via `triggering_event_id`

## Build Status

| Check | Status |
|-------|--------|
| `cargo build` | ✅ 1 warning (dead_code: `ai_health_check`) |
| `cargo test` | ✅ 272/272 passed |
| `npx tsc --noEmit` | ✅ 0 errors |
| `npx vite build` | ✅ 1.20s, 33KB CSS, 217KB JS |

## SQLite Schema (nexus.db)

| Table | Module | Purpose |
|-------|--------|---------|
| `memory_records` | M2 | Memory CRUD + FTS5 |
| `graph_entities` | M3 | Entity nodes |
| `graph_relationships` | M3 | Entity edges |
| `context_snapshots` | M4 | Saved context packages |
| `automatic_commits` | M28 | Version history |
| `causality_records` | M28 | Causal links |
| `version_edges` | M28 | Version graph |
| `configuration_kv` | M1 | Key-value config |

## Files Modified for Integration

| File | Change |
|------|--------|
| `src-tauri/src/main.rs` | Initialize repos, event bus, setup() hook |
| `src-tauri/src/commands/memory.rs` | Wired to SqliteMemoryRepository |
| `src-tauri/src/commands/graph.rs` | Wired to SqliteGraphRepository |
| `src-tauri/src/commands/context.rs` | New: builds context from M2+M3 |
| `src-tauri/src/commands/mod.rs` | Added context module |
| `src-tauri/src/core/versioning/versioning_listener.rs` | New: M2→M28 bridge |
| `src-tauri/src/core/versioning/mod.rs` | Added versioning_listener |
| `src/stores/contextStore.ts` | New: connects to build_context |
