-- V4: Create versioning tables (automatic_commits, causality_records, version_edges)
CREATE TABLE IF NOT EXISTS automatic_commits (
    id            TEXT PRIMARY KEY,
    hash          TEXT NOT NULL UNIQUE,
    version_number INTEGER NOT NULL,
    entity_type   TEXT NOT NULL,
    entity_id     TEXT NOT NULL,
    change_type   TEXT NOT NULL,
    diff_json     TEXT,
    baseline_snapshot_id TEXT,
    is_baseline   INTEGER DEFAULT 0,
    created_at    TEXT NOT NULL,
    created_by    TEXT NOT NULL DEFAULT 'system',
    triggering_event_type TEXT NOT NULL DEFAULT '',
    triggering_event_id   TEXT NOT NULL DEFAULT '',
    change_reason TEXT,
    linked_entity_ids_json   TEXT NOT NULL DEFAULT '[]',
    linked_decision_ids_json TEXT NOT NULL DEFAULT '[]',
    is_indexed     INTEGER DEFAULT 0,
    is_archived    INTEGER DEFAULT 0,
    size_bytes     INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_ac_entity  ON automatic_commits(entity_id);
CREATE INDEX IF NOT EXISTS idx_ac_created ON automatic_commits(created_at);
CREATE INDEX IF NOT EXISTS idx_ac_type    ON automatic_commits(entity_type);

CREATE TABLE IF NOT EXISTS causality_records (
    id TEXT PRIMARY KEY,
    entity_id TEXT NOT NULL,
    version_id TEXT NOT NULL,
    reason TEXT NOT NULL DEFAULT '',
    affected_entities_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS version_edges (
    id TEXT PRIMARY KEY,
    from_version_id TEXT NOT NULL,
    to_version_id TEXT NOT NULL,
    relationship_type TEXT NOT NULL,
    created_at TEXT NOT NULL
);
