# MVP Integration Report

## Module Connections

```
┌──────────────────────────────────────────────────────────────────────┐
│                        Tauri IPC Layer                               │
│  commands/memory.rs │ commands/graph.rs │ commands/context.rs        │
│  commands/files.rs  │ commands/workspace.rs │ commands/savings.rs    │
│  commands/ai.rs     │ commands/copilot.rs │ commands/config.rs       │
│  commands/setup.rs                                                       │
└──────┬────────────────────┬────────────────────┬──────────────────────┘
       │                    │                    │
       ▼                    ▼                    ▼
┌──────────────┐   ┌──────────────┐   ┌──────────────┐
│  M2: Memory  │   │  M3: Graph   │   │  M4: Context │
│  SqliteMemory│   │  SqliteGraph │   │  SqliteContext│
│  Repository  │   │  Repository  │   │  Repository  │
└──────┬───────┘   └──────┬───────┘   └──────┬───────┘
       │                   │                  │
       └───────────────────┼──────────────────┘
                           │
                 ┌─────────▼──────────┐
                 │  SQLite (nexus.db)  │
                 │  WAL mode, 11 migrations │
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
                 │  versioning_       │
                 │  listener.rs       │
                 │  M2→M28 bridge     │
                 └────────────────────┘

AI-слой (поверх ядра):
┌──────────────────────────────────────────────────────────────┐
│  ai/copilot.rs    — slash-команды копилота через opencode CLI │
│  ai/mcp_server.rs — MCP stdio-сервер, 66 инструментов        │
│  core/context/    — semantic_search (ONNX), indexer,          │
│                     provenance, auto_graph_builder, export    │
│  core/tokenizer.rs— BPE-токенизатор (exact/estimated)        │
│  core/sandbox.rs  — песочница файловых операций (whitelist)   │
│  core/interpreter/— code/config/markdown/image парсеры        │
└──────────────────────────────────────────────────────────────┘
```

## Module Details

| Module | File | Repository | SQLite Tables | Tauri Commands |
|--------|------|------------|---------------|----------------|
| M1 | `commands/config.rs` | `ConfigurationProvider` (InMemory) | `configuration_kv` | `get_config`, `set_config`, `get_all_config`, `delete_config`, `get_db_stats` |
| M2 | `commands/memory.rs` | `SqliteMemoryRepository` | `memory_records`, `memory_fts`, `memory_semantic_fingerprints` | `get_memories`, `get_memory`, `create_memory`, `search_memories`, `update_memory`, `delete_memory`, `get_project_memories`, `create_project_memory` |
| M3 | `commands/graph.rs` | `SqliteGraphRepository` | `graph_entities`, `graph_relationships`, `entity_snapshots`, `memory_entity_links` | `get_graph`, `get_entity`, `create_entity`, `update_entity`, `delete_entity`, `get_entity_metadata`, `link_entity_to_project`, `delete_relationship`, `get_projects`, `get_project_entities` |
| M4 | `commands/context.rs` | `SqliteContextRepository` | `context_snapshots` | `build_context`, `build_context_for_entity`, `export_context` |
| M28 | `versioning_listener.rs` | `SqliteVersioningRepository` | `automatic_commits`, `causality_records`, `version_edges` | (event-driven, no direct commands) |
| M5 | `core/sandbox.rs` | — (встроен в files/workspace) | — | все файловые команды проходят песочницу |
| Файлы | `commands/files.rs`, `commands/workspace.rs` | — | `workspace_entries` (V8) | `scan_folder`, `read_file`, `write_file`, `create_file`, `delete_file`, `rename_file`, `move_entry`, `pick_files`, `get_workspace_tree`, `add_to_workspace`, `sync_workspace`, `check_stale_projects` и др. |
| Экономия | `commands/savings.rs` | — | `savings_tracking` (V10/V11) | `get_savings_stats`, `record_savings_event`, `get_savings_report`, `get_model_savings` |
| Setup | `commands/setup.rs` | — | — | `setup_status`, `setup_needed`, `install_opencode`, `register_mcp`, `save_api_key`, `select_model`, `complete_setup` |
| AI | `commands/ai.rs`, `commands/copilot.rs` | — | — | `ai_health_check`, `ai_chat_stream`, `ai_list_models`, `copilot_execute`, `copilot_list_commands` |
| MCP | `ai/mcp_server.rs` | — | — | `--mcp` режим: 66 инструментов (stdio) |

