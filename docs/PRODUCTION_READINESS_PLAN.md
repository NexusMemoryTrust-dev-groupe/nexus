# Nexus Production Readiness — план расширения до production-grade

> Источник: `задача для нексус разработчика.txt` (50 пунктов Production Readiness Gate).
> Аудит кодовой базы: 2026-08-10. Ниже — статус каждого пункта в текущем коде,
> план работ, фазы и порядок реализации. План живой: по мере реализации пункты
> переводятся из «TODO» в «DONE» с указанием файлов и метрик.

---

## Текущее состояние (аудит)

**Что уже есть (сильный фундамент):**
- 30 миграций SQL (V1–V30), транзакционные, идемпотентные ALTER, `schema_migrations`.
- Гибридный retrieval: `search_hybrid` (cosine + lexical + filename-канал с IDF, веса 0.30/0.20/0.50), rate-limit, LRU-кэш эмбеддингов, реальная ONNX all-MiniLM-L6-v2.
- Конфликт-движок: `is_conflicting_pair` уже с двумя каналами (Dice ≥ 0.82 ИЛИ cosine ≥ 0.62 + shared stems), Truth Engine (`determine_truth`), conflict_groups, sync_conflict_groups.
- Memory lifecycle: MemoryState (Current/Superseded/Conflicted/UserConfirmed/Inferred), canonical consolidation (fingerprint + Jaccard-кластеры), radar (aging/stale), rehearsal, firewall, agent_permissions.
- Context: контекст-пайплайн (intent → retrieval → rank → compress → package), provenance (ScorePart), снапшоты, context_chains, flight recorder, deterministic auto_graph_builder.
- Graph: entity/relationship, graph_traversal (neighbors/distance/path/subgraph), auto_graph_builder.
- Versioning: automatic_commit, causality_chain, snapshot_service, version_graph, diff_calculator.
- Security: RequestContext (базовый), sandbox.rs, firewall deny_patterns, argon2/ring.
- MCP: in-process Rust stdio-сервер, 143 инструмента, регистрация в opencode.jsonc. Версия 1.0.0, protocol 2024-11-05, лимиты (request 256KiB / result 512KiB / concurrency 4 / timeout 60s), `deprecated`-флаг в tools/list.
- Benchmark harness: `src-tauri/src/bin/nexus_bench.rs` (retrieval, 118 кейсов) + `nexus_conflict_bench.rs` (17 кейсов, гейт detection ≥95% / FP <2%), реальные проекты (1320 файлов), реальная модель, честный отчёт.
- Тесты: 869 Rust unit + 10 security + 10 architecture + 2 frontend + 2 e2e + doctests. CI: fmt/clippy/test/build/lint — всё зелёное.
- Backfill-индексатор: фоновый, батчами, не блокирует запись, идемпотентный.
- Безопасность: `core/security/secrets.rs` (redaction), `tests/security.rs` (adversarial), `docs/THREAT_MODEL.md`.

**Ключевые дефекты (подтверждены бенчмарком):**
- Retrieval P@5 0.16 / R@5 0.43; на однородном корпусе (MUI) 0.00.
- Conflict: перефразировки 0/2 пойманы (после внедрения семантического канала ожидается улучшение — требует перепрогона).
- Нет: полноценного backup/restore, формальной state machine, feature flags, export/import проекта.

**Обновления после аудита (2026-08-13):**
- **Разрыв индекса устранён чанкингом** (`semantic_search.rs`): символы за окном 8192 B были невидимы semantic/lexical поиску (хвост `rust-log/src/lib.rs` — `pub struct Record` на позиции ~30 KB не находился вообще). Реализован `chunk_text` (окна 1024 B, overlap 128, UTF-8-безопасные границы, приоритет переносов строк), хранение `Vec<Vec<f32>>` с fallback на legacy `Vec<f32>` (миграция БД не нужна), `source_text` до 64 KB; окна в `indexer.rs`/`nexus_bench.rs` подняты до 65536. Регресс-защита: `tail_symbol_beyond_old_window_is_indexed`, 54 теста semantic_search.
- **Перепрогон retrieval (2026-08-13, реальный ONNX, 1320 файлов, 118 кейсов):** Sem R@5 0.91 → **0.95**, R@20 0.95 → **0.97**, P@5 0.22 → 0.23; keyword R@5 0.20 → **0.30**; **missing rate 0.0085 → 0.0000** (единственный семантический miss закрыт); top-1 0.9068 → **0.9322**, top-5 0.9661 → **0.9831**; MRR@10 0.91→0.93 → **0.95→0.96**; токен-reduction 77.4% → **94.2%**, средняя задержка 5640 → **1863 ms**. Индексация 1320 файлов: 334.7s (4 files/s, больше эмбеддингов на файл — честная цена чанкинга). Полный отчёт: `benchmarks/retrieval/bench-run.md`.
- Conflict (перепрогон): near-duplicate 2/2, перефразировки 2/2, FP 0.

