-- V7: Create context engine tables
CREATE TABLE IF NOT EXISTS context_snapshots (
    id TEXT PRIMARY KEY,
    entity_id TEXT NOT NULL,
    package_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    label TEXT
);

CREATE INDEX IF NOT EXISTS idx_cs_entity ON context_snapshots(entity_id);
CREATE INDEX IF NOT EXISTS idx_cs_created ON context_snapshots(created_at);
