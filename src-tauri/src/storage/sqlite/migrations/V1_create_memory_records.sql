-- V1: Create memory_records table (base schema) and FTS index
-- Columns added in later migrations: attached_files_json (V2),
-- derived_from_json, reason, version, updated_by (V3)
CREATE TABLE IF NOT EXISTS memory_records (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    author TEXT NOT NULL,
    source TEXT NOT NULL,
    confidence_score REAL NOT NULL DEFAULT 0.5,
    importance_score REAL NOT NULL DEFAULT 0.5,
    visibility TEXT NOT NULL DEFAULT 'Private',
    capture_mode TEXT NOT NULL DEFAULT 'Passive',
    project_space_id TEXT,
    linked_entity_ids_json TEXT NOT NULL DEFAULT '[]',
    latest_version_id TEXT,
    status TEXT NOT NULL DEFAULT 'Active',
    layer TEXT NOT NULL DEFAULT 'Raw'
);

CREATE INDEX IF NOT EXISTS idx_memory_project ON memory_records(project_space_id);
CREATE INDEX IF NOT EXISTS idx_memory_author ON memory_records(author);
CREATE INDEX IF NOT EXISTS idx_memory_status ON memory_records(status);
CREATE INDEX IF NOT EXISTS idx_memory_layer ON memory_records(layer);

CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
    id,
    title,
    summary,
    content,
    content=memory_records,
    content_rowid=rowid
);

CREATE TRIGGER IF NOT EXISTS memory_records_ai AFTER INSERT ON memory_records BEGIN
    INSERT INTO memory_fts(rowid, id, title, summary, content)
    VALUES (new.rowid, new.id, new.title, new.summary, new.content);
END;

CREATE TRIGGER IF NOT EXISTS memory_records_ad AFTER DELETE ON memory_records BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, id, title, summary, content)
    VALUES ('delete', old.rowid, old.id, old.title, old.summary, old.content);
END;

CREATE TRIGGER IF NOT EXISTS memory_records_au AFTER UPDATE ON memory_records BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, id, title, summary, content)
    VALUES ('delete', old.rowid, old.id, old.title, old.summary, old.content);
    INSERT INTO memory_fts(rowid, id, title, summary, content)
    VALUES (new.rowid, new.id, new.title, new.summary, new.content);
END;
