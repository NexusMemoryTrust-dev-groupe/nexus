-- V22: Flight Recorder — бортовой самописец операций (Система 5).
-- Записывает каждый значимый шаг экосистемы: кто, что, когда и с каким
-- результатом. Расширяет event_bus и audit: всё, что происходит в системе
-- (создание памяти, конфликт, карантин, rehearsal, скилл, MCP-вызов),
-- оседает в журнале полёта и может быть воспроизведено по цепочке.
--
-- Две таблицы:
--   flight_sessions  — сессия полёта: период активности (один проход агента,
--                      одна задача). Сессии могут быть открыты/закрыты явно.
--   flight_records   — сами записи: атомарные шаги с результатом.
CREATE TABLE IF NOT EXISTS flight_sessions (
    id         TEXT PRIMARY KEY,
    title      TEXT NOT NULL DEFAULT '',      -- человекочитаемое имя сессии
    purpose    TEXT NOT NULL DEFAULT '',      -- зачем эта сессия
    actor      TEXT NOT NULL DEFAULT 'system',-- кто инициатор (user|agent|mcp|system)
    source     TEXT NOT NULL DEFAULT '',      -- откуда (copilot|mcp|cli|ui|auto)
    status     TEXT NOT NULL DEFAULT 'active',-- active | closed
    started_at TEXT NOT NULL,
    ended_at   TEXT
);

CREATE INDEX IF NOT EXISTS idx_flight_sessions_status ON flight_sessions(status);
CREATE INDEX IF NOT EXISTS idx_flight_sessions_started ON flight_sessions(started_at);

CREATE TABLE IF NOT EXISTS flight_records (
    id           TEXT PRIMARY KEY,
    session_id   TEXT,                        -- NULL = вне сессии
    recorded_at  TEXT NOT NULL,
    actor        TEXT NOT NULL DEFAULT 'system',
    category     TEXT NOT NULL,               -- memory|conflict|firewall|rehearsal|radar|skill|context|team|versioning|mcp|system
    action       TEXT NOT NULL,               -- create_memory|resolve_conflict|quarantine|run_cycle|call_tool|...
    entity_type  TEXT NOT NULL DEFAULT '',
    entity_id    TEXT NOT NULL DEFAULT '',
    summary      TEXT NOT NULL DEFAULT '',    -- одна строка человекочитаемого описания
    details_json TEXT NOT NULL DEFAULT '{}',  -- JSON с деталями (параметры, причиной)
    duration_ms  INTEGER NOT NULL DEFAULT 0,
    outcome      TEXT NOT NULL DEFAULT 'success' -- success | error | blocked | skipped
);

CREATE INDEX IF NOT EXISTS idx_flight_records_session ON flight_records(session_id);
CREATE INDEX IF NOT EXISTS idx_flight_records_entity ON flight_records(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_flight_records_recorded ON flight_records(recorded_at);
CREATE INDEX IF NOT EXISTS idx_flight_records_category ON flight_records(category);
