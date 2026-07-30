# Foundation DECISIONS.md

## D001: Project Structure
- Date: 2026-07-22
- Decision: Clean Architecture с четкими границами слоёв (core/, storage/, infra/, commands/)
- Reason: Масштабируемость, тестируемость, замена компонентов без влияния на бизнес-логику

## D002: Error Handling
- Date: 2026-07-22
- Decision: Result<T> с thiserror, без исключений в бизнес-логике
- Reason: Явное управление ошибками, нет скрытых падений, компилятор проверяет обработку

## D003: DI Approach
- Date: 2026-07-22
- Decision: Traits для контрактов, constructor injection
- Reason: Тестируемость (mocking), замена реализаций, слабая связанность

## D004: Logging
- Date: 2026-07-22
- Decision: tracing + structured logging с env-filter
- Reason: Производительность, структурированные данные, гибкая настройка уровня

## D005: Database
- Date: 2026-07-22
- Decision: rusqlite с bundled feature для SQLite
- Reason: Нативная скорость, встроенная БД, нет внешних зависимостей для пользователя

---

# M1 Core Platform — DECISIONS

## D006: Module Lifecycle (async)
- Date: 2026-07-23
- Decision: Module trait с async initialize/shutdown
- Reason: Модули могут инициализировать БД-пулы, broadcast-каналы, таймеры — всё требует async. Async shutdown guarantee корректного завершения.

## D007: EventBus as Trait (не структура)
- Date: 2026-07-23
- Decision: EventBus — trait с publish/subscribe/unsubscribe, три уровня: Domain, Application, Integration
- Reason: Разделение шины по уровням предотвращает «загрязнение» доменных событий application/integration логикой. Trait позволяет подменять InMemory на Redis/NATS в будущем.

## D008: tokio::broadcast для InMemory EventBus
- Date: 2026-07-23
- Decision: Использовать tokio::broadcast как бэкенд для всех трёх уровней EventBus
- Reason: mpmc (multi-producer, multi-consumer) — один EventBus может обслуживать несколько модулей-подписчиков. Zero-cost fan-out, lock-free.

## D009: DomainEvent — struct (не trait)
- Date: 2026-07-23
- Decision: DomainEvent как struct с DomainEventType enum
- Reason: Единая модель для всех уровней EventBus. Enum фиксирует известные типы событий на этапе компиляции. Метаданные (timestamp, payload, metadata map) — универсальны.

## D010: EntityId — UUID v4 string wrapper
- Date: 2026-07-23
- Decision: EntityId как newtype struct вокруг String, валидация UUID v4
- Reason: Type-safe идентификаторы без runtime-ошибок. Простая сериализация в JSON. Методы parse/as_str для интеграции с БД.

## D011: ValueObject trait
- Date: 2026-07-23
- Decision: ValueObject trait с validate() методом
- Reason: DDD pattern — значение валидируется при создании, после чего неизменно. Email, Slug, Money и прочие value objects реализуют этот trait.

## D012: ConfigurationProvider trait
- Date: 2026-07-23
- Decision: ConfigurationProvider trait с get/set/delete, InMemoryConfig для dev
- Reason: Бизнес-логика никогда не обращается к std::env напрямую. Тестируемость: в тестах подменяем InMemoryConfig с предустановленными значениями.

## D013: RequestContext (Security)
- Date: 2026-07-23
- Decision: RequestContext с user_id, session_id, device_id, correlation_id, timestamp
- Reason: Zero Trust: каждый запрос несёт полный контекст безопасности. Correlation ID — для distributed tracing между модулями.

## D014: infra/logging — tracing с EnvFilter
- Date: 2026-07-23
- Decision: tracing_subscriber::fmt + EnvFilter через RUST_LOG
- Reason: Структурированные логи, гибкая фильтрация по модулям (RUST_LOG=nexus::core::event_bus=debug), production-ready.

## D015: Broadcast "no subscribers" = OK
- Date: 2026-07-23
- Decision: publish() при отсутствии подписчиков возвращает Ok(()), не Err
- Reason: Публикация событий не должна падать если подписчики ещё не подключились. Логирование «пустой шины» — на уровне tracing, не на уровне ошибки.

