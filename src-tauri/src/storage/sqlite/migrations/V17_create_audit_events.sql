-- V17: Create audit_events table (Audit Memory — проверяемая память)
-- Append-only decision journal: every auditable action on a memory (created,
-- alternative considered, confirmed, superseded, note) gets one row so the
-- full chain "why did we decide this" can be reconstructed.
CREATE TABLE IF NOT EXISTS audit_events (
    id            TEXT PRIMARY KEY,
    memory_id     TEXT NOT NULL,
    event_type    TEXT NOT NULL,          -- Created | Alternative | Confirmed | Superseded | Note
    actor         TEXT NOT NULL DEFAULT '', -- who performed the action (member / user / system)
    detail        TEXT,                   -- free text; for Alternative events this is JSON {title, reason}
    related_memory_id TEXT,               -- for Superseded: the memory that replaced this one
    created_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ae_memory   ON audit_events(memory_id);
CREATE INDEX IF NOT EXISTS idx_ae_created  ON audit_events(created_at);
CREATE INDEX IF NOT EXISTS idx_ae_type     ON audit_events(event_type);
