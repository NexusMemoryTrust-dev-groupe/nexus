# Disaster Recovery — Nexus

Статус: актуально для v1.1.0. Пункт 9.1 Production Readiness Plan.

Этот документ описывает, как обнаружить, восстановить и проверить данные Nexus
в четырёх сценариях отказа: повреждение базы данных, сбой миграции, сломанное
обновление и повреждение эмбеддингов. Каждый сценарий следует одному циклу:
**detection → recovery → verification**.

---

## 1. Карта данных и активов

| Ассет | Где живёт | Как восстанавливается |
|---|---|---|
| Основная БД (память, граф, аудит, снапшоты, версии, скиллы) | `%LOCALAPPDATA%\Nexus\nexus.db` (Windows), `~/.nexus/nexus.db` (иначе) | `.nexusbackup` → restore |
| WAL/SHM файлы | рядом с `.db` (`nexus.db-wal`, `nexus.db-shm`) | часть SQLite, восстанавливаются вместе с БД, автоочистка |
| Бэкапы | `<соседняя папка БД>\backups\*.nexusbackup` | `create_backup` / `restore_backup` |
| Журнал бэкапов | таблица `backup_history` (V31) в БД | пересоздаётся миграцией |
| Эмбеддинг-модель (ONNX, AllMiniLML6V2, 384-dim) | `<папка БД>\.fastembed_cache` (или `FASTEMBED_CACHE_DIR`) | повторная загрузка; fallback — хэш-векторы |
| Семантические фингерпринты | `memory_semantic_fingerprints` (V9), `document_fingerprints` (V14) | пересоздаются из текста памяти автоматически |
| Миграции схемы | `src/storage/sqlite/migrations/V*.sql` (V1–V33) | `apply_migrations` (идемпотентно, атомарно) |

Ключевые прагмы соединения (`db.rs`): `journal_mode=WAL`, `foreign_keys=ON`,
`synchronous=NORMAL`, busy_timeout 5 c.

---

## 2. Обнаружение — Doctor

Все проверки собраны в `core/doctor.rs`. Запуск:

```text
cargo run --bin nexus_doctor
cargo run --bin nexus_doctor -- --json     # машинно-читаемый отчёт
```

Чеки: `db_file` (файл существует), `db_open` (открывается),
`migrations` (версия схемы vs latest), `integrity` (`PRAGMA integrity_check`),
`foreign_keys` (`PRAGMA foreign_key_check`), `memory_records` (счётчик),
`fts_sync` (memory_records == memory_fts), `semantic_index` (покрытие >= 99%),
`graph_orphans` (нет повисших рёбер), `backup` (таблица `backup_history`
присутствует).

Любой чек со статусом `Error` → `healthy=false`. Доктор — первый шаг каждого
сценария ниже.

---

## 3. Сценарий A — повреждение базы данных (DB corruption)

### Detection
- `nexus_doctor` → `integrity` = **Error** (`integrity_check reported: ...`).
- Приложение падает с `cannot open database` / `database disk image is malformed`.
- `verify_backup` падает на проверке целостности извлечённого payload.

### Recovery
1. **Остановить записи** в повреждённую БД (закрыть приложение).
2. Найти последний валидный бэкап: `list_backups(<dir>)` → выбрать по времени,
   проверить `verify_backup(path)` (checksum SHA-256 + `PRAGMA integrity_check`
   на распакованном файле — `core/backup.rs`).
3. Восстановить:
   ```text
   restore_backup_at(db_path, backup_path)
   ```
   Внутри (`restore_backup_inner`):
   - полная верификация бэкапа (fail-fast),
   - проверка совместимости схемы: бэкап **новее** билда → отказ; **старее** →
     предупреждение + автоматический safety-бэкап текущего состояния,
   - safety-бэкап текущего состояния в `<backups>`,
   - применение снапшота через **SQLite Online Backup API** (`Connection::restore`)
     — атомарно, безопасно с WAL, без окна полузаписанной БД,
   - повторный прогон `apply_migrations` (идемпотентный) — догон до актуальной схемы,
   - запись в `backup_history` со статусом `restored`.

### Verification
1. `cargo run --bin nexus_doctor` → `integrity` = **ok**, `migrations` = latest,
   `graph_orphans` = 0, `healthy=true`.
2. Точечная сверка данных: `SELECT COUNT(*) FROM memory_records` → совпадает
   с manifest бэкапа (`memory_count` в `RestoreReport`).
3. Smoke-тест: создать/найти запись памяти через MCP или UI.

---

## 4. Сценарий B — сбой миграции (failed migration)

### Detection
- `nexus_doctor` → `migrations` = **Warning** (schema v<N> behind latest) или
  **Error** (schema v>N+1 — новее, чем поддерживает билд).
- Лог: ошибка `apply_migrations` при старте.

### Recovery
Механизм миграций спроектирован атомарно:
- каждая миграция — отдельный `.sql`-файл (V1–V33),
- сбой миграции **откатывается в транзакции** (тест
  `failed_migration_rolls_back_atomically`),
- после исправления SQL миграция **повторно применяется** (тест
  `failed_migration_retries_after_fix`).

Шаги:
1. `cargo run --bin nexus_doctor` → определить, какая версия схемы стоит.
2. Если схема **отстаёт**: просто запустить приложение — `apply_migrations`
   применит недостающие миграции идемпотентно. Проверить доктором.
3. Если миграция **падает**: исправить/дополнить проблемный `V*.sql`,
   перезапустить. Транзакционный откат гарантирует отсутствие частичного
   применения.
4. Если схема **новее** билда (откат приложения): см. Сценарий C, шаг 2 —
   restore более старого бэкапа или обновление билда.