---

# M28 Core Versioning — DECISIONS

## D027: Diff-based Versioning (Delta Storage)
- Date: 2026-07-23
- Decision: Хранить только дельты (diff), baseline-снапшот каждые 20 версий
- Reason: Экономия хранилища. Baseline snapshots позволяют быстро восстановить состояние без реконструкции цепочки дельт. Порог 20 — баланс между частотой снапшотов и размером.

## D028: Commit Hash via DefaultHasher
- Date: 2026-07-23
- Decision: commit_hash вычисляется через std::collections::hash_map::DefaultHasher → hex string
- Reason: Нет необходимости в криптографической хэш-функции для versioning integrity — DefaultHasher быстр и sufficient для обнаружения несанкционированных изменений. Если понадобится SHA-256 — замена через trait.

## D029: CommitService as Async Trait
- Date: 2026-07-23
- Decision: CommitService — async trait с create_commit, get_commit, get_entity_history
- Reason: DI через trait: InMemory для тестов, SQLite для продакшена. Async — не блокирует tokio runtime при записи в БД.

## D030: CausalityRecord for Cross-Entity Tracking
- Date: 2026-07-23
- Decision: CausalityRecord отслеживает cause→effect связи между версиями разных сущностей
- Reason: Когда Memory вызывает VersionGraph (M28) → это causa chain. CausalityChain trait позволяет трассировать межмодульные зависимости — critical для debug и аудита.

---

# M3 Knowledge Graph — DECISIONS

## D031: Entity as Universal World Object
- Date: 2026-07-23
- Decision: Entity — универсальный объект графа с EntityType enum (14 базовых + Custom)
- Reason: Закрытый список типов (Person, Project, Task...) гарантирует предсказуемость, Custom(String) — расширяемость. Entity携带 canonical_id для merge/dedup.

## D032: Relationship Weight Range [0.0, 1.0]
- Date: 2026-07-23
- Decision: Relationship.weight — f64 в диапазоне [0.0, 1.0], валидация при создании и обновлении
- Reason: Вес определяет силу связи (создал = 1.0, упомянул = 0.2). Фиксированный диапазон позволяет корректно считать knowledge density.

## D033: SQLite Persistence without petgraph
- Date: 2026-07-23
- Decision: Граф хранится в SQLite (entities + relationships таблицы), traversal через BFS на SQL-запросах
- Reason: petgraph добавляет in-memory overhead без преимуществ для local-first desktop app. SQLite с индексами на source/target entity_id достаточно быстр для NFR-PERF-005 (traversal < 1s на 100k). petgraph можно добавить позже как optimization layer.

## D034: BFS Traversal with Depth Limit
- Date: 2026-07-23
- Decision: GraphTraversal.get_neighbors использует BFS с configurable depth, get_distance/find_path — BFS с early exit
- Reason: BFS гарантирует кратчайший путь. Depth limit 10 предотвращает runaway queries. Mutex<Connection> блокирует на время запроса — acceptable для single-writer SQLite.

## D035: Entity Merge via Canonical ID
- Date: 2026-07-23
- Decision: merge_entities() перенаправляет связи, объединяет metadata, помечает дубли как Merged с canonical_id
- Reason: Deduplication — критическая функция для graph quality. Relationships перенаправляются на primary entity, metadata объединяется через entry().or_insert(). Duplicates получают status=Merged + canonical_id pointing to primary.

## D036: GraphQuery as Separate Trait
- Date: 2026-07-23
- Decision: GraphQuery — отдельный trait от GraphStore (query, knowledge_density, timeline)
- Reason: Разделение ответственности: GraphStore = CRUD, GraphTraversal = навигация, GraphQuery = аналитика. Каждый trait тестируется независимо.

---

# M4 Context Engine — DECISIONS

## D037: ContextPackage as Computed Structure
- Date: 2026-07-23
- Decision: ContextPackage — вычислимая структура (entities + relationships + memory_records + temporal_slice + scores + intent), а не строка
- Reason: Это ключевое отличие от простой конкатенации истории. Package позволяет программно ранжировать, сжимать и кешировать контекст.