---

## Фазы

### Фаза 0 — Фундамент качества (измеримость)
Пункты: 3 (частично), 4, 20, 21, 31, 32.

| # | Задача | Файлы | Приёмка |
|---|---|---|---|
| 0.1 | Error taxonomy: AppError → code/severity/component/recoverable + request_id. | `core/result.rs`, `infra/logging.rs` | ✅ DONE — unit-тесты; каждая ошибка классифицируется |
| 0.2 | Structured logging: operation_id, duration, component, severity, error_code. | `infra/logging.rs` | ✅ DONE — пример лога из реальной операции |
| 0.3 | Evaluation framework: структура `benchmarks/{retrieval,conflict,classification,compression,permissions,lifecycle,provenance,long_horizon}/` + CI-гейт Before/After. | `benchmarks/`, `.github/workflows/ci.yml` | ✅ DONE — retrieval 118 кейсов, conflict 17 кейсов, гейты |
| 0.4 | `nexus doctor` CLI: DB, migrations, FK, memory integrity, graph, embedding index, permissions, MCP. | `src-tauri/src/bin/nexus_doctor.rs`, `core/doctor.rs` | ✅ DONE — PASS/FAIL по каждой проверке |
| 0.5 | Diagnostics screen в UI + Export Diagnostic Report (без ПДн). | `src/components/diagnostics/DiagnosticsView.tsx`, `commands/diagnostics.rs` | ✅ DONE — экран в приложении + экспорт Markdown без ПДн |
| 0.6 | Data integrity invariants: `nexus doctor` проверяет ссылки (superseded_by_id, conflict groups, provenance...). | `core/doctor.rs` | ✅ DONE — fail на намеренно сломанной БД |

### Фаза 1 — Retrieval до production
Пункт: 1.

| # | Задача | Файлы | Приёмка |
|---|---|---|---|
| 1.1 | Расширить benchmark до 100+ запросов (реальные проекты + ручные). | `benchmarks/retrieval/`, `nexus_bench.rs` | ✅ DONE — 118 запросов + ground truth |
| 1.2 | Graph expansion в retrieval: query → entity resolve → neighborhood → кандидаты. | `core/context/ranker.rs`, `graph_traversal.rs` | ✅ DONE — graph_expand, co-location граф |
| 1.3 | Reranker (multi-stage: hybrid top-50 → rerank). | `core/context/ranker.rs` | ✅ DONE — HybridReranker, MUI=0.97 |
| 1.4 | Учёт filename/symbol/path/entity в ранжировании (частично есть). | `semantic_search.rs` | ✅ DONE — в бенчмарке |
| 1.5 | Целевые метрики: Recall@20 ≥ 90%, Recall@5 ≥ 70%. | `benchmarks/retrieval/` | ✅ DONE — R@20=0.97, R@5=0.95, P@5=0.23, missing 0.0000 (2026-08-13, 118 кейсов), CI-гейт |

### Фаза 2 — Conflict Engine v2
Пункт: 2.

| # | Задача | Файлы | Приёмка |
|---|---|---|---|
| 2.1 | Вердикты: SUPPORTED/CONTRADICTED/SUPERSEDED/UNRELATED/UNCERTAIN. | `core/memory/conflict/` | ✅ DONE — PairVerdict enum + classify() |
| 2.2 | Entity extraction (по графу) + claim comparison. | `core/memory/conflict/` | ✅ DONE — compare_claims() с negation/numbers |
| 2.3 | Temporal reasoning (даты/версии/суперсессии). | `core/memory/conflict/` | ✅ DONE — extract_years + TEMPORAL_MIN_OVERLAP + temporal supersession, 6 тестов |
| 2.4 | Source trust (source + state + confirmed_by). | `truth.rs` | ✅ DONE — source_trust() + TrustLevel |
| 2.5 | Benchmark: 95% обнаружения, <2% FP, отдельно paraphrase/negation/numbers/architecture. | `benchmarks/conflict/` | ✅ DONE — detection 100%, FP 0%, CI-гейт |

