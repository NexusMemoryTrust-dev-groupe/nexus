-- V5: Create entity_snapshots table
CREATE TABLE IF NOT EXISTS entity_snapshots (
    id            TEXT PRIMARY KEY,
    entity_type   TEXT NOT NULL,
    entity_id     TEXT NOT NULL,
    snapshot_data BLOB NOT NULL,
    size_bytes    INTEGER NOT NULL DEFAULT 0,
    is_baseline   INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_es_entity ON entity_snapshots(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_es_baseline ON entity_snapshots(entity_type, entity_id, is_baseline);
CREATE INDEX IF NOT EXISTS idx_es_created ON entity_snapshots(created_at);
