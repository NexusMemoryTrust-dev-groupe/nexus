-- V19: Conflict groups — Memory Conflict Engine (Система 2)
-- Группа связывает записи, помеченные Conflicted детектором, в один конфликт:
-- участники, timeline (через записи), статус open/resolved и результат разрешения.
CREATE TABLE IF NOT EXISTS conflict_groups (
    id            TEXT PRIMARY KEY,
    topic         TEXT NOT NULL,               -- нормализованная тема для группировки
    member_ids    TEXT NOT NULL DEFAULT '[]',  -- JSON-массив id записей-участников
    detected_at   TEXT NOT NULL,
    resolved_at   TEXT,
    resolution    TEXT,                        -- JSON: {winner_id, confidence, reasons[], by, at}
    status        TEXT NOT NULL DEFAULT 'open' -- open | resolved
);

CREATE INDEX IF NOT EXISTS idx_cg_status ON conflict_groups(status);
CREATE INDEX IF NOT EXISTS idx_cg_topic  ON conflict_groups(topic);
