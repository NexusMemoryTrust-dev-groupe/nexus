-- V9: Create semantic fingerprints table
CREATE TABLE IF NOT EXISTS memory_semantic_fingerprints (
    memory_id       TEXT PRIMARY KEY,
    keywords_json   TEXT NOT NULL DEFAULT '{}',
    created_at      TEXT NOT NULL
);
