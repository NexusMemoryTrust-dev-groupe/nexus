-- V8: Create workspace_entries and memory_entity_links tables
CREATE TABLE IF NOT EXISTS workspace_entries (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL,
    name        TEXT NOT NULL,
    native_path TEXT NOT NULL,
    parent_id   TEXT,
    is_dir      INTEGER NOT NULL DEFAULT 0,
    size_bytes  INTEGER NOT NULL DEFAULT 0,
    mime_type   TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_we_project  ON workspace_entries(project_id);
CREATE INDEX IF NOT EXISTS idx_we_parent   ON workspace_entries(project_id, parent_id);
CREATE INDEX IF NOT EXISTS idx_we_native   ON workspace_entries(project_id, native_path);

CREATE TABLE IF NOT EXISTS memory_entity_links (
    id              TEXT PRIMARY KEY,
    memory_id       TEXT NOT NULL,
    entity_id       TEXT NOT NULL,
    relationship    TEXT NOT NULL DEFAULT 'Related',
    weight          REAL NOT NULL DEFAULT 1.0,
    created_at      TEXT NOT NULL,
    created_by      TEXT NOT NULL DEFAULT 'system',
    UNIQUE(memory_id, entity_id, relationship)
);

CREATE INDEX IF NOT EXISTS idx_mel_memory ON memory_entity_links(memory_id);
CREATE INDEX IF NOT EXISTS idx_mel_entity ON memory_entity_links(entity_id);
CREATE INDEX IF NOT EXISTS idx_mel_rel    ON memory_entity_links(relationship);
