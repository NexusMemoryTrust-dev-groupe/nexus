# Threat Model — Nexus Memory Trust

Статус: актуально для v1.1.0. Пункт 4.2 Production Readiness Plan.

Этот документ описывает угрозы, которые Nexus считает реальными для своей
архитектуры (локальное десктоп-приложение с SQLite, MCP-сервером и
ИИ-агентами), какие контрмеры уже внедрены и какие зоны остаются
за пределами ответственности продукта.

---

## 1. Ассеты (что защищаем)

| Ассет | Где живёт | Критичность |
|---|---|---|
| Память (memory records, проект/личная) | `nexus.db` (SQLite, локально) | высокая |
| Секреты в памяти (API-ключи, пароли, токены) | записи памяти, конфиг | высокая |
| Файлы проекта (workspace) | диск пользователя | средняя |
| Audit-журнал (кто/что менял) | `audit_events` в БД | высокая |
| MCP-интерфейс (команды от агентов) | stdio | средняя |
| Граф знаний (entities/relationships) | `graph_*` таблицы | средняя |

## 2. Зоны доверия

```
[Пользователь] ── UI (Tauri) ──┐
                              ├── Nexus core (Rust) ── SQLite (локально)
[ИИ-агент] ── MCP (stdio) ────┘
```

- **Внутренняя зона**: Rust-процесс Nexus + локальная БД. Полное доверие коду.
- **Пользователь**: доверенный, но не привилегированный — не должен случайно
  удалить данные без подтверждения (backup/restore, sandbox).
- **Агент (MCP)**: НЕ доверенный. Может галлюцинировать пути, аргументы,
  инъекции. Главный adversarial-актор этой модели.

## 3. Угрозы и контрмеры

### T1 — Path traversal через MCP-инструменты (критично)

**Атака**: `nexus_write_file("C:\..\..\Windows\System32\drivers\etc\hosts")`
или `%2e%2e`-кодировка, UNC, symlink/junction, reserved device names (`NUL`),
`C:\Proj` vs `C:\Project2` (prefix-баг).

**Контрмеры** (реализованы в `core/sandbox.rs`):
- Канонизация (`std::fs::canonicalize`, следует за symlink/junction).
- Отказ на `..` в несуществующем хвосте пути.
- Покомпонентное сравнение, не строковый префикс.
- Reserved device names (`CON`, `NUL`, `COM1..`, `LPT1..`) отклоняются.
- Relative paths запрещены — только абсолютные.
- Разрешены только корни: workspace, `sandbox.extra_roots`, data dir.

**Проверка**: `tests/security.rs` — adversarial suite
(`rejects_dot_dot_escape`, `rejects_url_encoded_traversal`,
`rejects_backslash_escape_forms`, `rejects_reserved_device_names`,
`rejects_relative_and_unc_escape`).

### T2 — Утечка секретов в логи/аудит/MCP-вывод (высокая)

**Атака**: память содержит `sk-...` или JWT; сообщение аудита, лог или ответ
MCP печатает значение целиком.

**Контрмеры** (реализованы в `core/security/secrets.rs`):
- `looks_like_secret` — детекция по форме: JWT (3 сегмента), `sk-`/`ghp_`/
  `AKIA`/`xoxb-` и т.п., PEM-блоки, `key=value` с секретным ключевым словом.
- `redact(text, known)` — замена известных значений на `[REDACTED:kind]`.
- `redact_value(value)` — маскирование значения целиком для API-ответов.

**Проверка**: `tests/security.rs` (`detects_secret_shapes`,
`redacts_secrets_from_audit_text`, `injection_with_embedded_secret...`),
unit-тесты `secrets.rs` (11).

**Остаточный риск**: значения, которые не известны движку и не имеют
характерной формы, не маскируются. Митигация: не логировать тела MCP-запросов
целиком.

### T3 — Prompt injection через память/файлы (средняя)

**Атака**: злонамеренный текст ("Ignore all previous instructions...")
в файле проекта, индексируется в память и попадает в контекст агента.

**Контрмеры**:
- Память — это данные; классификация слоёв (`LayerClassifier`) не исполняет
  инструкции.
- Agent firewall (`agent_permissions.rs`): deny-паттерны на секреты,
  политики видимости/слоёв для агентов.
- Secrets-редaкция снимает «крючок» инъекции, ворующей секреты.

**Проверка**: `tests/security.rs`
(`prompt_injection_payload_is_treated_as_data_not_instruction`,
`injection_with_embedded_secret_still_redacts_only_the_secret`).

**Остаточный риск**: полная защита от социальной инженерии модели невозможна
на уровне продукта; снижается периметром (sandbox не даёт агенту выйти за
workspace).

### T4 — Несанкционированный доступ агента к памяти (средняя)

**Атака**: агент запрашивает секретную/чужую память через MCP.

**Контрмеры**:
- `RequestContext` (`core/security/request_context.rs`): user/session/device/
  correlation_id для каждой операции.
- Agent Passport (`core/knowledge/agent_passport.rs`): идентичность агента,
  role, memory scope, trust score — identity ≠ authorization.
- `agent_permissions.rs`: `assess_agent_access` — Deny/Allow по
  visibility/layer/deny_patterns.

### T5 — Потеря/повреждение данных (высокая)

**Атака**: сбой во время записи, битая миграция, случайное удаление.

**Контрмеры**:
- Backup/restore (`core/backup.rs`): SQLite Online Backup API, атомарные
  снапшоты, restore → temp → integrity → swap, версии, checksum.
- Миграции с тестами отказа на каждом шаге (`schema.rs`).
- `nexus doctor` — проверка целостности.

### T6 — Подделка audit-журнала (средняя)

**Атака**: изменение/удаление событий аудита через MCP.

**Контрмеры**:
- Audit пишется отдельным сервисом с полными событиями
  (memory changed, permission changed, firewall denied, superseded...).
- Вектор записи отделён от вектора чтения инструментами.

**Остаточный риск**: SQLite без WAL-журнала криптографической подписи —
локальный злоумышленник с полным доступом к диску может модифицировать БД.
Для десктоп-приложения принят как приемлемый (нарушитель уже владеет машиной).

## 4. Не в scope

- Шифрование БД на диске (полнодисковое шифрование — ответственность ОС).
- Сетевая аутентификация (Nexus не слушает сеть; MCP — только stdio).
- Защита от физического доступа к машине.

## 5. Запуск security-проверок

```bash
cargo test --test security        # adversarial suite (path traversal, secrets, injection)
cargo test --bin nexus security:: # unit-тесты secrets/request_context
cargo run --bin nexus_conflict_bench  # conflict engine gate
```