## Event Flow

1. **User creates memory** → `create_memory` Tauri command
2. `SqliteMemoryRepository::save_record()` inserts into `memory_records` and `memory_fts`
3. **Event published** → `DomainEvent` with `MemoryRecordCreated` type
4. `versioning_listener` receives event
5. `SqliteVersioningRepository::create_automatic_commit()` inserts into `automatic_commits`
6. Auto-commit linked to original memory record via `triggering_event_id`
7. **Параллельно** `indexer::spawn_index_memory()` (fire-and-forget) считает эмбеддинг и пишет в `memory_semantic_fingerprints` — семантический поиск всегда в актуальном состоянии

## Build Status

| Check | Status |
|-------|--------|
| `cargo build` | ✅ zero errors |
| `cargo clippy --all-targets -- -D warnings` | ✅ zero warnings |
| `cargo test` | ✅ 452 unit + 7 integration |
| `npx tsc --noEmit` | ✅ 0 errors |
| `npx vite build` | ✅ ~6s |
| `npx vitest run` | ✅ 32/32 |
| `npx playwright test` | ✅ 10/10 (smoke + strata) |

## SQLite Schema (nexus.db) — 11 вкомпилированных миграций

| Table | Module | Purpose |
|-------|--------|---------|
| `memory_records` | M2 | Memory CRUD + FTS5 |
| `memory_fts` | M2 | FTS5-индекс памяти |
| `attached_files` | M2 (V2) | Вложения записей |
| `entity_snapshots` | M28 (V5) | Снапшоты сущностей |
| `graph_entities` | M3 | Entity nodes |
| `graph_relationships` | M3 | Entity edges |
| `context_snapshots` | M4 | Saved context packages |
| `workspace_entries` | M3/M9 (V8) | Рабочие области + связи |
| `memory_entity_links` | M3 (V8) | Связи память↔сущность |
| `memory_semantic_fingerprints` | M4 (V9) | Векторные отпечатки (ONNX) |
| `savings_tracking` | Экономия (V10/V11) | Измеренная экономия токенов |
| `automatic_commits` | M28 | Version history |
| `causality_records` | M28 | Causal links |
| `version_edges` | M28 | Version graph |
| `configuration_kv` | M1 | Key-value config |

## Files Modified for Integration

| File | Change |
|------|--------|
| `src-tauri/src/main.rs` | Initialize repos, event bus, setup() hook, 50+ команд, `--mcp` режим |
| `src-tauri/src/commands/memory.rs` | Wired to SqliteMemoryRepository |
| `src-tauri/src/commands/graph.rs` | Wired to SqliteGraphRepository |
| `src-tauri/src/commands/context.rs` | Builds context from M2+M3 |
| `src-tauri/src/commands/mod.rs` | 11 command modules |
| `src-tauri/src/core/versioning/versioning_listener.rs` | M2→M28 bridge |
| `src-tauri/src/core/context/indexer.rs` | Фоновый backfill семантических отпечатков |
| `src-tauri/src/core/context/semantic_search.rs` | Векторный поиск (fastembed + ONNX) |
| `src-tauri/src/core/tokenizer.rs` | BPE-токенизатор (exact/estimated) |
| `src-tauri/src/core/sandbox.rs` | Песочница файловых операций |
| `src-tauri/src/ai/mcp_server.rs` | MCP stdio-сервер, 66 инструментов |
| `src-tauri/src/ai/copilot.rs` | Slash-команды копилота |
| `src/stores/*` | contextStore, projectStore, savingsStore и др. |