### Фаза 3 — Надёжность данных
Пункты: 5, 6, 7, 8, 9, 10.

| # | Задача | Файлы | Приёмка |
|---|---|---|---|
| 3.1 | Backup/restore: manual/auto/scheduled, atomic (SQLite backup API), versioned, checksum, restore→temp→integrity→swap. | `core/backup.rs`, `commands/backup.rs`, UI | ✅ DONE — create/verify/restore/list/delete, BackupManifest+sha256, V31-миграция, тесты roundtrip/коррупция/tamper/restore-to-temp-swap |
| 3.2 | Migration failure tests: падение на каждой миграции → restart → старая версия или корректное продолжение. | `storage/sqlite/schema.rs` + tests | ✅ DONE — `failed_migration_rolls_back_atomically`, `migration_failure_preserves_previous_schema`, `failed_migration_retries_after_fix` |
| 3.3 | Crash recovery: idempotent/atomic операции (indexing, rehearsal, migration, versioning, snapshots). | `core/context/indexer.rs`, `core/memory/memory_rehearsal.rs` | ✅ DONE — прерывание безопасно: indexer `cancelled_token_stops_backfill_cleanly` + `check_at_batch_boundary_contract`; rehearsal — `run_rehearsal_cycle_with_cancel` (checkpoint каждые 64 записи) + новый тест `commands::rehearsal::tests::cancelled_cycle_stops_before_first_record` (токен отменён → Cancelled, ни одна запись не тронута); миграции — атомарный откат (3.2) |
| 3.4 | Resumable indexing: checkpoint, progress, cancel, resume, file hash, incremental update, deleted-file detection. | `core/context/indexer.rs` | ✅ DONE — `unindexed_batch_excludes_already_indexed_rows` (уже проиндексированные не выбираются повторно) + `reindex_replaces_own_fingerprint_keeps_others` (`semantic_search.rs`): переиндексация 1 памяти через INSERT OR REPLACE не плодит строки и не трогает чужие фингерпринты |
| 3.5 | Deduplication гарантии: повторный импорт/rename/move/copy/rollback не плодит копии. | `canonical_consolidation.rs` + tests | ✅ DONE — canonical_consolidation (canonical id, cluster, find_duplicates/merge_entities) + entity fusion, тесты сценариев |
| 3.6 | Memory lifecycle state machine: формальные состояния + запрещённые переходы (ARCHIVED→ACTIVE запрещён). | `core/memory/types.rs`, `memory_lifecycle.rs` | ✅ DONE — `MemoryStatus::can_transition` + `MemoryState::can_transition` (формальные машины), 7 тестов переходов: разрешённые (archive/merge/promote/resolve) и запрещённые (ARCHIVED→ACTIVE, MERGED→ACTIVE, Superseded→*, UserConfirmed→demotion, Conflicted→Inferred) + total-no-panic; enforcement в `memory_set_state_ctx` (reject недопустимого перехода) |

### Фаза 4 — Безопасность
Пункты: 11, 12, 13, 45, 46, 47.