## D038: Keyword-Based Intent Detection
- Date: 2026-07-23
- Decision: IntentDetector классифицирует запрос по ключевым словам (рус/англ), confidence — эвристика по длине
- Reason: M4 — базовая реализация. AI-based intent detection будет в M7 (AI Gateway). Keyword approach sufficient для MVP и не требует внешних вызовов.

## D039: InMemory Context Cache with TTL
- Date: 2026-07-23
- Decision: ContextCache — InMemory HashMap с TTL-based expiration, Mutex для thread safety
- Reason: Local-first desktop app: Redis/NATS не применимы. TTL предотвращает устаревшие контексты. Mutex acceptable для single-writer desktop.

## D040: Token Count as Rough Estimate
- Date: 2026-07-23
- Decision: calculate_token_count() использует len()/4 как грубую оценку токенов
- Reason: Точный подсчёт требует tiktoken-rs (Python binding) или аналога. Для M4 грубой оценки достаточно для NFR-CTX-003 (70-90% экономия). Точный токенизатор добавится в M7.

## D041: SQLite for Context Snapshots
- Date: 2026-07-23
- Decision: ContextSnapshot хранится в SQLite как JSON (package_json), entity_id с индексом
- Reason: Context snapshots — read-heavy workload, SQLite WAL sufficient. JSON serialization позволяет гибкую эволюцию схемы без миграций.

## D042: 6-Step Builder Pipeline
- Date: 2026-07-23
- Decision: ContextBuilder Pipeline: Intent → Seeding → Expansion → Injection → Compression → Ranking
- Reason: Каждый шаг — отдельная ответственность. Pipeline позволяет заменять/настраивать шаги независимо. Тестируется каждый шаг отдельно.

---

# M5 Execution Layer — DECISIONS

## D043: Tool as Async Trait
- Date: 2026-07-23
- Decision: Tool — async trait с name(), description(), execute(), validate_params(). Все инструменты — Box<dyn Tool> или Arc<dyn Tool>.
- Reason: Динамическая диспетчеризация позволяет регистрировать новые инструменты без изменения кода executor'а. Async обязателен — файловые операции и git команды блокируются на I/O.

## D044: SimplePlanner — Keyword-Based (без LLM)
- Date: 2026-07-23
- Decision: SimplePlanner разбивает intent по ";" на отдельные шаги, первое слово = action, остальное = target.
- Reason: M5 не зависит от M7 (AI Gateway). Keyword-based подход sufficient для базовых workflow. AI-powered планирование будет добавлено в M7.

## D045: Sandbox for Path/Command Validation
- Date: 2026-07-23
- Decision: Sandbox — структура с allowed_paths, blocked_commands, max_file_size. Каждый Tool проходит через sandbox.validate_path() перед выполнением.
- Reason: SEC-001: Defense in Depth. Даже если tool registration неверен, sandbox блокирует опасные операции. Blocked commands включают "rm -rf /" и "sudo" по умолчанию.

## D046: Interior Mutability for StateTracker
- Date: 2026-07-23
- Decision: ExecutionStateTracker trait использует `&self` (не `&mut self`) с Mutex внутри InMemoryStateTracker.
- Reason: ExecutionService хранит `Arc<dyn ExecutionStateTracker>`. Arc не позволяет &mut self. Mutex interior mutability — стандартный паттерн для thread-safe state в Rust.

## D047: DefaultActionExecutor Stops on First Failure
- Date: 2026-07-23
- Decision: execute_plan() прерывает выполнение при первом failed step, ставит ExecutionStatus::Failed.
- Reason: Fail-fast предотвращает каскадные ошибки. Replan через ExecutionService.replan() позволяет восстановиться после анализа ошибки.

## D048: AppError::Security for Sandbox Violations
- Date: 2026-07-23
- Decision: Добавлен `AppError::Security(String)` в result.rs для sandbox violations.
- Reason: Безопасностные ошибки отделены от Validation (невалидные данные) и Internal (баги). Это позволяет differentiated error handling — security errors могут логироваться по-другому.
