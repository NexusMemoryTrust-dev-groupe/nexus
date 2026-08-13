-- V21: Memory Firewall — защита хранилища от нежелательного входящего контента (Система 4).
-- Две таблицы:
--   firewall_rules      — пользовательские правила: подстрока/паттерн → действие block|quarantine.
--   quarantine_entries  — карантин: контент, который эвристики не пропустили, но жёстко
--                         блокировать не стали. Пользователь решает: approve (создать память)
--                         или reject (удалить навсегда).
CREATE TABLE IF NOT EXISTS firewall_rules (
    id         TEXT PRIMARY KEY,
    pattern    TEXT NOT NULL,                -- подстрока/паттерн для поиска в title+content
    action     TEXT NOT NULL,                -- block | quarantine
    enabled    INTEGER NOT NULL DEFAULT 1,   -- 1 = активно, 0 = отключено
    reason     TEXT NOT NULL DEFAULT '',     -- человекочитаемое объяснение правила
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_firewall_rules_enabled ON firewall_rules(enabled);

CREATE TABLE IF NOT EXISTS quarantine_entries (
    id           TEXT PRIMARY KEY,
    title        TEXT NOT NULL,
    content      TEXT NOT NULL,
    author       TEXT NOT NULL DEFAULT 'unknown',
    source       TEXT NOT NULL DEFAULT 'manual',
    reasons_json TEXT NOT NULL DEFAULT '[]',  -- JSON-массив причин карантина
    scores_json  TEXT NOT NULL DEFAULT '{}',  -- JSON: {toxicity, spam, injection, pii}
    status       TEXT NOT NULL DEFAULT 'pending', -- pending | approved | rejected
    created_at   TEXT NOT NULL,
    decided_at   TEXT
);

CREATE INDEX IF NOT EXISTS idx_quarantine_status ON quarantine_entries(status);