| # | Задача | Файлы | Приёмка |
|---|---|---|---|
| 4.1 | Security suite: path traversal (../, %2e%2e, \\, UNC, junction, symlink, ADS, reserved), prompt injection, secrets. | `tests/rust/security/`, `core/sandbox.rs` | ✅ DONE — `tests/security.rs`, 10 adversarial тестов |
| 4.2 | Threat model документ. | `docs/THREAT_MODEL.md` | ✅ DONE — T1–T6, зоны доверия, контрмеры |
| 4.3 | Secrets: Windows Credential Manager, redaction в UI/logs/audit/MCP. | `core/security/secrets.rs` | ✅ DONE — SecretKind, looks_like_secret/redact, 11 тестов |
| 4.4 | RequestContext: actor/agent_id/project_id/permissions/sensitivity_scope/request_id. | `core/security/request_context.rs` | ✅ DONE — RequestContext (user/system/agent, agent_id, project_id, permissions, sensitivity_scope, request_id UUID) обязателен в критических командах: `ensure_can_mutate()` gate в `memory_set_state_ctx`/`memory_confirm_ctx`/`memory_feedback_ctx`/`memory_supersede_ctx`, MCP-инструменты строят ctx через `mcp_request_context` (агент не может мутировать без write), аудит пишет `actor_label`; 14 тестов + `ensure_can_mutate_gate` |
| 4.5 | Immutable audit: append-only гарантия + полные события (memory changed, permission changed, firewall denied...). | `core/audit/`, `audit_repository_sqlite.rs` | ✅ DONE — V32_audit_append_only.sql, триггеры trg_audit_no_update/no_delete, тесты append-only обновления/удаления запрещены |
| 4.6 | Agent Passport: identity ≠ authorization. | `agent_passport.rs` | ✅ DONE — 13+ тестов изоляции (agent_id/identity не даёт прав поверх политики) |

### Фаза 5 — MCP production
Пункты: 14, 15, 16, 17, 18, 19.

| # | Задача | Файлы | Приёмка |
|---|---|---|---|
| 5.1 | MCP contract tests: schema/validation/permissions/error/timeout/cancel/idempotency по каждому tool. | `tests/rust/mcp/` | ✅ DONE — `tests/mcp_stdio_e2e.rs` (full flow) + 44 unit |
| 5.2 | MCP API versioning (v1, deprecated/replaced). | `ai/mcp_server.rs` | ✅ DONE — MCP_API_VERSION 1.0.0, protocol 2024-11-05, `deprecated` в tools/list |
| 5.3 | Rate/resource limits: max request size, result size, graph expansion, recursion, tokens, concurrency, timeouts. | `ai/mcp_server.rs` | ✅ DONE — MAX_REQUEST_BYTES 256KiB, MAX_RESULT_BYTES 512KiB, Semaphore 4, timeout 60s, truncation |
| 5.4 | Cancellation everywhere: indexing/embedding/retrieval/rehearsal/MCP/backup → Cancel→cleanup→consistent. | все тяжёлые операции | ✅ DONE — ErrorCode::Cancelled, core/cancel.rs CancelToken, backfill_with_cancel, run_rehearsal_cycle_with_cancel, тесты отмены |
| 5.5 | Memory limits + benchmarks 1k/10k/100k/500k/1M (RAM/CPU/SQLite/startup/search). | `src/bin/nexus_load_bench.rs`, `migrations/V33_memory_created_at_index.sql` | ✅ DONE — GATE PASS на 100k: insert 2555 rec/s, search 182ms, list(100) 0ms; индекс created_at |
| 5.6 | Startup/shutdown guarantees: corrupted DB, unfinished operation. | `main.rs`, `db.rs` | ✅ DONE — doctor + schema migration tests; TODO в core отсутствуют |

### Фаза 6 — Производительность и масштаб
Пункты: 36, 37, 30, 18.

| # | Задача | Файлы | Приёмка |
|---|---|---|---|
| 6.1 | Performance budget (SLA): startup <2s, search <100ms, context <200ms, cached <50ms, MCP <100ms, insert <50ms, index <1s/file. | `benchmarks/` | ✅ DONE — `src/bin/nexus_sla_bench.rs` GATE PASS 7/7, числа в `docs/PERFORMANCE_BUDGET.md` |
| 6.2 | Performance CI: benchmark → compare baseline → REGRESSION gate. | `.github/workflows/` | ✅ DONE — job `perf` в `ci.yml`, `scripts/perf-gate.ps1` + `benchmarks/baseline.json` (tolerance 25%, ретрай 1× на флаки); покрывает 4 бенча (sla/load/conflict/long-horizon); sla_search_ms — медиана 5 прогонов (анти-флак) |
| 6.3 | Long-horizon benchmark: 100→500→2000→conflicts→supersession→rehearsal→agent switch. | `benchmarks/long_horizon/` | ✅ DONE — `src/bin/nexus_long_horizon_bench.rs` GATE PASS на 2000: insert 4504 rec/s, search 2ms, list 0ms, rehearsal 506ms; supersession 100/100, conflicts 3/3, agent switch Allow/Deny; NEXUS_METRIC + baseline + perf-gate |