### Verification
- `nexus_doctor` → `migrations` = **ok**, `schema at latest version v33`.
- `cargo test --lib storage::sqlite::schema` (тесты отката/повтора миграций).

---

## 5. Сценарий C — сломанное обновление (broken update)

Симптомы: приложение не стартует после установки новой версии; БД открывается,
но схема несовместима; новые таблицы отсутствуют; краш в момент миграции.

### Detection
- `nexus_doctor` → `migrations` = **Error** (`schema vN is newer than this build`).
- Логи запуска: паника в `apply_migrations`, ошибка открытия БД.

### Recovery
1. **Не перезаписывать БД.** Данные физически целы — проблема в несоответствии
   схемы и билда.
2. **Схема новее билда** (откат версии приложения): `restore_backup_at` откажет
   бэкап новее билда — использовать бэкап, созданный **до** обновления, либо
   установить актуальный билд приложения. Safety-бэкап создаётся автоматически
   перед каждым restore.
3. **Схема старее билда** (обновление прервано): просто перезапустить — миграции
   догонят схему; при необходимости `restore_backup_at` догонит до нужной версии.
4. Альтернативный путь (портативное восстановление данных, без файлового
   restore): `ProjectExport` → `to_json()` → загрузить JSON в свежую БД через
   `import_project()` (пункт 9.2, `core/export.rs`) — версионный формат
   сохраняет все секции (memories, entities, relations, decisions, skills,
   provenance, snapshots).

### Verification
- `nexus_doctor` → `healthy=true`, все чеки OK/Warning.
- Приложение стартует, команды MCP отвечают (e2e-тест).
- `cargo test --lib core::export` (roundtrip) — подтверждает, что экспортные
  данные переносимы между билдами.

---

## 6. Сценарий D — повреждение эмбеддингов (corrupted embeddings)

Эмбеддинги хранятся **двумя слоями**:
1. **Модель** — ONNX файл в `.fastembed_cache` (рядом с БД; переопределяется
   переменной `FASTEMBED_CACHE_DIR`). При повреждении/недоступности движок
   **бесшовно деградирует** на детерминированные хэш-векторы (fallback,
   `semantic_search.rs`) — поиск работает, но семантическое качество ниже.
2. **Данные** — векторы JSON в `memory_semantic_fingerprints` /
   `document_fingerprints`, вычисленные из текста памяти. Они производны:
   при повреждении пересоздаются из исходного текста.

### Detection
- `nexus_doctor` → `semantic_index` = **Warning** (покрытие < 99% — бэкграунд
  индексатор догоняет).
- Лог: `Failed to load ONNX embedding model ({}), using fallback` — модель
  повреждена/отсутствует.
- Результаты семантического поиска деградировали (только лекс. канал).

### Recovery
1. **Повреждённая модель**: удалить содержимое `.fastembed_cache` (или
   переопределить `FASTEMBED_CACHE_DIR` на новую папку) → при следующем поиске
   модель скачается заново. Пока модель не загружена, работает fallback.
2. **Повреждённые фингерпринты**: фингерпринты пересоздаются на лету при
   записи/обновлении памяти (`store_fingerprint`). Для принудительной
   пересборки — запустить повторное индексирование памяти
   (перезапись записей памяти или очистка таблицы фингерпринтов — данные
   в `memory_records` при этом не затрагиваются).
3. Убедиться, что поиск вычисляет векторы детерминированно: fallback —
   хэш-векторы, production — ONNX AllMiniLML6V2 (384-dim, LRU-кэш).

### Verification
- `nexus_doctor` → `semantic_index` = **ok** (покрытие >= 99%).
- `cargo test --lib core::context::semantic_search` (фингерпринты + косинус +
  fallback).
- Запрос через семантический поиск возвращает релевантные памяти.

---

## 7. Резюме: порядок действий при любом сбое

1. **Detect**: `cargo run --bin nexus_doctor [-- --json]` — зафиксировать какие
   чеки в Error/Warning.
2. **Isolate**: остановить запись (закрыть приложение), не перезаписывать
   `nexus.db*`.
3. **Recover**: по таблице сценариев выше; restore всегда идёт через
   `verify_backup` → safety-бэкап → Online Backup API (атомарно).
4. **Verify**: doctor `healthy=true` + точечная сверка данных + smoke-тест
   (создать/найти память).

---

## 8. Что НЕ является recovery (и почему)

- **Копирование `nexus.db` файлом поверх активной БД** — нет, при открытых
  соединениях и WAL это создаёт окно полузаписанного состояния; используйте
  `restore_backup_at` (Online Backup API) или `create_backup` перед ручными
  манипуляциями.
- **Удаление `nexus.db` «чтобы создать заново»** — только как крайняя мера при
  тотальном повреждении без бэкапа; приводит к потере всех данных. Документируйте
  и восстанавливайте из последнего экспорта (9.2), если он есть.
- **Редактирование `.sql` миграций вручную в установленной БД** — миграции
  применяются атомарно; ручные правки схемы обходят журнал и могут сломать
  проверку `migrations`.

---

## 9. Связанные ресурсы

- `src-tauri/src/core/backup.rs` — create/verify/list/restore, журнал.
- `src-tauri/src/core/doctor.rs` — все чеки здоровья.
- `src-tauri/src/core/export.rs` — портативный экспорт/импорт (9.2).
- `src-tauri/src/core/context/semantic_search.rs` — эмбеддинги и fallback.
- `src-tauri/src/storage/sqlite/schema.rs` + `migrations/` — миграции.
- `src-tauri/src/db.rs` — пути и прагмы соединений.