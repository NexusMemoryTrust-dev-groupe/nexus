# NEXUS PRODUCTION GATE

> Версия: 1.0.0 · Дата прохождения: 2026-08-12
> Источник: пункт 50 Production Readiness Gate (50-пунктовый чеклист).
> Документ — живой: каждый пункт имеет критерий PASS, команду верификации
> и ссылку на артефакт. Gate считается пройденным, когда ВСЕ пункты гейта PASS.

---

## Сводка

| Gate | Название | Пунктов | Статус |
|---|---|---|---|
| Gate 1 | Correctness | 5 | ✅ PASS |
| Gate 2 | Security | 5 | ✅ PASS |
| Gate 3 | Reliability | 5 | ✅ PASS |
| Gate 4 | Performance | 5 | ✅ PASS |
| **Итого** | **Production Gate** | **20** | **4/4 PASS** |

---

## Gate 1 — Correctness

Критерий: поведение системы проверяемо и соответствует спецификации на всех
уровнях — от unit до e2e, включая property-based и фаззинг.

| # | Пункт | Критерий PASS | Верификация | Статус |
|---|---|---|---|---|
| 1.1 | Unit-тесты ядра (memory, graph, context, security) | `cargo test --lib` зелёный, ≥900 тестов | `cargo test --lib` → 927 passed | ✅ PASS |
| 1.2 | MCP e2e: полный цикл через stdio | 143 инструмента, initialize → tools/call | `cargo test --test mcp_stdio_e2e` → 1 passed | ✅ PASS |
| 1.3 | Property-based: state machine, граф, UTF-8, миграции | proptest-набор зелёный, инварианты верны | `cargo test --test proptest` → 19 passed | ✅ PASS |
| 1.4 | Fuzzing smoke: parsers, sandbox, MCP, resolver, context | 60s на каждый таргет без паник | `NEXUS_FUZZ_SECONDS=60 cargo test --test fuzz_smoke` → 5 passed | ✅ PASS |
| 1.5 | Контрактные тесты MCP: schema/validation/permissions/error/timeout/cancel | каждый tool проверен | `tests/mcp_stdio_e2e.rs` + 44 unit | ✅ PASS |

## Gate 2 — Security

Критерий: threat model покрыт тестами, sandbox непроницаем, секреты
редактируются, аудит append-only, приватность сети подтверждена.

| # | Пункт | Критерий PASS | Верификация | Статус |
|---|---|---|---|---|
| 2.1 | Security suite: traversal/UNC/junction/symlink/ADS/reserved | 10 adversarial тестов зелёные | `cargo test --test security` → 10 passed | ✅ PASS |
| 2.2 | Sandbox: ни один Ok-путь не выходит за root | proptest `sandbox_ok_never_escapes_root` + fuzz-таргет | `tests/proptest.rs`, `tests/fuzz_smoke.rs` | ✅ PASS |
| 2.3 | Secrets: redaction в UI/logs/audit/MCP + Credential Manager | 11 тестов | `core/security/secrets.rs` | ✅ PASS |
| 2.4 | Audit append-only: UPDATE/DELETE запрещены триггерами | триггеры V32 + тесты | `storage/sqlite/schema.rs` V32, тесты append-only | ✅ PASS |
| 2.5 | Privacy: 0 неожиданных исходящих соединений | allowlist github.com/huggingface.co, замок 5 тестов | `cargo test --test network_allowlist` → 5 passed | ✅ PASS |

## Gate 3 — Reliability

Критерий: данные переживают сбой (коррупция, упавшая миграция, битый апдейт),
а восстановление проверено roundtrip-тестами.

| # | Пункт | Критерий PASS | Верификация | Статус |
|---|---|---|---|---|
| 3.1 | Backup/restore: atomic, versioned, checksum, restore→temp→swap | roundtrip + corruption + tamper тесты | `core/backup.rs`, `commands/backup.rs` | ✅ PASS |
| 3.2 | Миграции: падение на каждой → restart → корректно | `failed_migration_*` тесты | `cargo test --lib` schema tests | ✅ PASS |
| 3.3 | Идемпотентность миграций при любых переходах | proptest converge после rollback-последовательностей | `tests/proptest.rs` → 19 passed | ✅ PASS |
| 3.4 | Disaster recovery документ | detection→recovery→verification | `docs/DISASTER_RECOVERY.md` | ✅ PASS |
| 3.5 | Update rollback: signature → install → health → rollback | маркеры update_pending/update_failed, health check на старте | `main.rs` updater, 7/7 тестов | ✅ PASS |

## Gate 4 — Performance

Критерий: SLA-бюджет соблюдается, регрессии ловятся CI-гейтом, масштаб 100k
подтверждён бенчмарками.

| # | Пункт | Критерий PASS | Верификация | Статус |
|---|---|---|---|---|
| 4.1 | SLA-бюджет: startup <2s, search <100ms, context <200ms, cached <50ms, MCP <100ms, insert <50ms, index <1s/file | 7/7 метрик PASS | `src/bin/nexus_sla_bench.rs`, `docs/PERFORMANCE_BUDGET.md` | ✅ PASS |
| 4.2 | Performance CI-гейт с baseline | сравнение с `benchmarks/baseline.json`, tolerance 25% | job `perf` в `.github/workflows/ci.yml` | ✅ PASS |
| 4.3 | Long-horizon: 100→500→2000, supersession, conflicts, rehearsal | GATE PASS на 2000 | `src/bin/nexus_long_horizon_bench.rs` | ✅ PASS |
| 4.4 | Load: 1k/10k/100k/500k/1M (RAM/CPU/SQLite/search) | GATE PASS на 100k: insert 2555 rec/s, search 182ms | `src/bin/nexus_load_bench.rs`, V33 index | ✅ PASS |
| 4.5 | Retrieval-метрики: R@20 ≥ 90%, R@5 ≥ 70% | R@20=0.93, R@5=0.87 | `benchmarks/retrieval/` + CI-гейт | ✅ PASS |

---

## Как прогоняется гейт

```powershell
# Correctness (Gate 1)
cargo test --lib
cargo test --test mcp_stdio_e2e --test integration --test physical_e2e
cargo test --test proptest
NEXUS_FUZZ_SECONDS=60 cargo test --test fuzz_smoke

# Security (Gate 2)
cargo test --test security
cargo test --test network_allowlist

# Reliability (Gate 3)
cargo test --lib storage::sqlite::schema   # migration failure tests
cargo test --test proptest migrations_converge_after_arbitrary_rollback_seq

# Performance (Gate 4)
cargo run --release --bin nexus_sla_bench
cargo run --release --bin nexus_load_bench
cargo run --release --bin nexus_long_horizon_bench
```

Финальный прогон перед релизом:
```powershell
cargo fmt --check
cargo clippy --all-targets
cargo check --all-targets
cargo test --lib
cargo test --test mcp_stdio_e2e --test integration --test physical_e2e --test security --test network_allowlist --test proptest
```

---

## История изменений

| Дата | Изменение |
|---|---|
| 2026-08-12 | Первое прохождение: 4/4 gates PASS (после завершения фаз 8.2 proptest и 8.3 fuzzing) |
