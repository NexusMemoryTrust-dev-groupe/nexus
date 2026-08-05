# M2 Memory Engine — DEVLOG

## 2026-07-23 M2 Implementation

### Files Created

**core/memory/**
- `types.rs` — MemorySource, MemoryVisibility, MemoryCaptureMode, MemoryLayer, MemoryStatus enums (serde + tests)
- `memory_record.rs` — MemoryRecord struct, new() with validation, validate() for scores + title/content, touch()
- `memory_repository.rs` — MemoryRepository async trait (save, get_by_id, get_by_project, search, update, delete, list, count)
- `memory_recall.rs` — MemoryRecallService trait + RecallContext + RecallResult structs
- `memory_compression.rs` — MemoryCompressionService trait + CompressedMemory struct + SimpleCompressionService impl
- `memory_service.rs` — MemoryService (business logic orchestrator: create, get, update, archive, search, recall, compress, set_visibility, promote_layer)
- `mod.rs` — Module declarations + pub use re-exports

**storage/sqlite/**
- `schema.rs` — SQL schema (memory_records table + indexes), FTS5 virtual table, sync triggers, apply_migrations()
- `memory_repository_sqlite.rs` — SqliteMemoryRepository: Mutex<Connection>, full CRUD, FTS5 search, WAL mode, all field roundtrip
- `recall.rs` — InMemoryRecallService: delegates to repository search, ranks by confidence, filters by project/confidence threshold
- `mod.rs` — Module declarations + re-exports

**storage/**
- `mod.rs` — Storage module root

### Files Updated
- `core/mod.rs` — added `pub mod memory;`
- `main.rs` — added `mod storage;`

### Bug Fixed
- `memory_record.rs` validate(): Added title/content emptiness check (was only checking score ranges). Test `create_empty_title_fails` was failing because `validate()` didn't enforce non-empty title/content after manual mutation.

### Architecture Decisions
- FTS5 для full-text search; векторные эмбеддинги добавлены позже как расширение (`semantic_search.rs` + `indexer.rs`, таблица V9) — оба поиска сосуществуют
- Mutex<Connection> для thread safety (rusqlite Connection is !Send)
- IF NOT EXISTS для идемпотентных миграций
- Confidence/Importance scores: f64 in [0.0, 1.0] с явной валидацией

### Расширения (реализовано позже, тем же слоем)
- Семантический поиск: `core/context/semantic_search.rs` + `core/context/indexer.rs` (фоновый backfill)
- Связи память ↔ сущность: `storage/sqlite/memory_entity_links_repository.rs` + MCP-инструменты
- Версионирование: `versioning_listener.rs` автоматически коммитит изменения MemoryRecord в M28

### Verified
- `cargo build` ✅ — zero errors
- `cargo clippy` ✅ — zero warnings
- `cargo test` ✅ — **105/105 tests pass**

### Test Breakdown
| Module | Tests |
|--------|-------|
| M1 (all core) | 55 |
| M2 types | 5 |
| M2 memory_record | 11 |
| M2 memory_compression | 4 |
| M2 memory_recall | 2 |
| M2 memory_service | 9 |
| M2 storage/sqlite schema | 2 |
| M2 storage/sqlite repository | 10 |
| M2 storage/sqlite recall | 6 |
| **Total** | **105** |
