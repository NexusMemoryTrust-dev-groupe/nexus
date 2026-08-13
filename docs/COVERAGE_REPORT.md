# Отчёт по покрытию тестами (план 8.1)

**Статус**: ✅ DONE — все 4 security-critical/stateful модуля ≥90% region coverage.
**Дата**: 2026-08-12
**Инструмент**: `cargo llvm-cov` (stable toolchain; branch coverage недоступен на stable — используется **region coverage** как метрика гейта, line coverage приводится справочно).

## Итоговая сводка (финальный полный прогон)

Команда воспроизведения:

```powershell
$env:NEXUS_FUZZ_SECONDS="3"
cargo llvm-cov --lib --test security --test network_allowlist --test proptest --test fuzz_smoke --test tokenizer_state --test backup_global --test sandbox_live_policy
```

| Модуль | Region до | Region после | Line после | Гейт ≥90% |
|---|---|---|---|---|
| `core/sandbox.rs` | 81.18% | **96.52%** | 96.52% | ✅ |
| `core/tokenizer.rs` | 83.24% | **94.08%** | 96.02% | ✅ |
| `core/backup.rs` | 80.46% | **90.77%** | 93.70% | ✅ |
| `storage/sqlite/versioning_repository.rs` | 78.42% | **91.10%** | 95.40% | ✅ |

## Что добавлено

### `core/sandbox.rs` (81.18% → 96.52%)
- Display ошибок `SandboxError`: verb + path в сообщениях (`no_roots_error_displays_verb_and_path`, `outside_error_with_empty_roots_lists_none`, `unresolvable_error_displays_reason`, `not_absolute_and_reserved_errors_display`).
- `strip_verbatim`: UNC (`\\server\share`) и drive-пути (`C:\...`).
- Edge-кейсы `resolve`: пустые root-строки пропускаются, `..`-escape, разрешающийся за пределы workspace → `Outside`.
- Live-политика (`collect_roots`/`current`/`guard`) — перенесена в **`tests/sandbox_live_policy.rs`** (отдельный процесс; см. раздел «Архитектурное решение»).

### `core/tokenizer.rs` (83.24% → 94.08%)
- `Target::as_str()` для всех движков.
- `configured_model()`: корректные и некорректные значения `FASTEMBED_CACHE_DIR` (слиты в один тест — env-гонка между собой).
- `find_tokenizer_json()`: ранний выход на несуществующем пути.
- tiktoken: fallback на default-словарь для неизвестных моделей.
- `estimate_counts`: CJK-символы.
- Переходы глобального `ACTIVE` — перенесены в **`tests/tokenizer_state.rs`** (отдельный процесс).

### `core/backup.rs` (80.46% → 90.77%)
- `read_manifest`: bad magic, truncation, будущий формат версии, несоответствие длины payload.
- `verify_backup`: payload, не прошедший SQLite integrity check (0xFF-байты в середине → ветка «failed SQLite integrity check»); unreadable path.
- `create_backup`: отказ при невозможности создать директорию назначения; ошибка записи истории (`DROP TABLE backup_history`).
- `delete_backup`/`restore_backup`: отсутствующий файл; `list_backups` на отсутствующей директории; `extract_payload` с ошибкой.
- Вспомогательный `rebuild_container(bytes, new_payload, tweak)` для порчи manifest/payload.
- Глобальные обёртки (`create_backup`/`list_history`/`restore_backup`/`delete_backup` без явного пути) — перенесены в **`tests/backup_global.rs`**.

### `storage/sqlite/versioning_repository.rs` (78.42% → 91.10%)
- Export/import хелперы (план 9.2): roundtrip `insert_commit`/`list_all_commits` для всех `ChangeType` (включая `Deleted`), `insert_version_edge`/`list_all_version_edges` для всех `VersionEdgeType`, `insert_causality`/`list_all_causality`.
- Fallback ветка `row_to_commit` для неизвестной строки change_type → `Modified`.
- **Error-пути через отравленный mutex** (`poisoned_repo`): все 14 методов возвращают `AppError::Internal` при `lock()`-ошибке.
- **Error-пути без схемы** (`bare_repo`): prepare/INSERT-ошибки всех методов на пустом in-memory соединении.
- **Error-пути с битыми строками**: malformed `created_at`/`entity_id` → ошибки маппера в `get_commit`, `get_entity_history`, `get_baseline`, `get_lineage`, `get_dependents`, `trace_causes`, `find_effects`.

## Архитектурное решение: изоляция env-мутирующих тестов

Три теста мутируют процесс-глобальный `LOCALAPPDATA`/`ACTIVE` (читаются `db::db_path()`/`tokenizer::configured_model()` **в момент вызова**). В lib-прогоне они гонялись с параллельными тестами, использующими глобальную БД (`mcp_server`), и падали:

- `database is locked` — tokenizer-тесты открывали временную БД одновременно с redirect'ом.
- `Memory not found` — глобальная БД «сдвигалась» redirect'ом.

**Решение**: все env-мутирующие тесты вынесены в отдельные integration-бинарники (свой процесс — изолированное окружение, ноль гонок):

| Бинарник | Что перенесено |
|---|---|
| `tests/tokenizer_state.rs` | переходы глобального `ACTIVE` |
| `tests/backup_global.rs` | `LOCALAPPDATA`-redirect + глобальные backup-обёртки |
| `tests/sandbox_live_policy.rs` | `LOCALAPPDATA`-redirect + live-политика sandbox |

Lib-тесты больше не трогают окружение процесса; `TEST_APPDATA` mutex из `db.rs` удалён.

## Остаточные непокрытые строки (осознанно)

После достижения гейта остаются ~18–20 строк в `versioning_repository.rs` — недостижимые защитные ветки:

- `serde_json::to_string`/`to_vec` error-замыкания (сериализация `Value`/`Vec<String>` не может упасть).
- `row.get::<_, String>(0)` в мапперах на валидных колонках.
- `EntityId::parse` у строки, совпавшей с WHERE по валидному `EntityId` (всегда успешен).
- `prepare()` error-замыкания для статичных SQL (не может упасть на валидном синтаксисе).

Их покрытие требует инъекции ошибок ниже слоя SQLite и не добавляет защитной ценности.

## Проверка на регрессии

- `cargo test --lib` — 946+ тестов зелёные (включая 28 новых в versioning_repository).
- Integration: `tokenizer_state` 1/1, `backup_global` 1/1, `sandbox_live_policy` 1/1.
- E2E `tests/mcp_stdio_e2e.rs` ожидает ровно 143 инструмента — не затрагивался.
