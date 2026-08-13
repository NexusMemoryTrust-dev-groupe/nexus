# Network Privacy — Nexus

Статус: актуально для v1.1.0. Пункт 9.3 Production Readiness Plan.

Цель: **0 неожиданных исходящих подключений**. Nexus — локальное приложение
(«всё локально, никакой телеметрии», см. README). Этот документ фиксирует
полный перечень хостов, к которым приложение может обращаться само, и
автоматический замок, который не даёт этому списку тихо расшириться.

---

## 1. Allowlist исходящих подключений

Единственные хосты, до которых Nexus дозванивается **автоматически**:

| Хост | Что идёт | Когда | Механизм |
|---|---|---|---|
| `github.com` | манифест обновления + установщик (`latest.json`, `channel-beta/beta.json`, `channel-nightly/nightly.json`) | ~5 с после старта, фон (план 7.2, `infra/updater.rs`) | `tauri-plugin-updater` (rustls, TLS 1.2+) |
| `huggingface.co` | ONNX-модель `AllMiniLML6V2` (~90 МБ) | первое использование семантического поиска, кэш в `.fastembed_cache` | `fastembed` |

Всё остальное — локально: SQLite (`%LOCALAPPDATA%\Nexus\nexus.db`), WAL,
бэкапы, файловая песочница, MCP через stdio. Никакой телеметрии, аналитики,
трекеров.

**User-initiated (не считаются «неожиданными»):**
- вызовы внешних LLM — инициирует пользователь через копилот/OpenCode CLI
  (ключи хранятся в credential-store OpenCode, не в базе Nexus);
- статическая ссылка `nodejs.org` в мастерe настройки — открывается по клику
  пользователя, автоматических запросов нет.

---

## 2. Что проверяет автоматический тест

`src-tauri/tests/network_allowlist.rs` — детерминированный «замок» без сети и
внешних зависимостей. Пять тестов:

| Тест | Что сканирует | Отказ при |
|---|---|---|
| `updater_endpoints_only_github` | `infra/updater.rs` + `tauri.conf.json` | любой URL обновления не на `github.com` |
| `no_raw_network_primitives_in_rust_sources` | весь `src/**/*.rs` | `TcpStream`, `UdpSocket`, `TcpListener`, `reqwest::`, `ureq::`, `hyper::`, `ws://`, `wss://`, `WebSocket::connect` |
| `no_remote_resources_in_index_html` | `index.html` | любой `https://` в html (CDN-шрифты, скрипты) |
| `no_programmatic_network_in_frontend` | `src` фронтенда (`ts/tsx/js/jsx/html`) | `fetch(`, `axios`, `new WebSocket(`, `XMLHttpRequest` |
| `no_direct_http_client_dependencies` | секция `[dependencies]` в `Cargo.toml` | `reqwest` / `ureq` / `hyper` как прямая зависимость |

Запуск: `cargo test --test network_allowlist`.

### Как читать исключения по дизайну

- `opencode.ai` / `example.com` появляются в `src/core/mcp_register.rs` как
  встроенные строки JSON-schema `$schema` — это метаданные, которые Nexus
  записывает в *генерируемые документы*, а не запросы, которые он шлёт.
  Тесты их не ловят, потому что не ловят URL-литералы вообще — только
  примитивы соединений и фактически настраиваемые endpoint'ы.

---

## 3. Изменения, внесённые для выполнения мандата (v1.1.0 → 9.3)

| Файл | Было | Стало |
|---|---|---|
| `index.html` | `<link>` на `fonts.googleapis.com` + `fonts.gstatic.com` | удалены — webview не ходит в CDN |
| `src/styles/globals.css` | фоллбеки `-apple-system, sans-serif` | Windows-стек: `"Segoe UI Variable", "Segoe UI"`, mono → `"Cascadia Code", Consolas` |
| `src-tauri/Cargo.toml` | прямая зависимость `reqwest = "0.12"` (HTTP-клиент, 0 использований) | удалена — attack surface без неиспользуемого клиента |
| `src-tauri/tests/network_allowlist.rs` | — | новый e2e-замок (5 тестов) |
| `docs/NETWORK_PRIVACY.md` | — | этот документ |

---

## 4. Руководство по изменению (runbook)

**Я хочу добавить новый исходящий вызов.** Порядок:

1. Сначала напиши, зачем: нового хоста нет в allowlist раздела 1 — вызов
   **нельзя** добавлять по умолчанию.
2. Внеси изменение + обнови `docs/NETWORK_PRIVACY.md` (таблица раздела 1,
   при необходимости раздел 3).
3. Прогони `cargo test --test network_allowlist` — если тест красный, замок
   сработал как задумано: объясни, почему новый хост необходим, добавь его в
   allowlist и в документ, но **только** после ревью.
4. CI уже гоняет этот тест вместе с остальными `cargo test`.

**Я хочу убрать шрифт/CDN/зависимость.** Обратный порядок, тест должен
остаться зелёным после удаления.

**Проверка «приложение реально не ходит в сеть» вручную:**
1. Запусти приложение в офлайне (отключи сеть) — GUI, MCP-режим и семантика
   (с уже скачанным кэшем) работают.
2. `netstat -b` во время работы — из процессов `Nexus.exe` видны соединения
   только с `github.com` (updater), а после первого семантического поиска —
   ещё и `huggingface.co`.
3. Proxу-перехватчик (mitmproxy/proxifier) с политикой «allow только
   github.com + huggingface.co» — в логах не должно появиться ничего иного.

---

## 5. Связанные документы

- `docs/PRODUCTION_READINESS_PLAN.md` — пункт 9.3 (этот мандат).
- `docs/THREAT_MODEL.md` — модель угроз (сеть как вектор).
- `docs/DISASTER_RECOVERY.md` — пункт 9.1; `.fastembed_cache` и хэш-fallback.
- `README.md` — раздел «Приватность» (всё локально, нет телеметрии).