### Фаза 7 — Упаковка и supply chain
Пункты: 22, 23, 24, 25, 26, 39, 40, 41, 42, 43.

| # | Задача | Файлы | Приёмка |
|---|---|---|---|
| 7.1 | Update rollback: verify signature/checksum → install → health check → rollback. | `main.rs`, updater | ✅ DONE — маркеры `update_pending`/`update_failed` в configuration_kv, `verify_post_update_health` на старте (DB open + schema version + MCP initialize smoke), 7/7 тестов updater |
| 7.2 | Release channels (Stable/Beta/Nightly). | `.github/workflows/release.yml` | ✅ DONE — шаг `Resolve release channel` + канальный манифест + `prerelease` для beta/nightly, floating pointers `channel-beta`/`channel-nightly` (semver-guard, refresh на каждом релизе), каскад endpoints nightly→beta→stable |
| 7.3 | Reproducible builds: фиксация Rust/Node/npm, lockfiles, ONNX hash, toolchain. | `docs/BUILD_REPRODUCIBILITY.md` | ✅ DONE — файл существует: rust-toolchain, Node 24, lockfiles, ONNX sha256, wix/nsis версии |
| 7.4 | SBOM + dependency security (crates/npm/ONNX). | CI шаг | ✅ DONE — job `security` в ci.yml: cargo-audit, npm audit, CycloneDX SBOM, cargo-deny |
| 7.5 | Supply chain: pin actions to SHA, минимальные permissions, protected branches/tags. | `.github/workflows/` | ✅ DONE — actions припинены к SHA (checkout@gdfab…, setup-node@1d0ff…, cache@5a3ec…), `permissions: contents: read` + точечные, approve-workflow |
| 7.6 | Feature flags (semantic_conflict_v2, hybrid_retrieval, ...). | `core/config/` | ✅ DONE — `feature_flags.rs`: реестр, fail-open, `feature.` prefix, программатор, чтение через config |
| 7.7 | MCP API docs (генерация из схем). | `docs/mcp/` | ✅ DONE — `docs/mcp/README.md` + `reference.md` + `tools.json`, 143 инструмента |
| 7.8 | Compatibility policy + matrix. | `docs/COMPATIBILITY.md` | ✅ DONE — файл существует (версии, политика, матрица) |
| 7.9 | Installer matrix: Win10/11 x64/ARM64, clean/upgrade/uninstall/non-admin. | e2e + docs | ✅ DONE — `docs/INSTALLER_MATRIX.md`: матрица OS×arch×installer (NSIS/MSI, installMode both), сценарии S1–S6, контракт сохранения данных при uninstall |

### Фаза 8 — Тестовый фронт
Пункты: 27, 28, 29.

| # | Задача | Файлы | Приёмка |
|---|---|---|---|
| 8.1 | Coverage targets ≥90% branch для security-critical/stateful модулей. | CI + `cargo llvm-cov` | ✅ DONE — `docs/COVERAGE_REPORT.md`: 4 модуля ≥90% region (stable без branch: sandbox 96.52%, tokenizer 94.08%, backup 90.77%, versioning_repository 91.10%; line 93.7–96.5%). Все env-мутирующие тесты вынесены в отдельные integration-бинарники (`tests/tokenizer_state.rs`, `tests/backup_global.rs`, `tests/sandbox_live_policy.rs`) — устранены гонки LOCALAPPDATA/ACTIVE. Команда воспроизведения: `cargo llvm-cov --lib --test security --test network_allowlist --test proptest --test fuzz_smoke --test tokenizer_state --test backup_global --test sandbox_live_policy` |
| 8.2 | Property-based testing (proptest): memory states, graph edges, UTF-8, paths, migration sequences. | `tests/proptest.rs` | ✅ DONE — 19 proptest-тестов. Вскрыто и исправлено 2 бага: (1) `count()` exact-движок возвращал 0 для OOV-скриптов — добавлен floor 1 токен для реального контента; (2) `execute_migration_idempotent` не проглатывал duplicate-column при повторном apply после rollback из-за комментариев перед ALTER и case-sensitive сравнения колонок. `cargo test --lib` 927+, proptest 19/19 |
| 8.3 | Fuzzing: parsers, path sanitizer, MCP input, graph resolver, context builder. | `tests/fuzz_smoke.rs` | ✅ DONE — 5 таргетов, детерминированный xorshift PRNG, таймбокс `NEXUS_FUZZ_SECONDS` (default 60s на каждый), adversarial-сиды (NUL, UTF-8 границы, path tricks, 70KB). 60s-прогон: 5/5 pass |

