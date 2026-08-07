-- V12: Memory Trust lifecycle — актуальность, замена, подтверждение, обратная связь.
-- Колонки добавляются идемпотентно (ALTER TABLE ADD COLUMN обрабатывается schema.rs).
ALTER TABLE memory_records ADD COLUMN memory_state TEXT NOT NULL DEFAULT 'Current';
ALTER TABLE memory_records ADD COLUMN supersedes_id TEXT;
ALTER TABLE memory_records ADD COLUMN superseded_by_id TEXT;
ALTER TABLE memory_records ADD COLUMN confirmed_at TEXT;
ALTER TABLE memory_records ADD COLUMN confirmed_by TEXT;
ALTER TABLE memory_records ADD COLUMN expires_at TEXT;
ALTER TABLE memory_records ADD COLUMN feedback_json TEXT NOT NULL DEFAULT '{"useful":0,"irrelevant":0,"wrong":0}';

CREATE INDEX IF NOT EXISTS idx_memory_state ON memory_records(memory_state);
CREATE INDEX IF NOT EXISTS idx_memory_supersedes ON memory_records(supersedes_id);
