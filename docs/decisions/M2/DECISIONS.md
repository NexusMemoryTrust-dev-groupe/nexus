# M2 Memory Engine — DECISIONS

## D016: MemoryRecord as Core Entity
- Date: 2026-07-23
- Decision: MemoryRecord — struct с EntityId, DateTime<Utc>, валидацией через validate()
- Reason: Единица памяти системы. Immutable history: любое изменение создаёт версию (V1→V2→V3, как Git). Валидация при создании + при обновлении.

## D017: Confidence/Importance Score Range [0.0, 1.0]
- Date: 2026-07-23
- Decision: ConfidenceScore и ImportanceScore — f64 в диапазоне [0.0, 1.0]
- Reason: NFR: слух 0.2 / документ 0.8 / официальное решение 1.0. Диапазон фиксируется на этапе компиляции через validate().

## D018: MemoryRepository as async Trait
- Date: 2026-07-23
- Decision: MemoryRepository — async trait с save/get/update/delete/list/search/count
- Reason: DI через trait: в тестах подменяем InMemory, в продакшене — SQLite. Async позволяет не блокировать tokio runtime.

## D019: SQLite + FTS5 for Storage
- Date: 2026-07-23
- Decision: rusqlite (bundled) + FTS5 virtual table + WAL mode
- Reason: NFR-REL-004: WAL для concurrent reads. FTS5 для full-text search. Параметризованные запросы для NFR-SEC.

## D020: FTS5 Sync Triggers
- Date: 2026-07-23
- Decision: AFTER INSERT/UPDATE/DELETE триггеры для синхронизации memory_fts с memory_records
- Reason: FTS5 — отдельная таблица, триггеры автоматически обновляют индекс. Нет ручного sync — нет багов.

## D021: Mutex<Connection> for Thread Safety
- Date: 2026-07-23
- Decision: SqliteMemoryRepository хранит Mutex<Connection>
- Reason: rusqlite Connection не Send+Sync. Mutex блокирует на время запроса — acceptable для single-writer SQLite.

## D022: InMemoryRecallService (Not Vector Search)
- Date: 2026-07-23
- Decision: RecallService на базе FTS5 + confidence ranking, без vector embeddings
- Reason: M2 — базовая реализация. Vector search (hnsw-rs / lancedb) будет в M4+ когда появятся AI embeddings. FTS5 для M2 sufficient.

## D023: MemoryLayer Promotion
- Date: 2026-07-23
- Decision: Raw → Knowledge → Decision → Wisdom — promotion через MemoryService.promote_layer()
- Reason: DDD pattern: память «зрееет» по мере обработки. Promotion — явная операция с audit context.

## D024: MemoryCompressionService as Trait
- Date: 2026-07-23
- Decision: MemoryCompressionService — async trait с compress/decompress
- Reason: SimpleCompressionService — заглушка для M2. Полная реализация (с AI summary) будет позже. Trait позволяет подмену без изменения business logic.

## D025: SQLite Schema Idempotent Migrations
- Date: 2026-07-23
- Decision: Все CREATE TABLE/INDEX/TRIGGER используют IF NOT EXISTS
- Reason: Миграции безопасно запускать повторно. Нет ошибок при перезапуске приложения.

## D026: MemoryRecord Validation in Both new() and validate()
- Date: 2026-07-23
- Decision: new() проверяет title/content на пустоту + validate() проверяет title/content + scores
- Reason: new() — guard при создании. validate() — guard при обновлении (когда title/content могут быть изменены после создания). Двойная защита.