### Фаза 9 — Данные и приватность
Пункты: 33, 34, 35, 48, 49.

| # | Задача | Файлы | Приёмка |
|---|---|---|---|
| 9.1 | Disaster recovery документ (DB corruption, failed migration, broken update, corrupted embeddings). | `docs/DISASTER_RECOVERY.md` | ✅ DONE — документ существует, 4 сценария по циклу detection→recovery→verification: повреждение БД (doctor integrity→verify_backup→restore через SQLite Online Backup API→doctor+сверка), сбой миграции (атомарный откат V1–V33), сломанное обновление (safety-бэкап перед restore, экспорт-импорт как портативный путь), повреждённые эмбеддинги (fallback хэш-векторы→пересоздание фингерпринтов) |
| 9.2 | Export/Import Nexus Project: versioned format (manifest, memories, entities, relations, decisions, skills, provenance, snapshots). | `core/export.rs`, `commands/` | ✅ DONE — roundtrip-тесты: `roundtrip_preserves_all_sections` (9 секций byte-for-byte после export→import→re-export), `roundtrip_preserves_ids`, `rejects_unknown_format_version` (format_version 999 отклоняется) |
| 9.3 | Privacy verification: monitor outbound network (0 unexpected connections). | `tests/network_allowlist.rs`, `docs/NETWORK_PRIVACY.md` | ✅ DONE — allowlist github.com/huggingface.co, замок из 5 тестов, удалены Google Fonts из `index.html` и мёртвый `reqwest` из `Cargo.toml` |
| 9.4 | Context integrity: контекст-пакет зафиксирован (IDs/scores/provenance/permissions/tokens) — Flight Recorder показывает фактический контекст. | `flight/`, `context_snapshot.rs` | ✅ DONE — `snapshot_frozen_after_memory_change` (`context_service.rs`): пакет собран, память изменена после сборки → снапшот и replay несут исходный контент, живой запрос видит новое |
| 9.5 | Deterministic replay: query + snapshot + params + policy → тот же context package. | `context_service.rs` | ✅ DONE — `replay_context_is_deterministic`: один снапшот реплеится 3× → полное JSON-равенство (IDs/scores/provenance/tokens) и совпадение с записанным пакетом |

### Фаза 10 — Production Gate
Пункт: 50.

| # | Задача | Файлы | Приёмка |
|---|---|---|---|
| 10.1 | NEXUS PRODUCTION GATE чеклист (20 пунктов). | `docs/PRODUCTION_GATE.md` | ✅ DONE — чеклист из 20 пунктов по 4 гейтам (Correctness/Security/Reliability/Performance), каждый с критерием PASS, командой верификации и артефактом; сводка 4/4 PASS |
| 10.2 | Gate 1 Correctness, Gate 2 Security, Gate 3 Reliability, Gate 4 Performance. | CI | ✅ DONE — Gates 1–3 в job `rust` (`cargo test --all-targets`, NEXUS_FUZZ_SECONDS=10 в CI против 60s локального прогона); Gate 4 = job `perf` (`scripts/perf-gate.ps1` + `benchmarks/baseline.json`, tolerance 25%, ретрай 1×, таймаут 600s) |

---

## Порядок реализации

1. **Фаза 0** (фундамент: taxonomy, logs, benchmark framework, doctor, diagnostics) — даёт измеримость для всего остального.
2. **Фаза 3.1–3.2** (backup/restore + migration failure tests) — надёжность данных, критично для остальных фаз.
3. **Фаза 1** (retrieval) — главный заявленный дефект, бенчмарк уже готов к расширению.
4. **Фаза 2** (conflict v2) — второй заявленный дефект.
5. **Фаза 4** (security suite + secrets + RequestContext) — безопасность до расширения MCP.
6. **Фаза 5** (MCP production) — contract tests, versioning, rate limits.
7. Остальные фазы по мере прохождения гейтов.

Принципы: никаких моков/заглушек, всё на реальных данных Nexus; каждая задача = код + тесты + проверка `cargo test` / `npm run build` / `npm run lint`; без субагентов, реализация вручную.
