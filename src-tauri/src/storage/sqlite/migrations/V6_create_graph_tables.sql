-- V6: Create knowledge graph tables
CREATE TABLE IF NOT EXISTS graph_entities (
    id TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'Active',
    metadata_json TEXT NOT NULL DEFAULT '{}',
    canonical_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_ge_type   ON graph_entities(entity_type);
CREATE INDEX IF NOT EXISTS idx_ge_status ON graph_entities(status);
CREATE INDEX IF NOT EXISTS idx_ge_title  ON graph_entities(title);

CREATE TABLE IF NOT EXISTS graph_relationships (
    id TEXT PRIMARY KEY,
    source_entity_id TEXT NOT NULL,
    target_entity_id TEXT NOT NULL,
    relationship_type TEXT NOT NULL,
    weight REAL NOT NULL DEFAULT 1.0,
    created_at TEXT NOT NULL,
    created_by TEXT NOT NULL DEFAULT '',
    metadata_json TEXT NOT NULL DEFAULT '{}',
    FOREIGN KEY (source_entity_id) REFERENCES graph_entities(id) ON DELETE CASCADE,
    FOREIGN KEY (target_entity_id) REFERENCES graph_entities(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_gr_source ON graph_relationships(source_entity_id);
CREATE INDEX IF NOT EXISTS idx_gr_target ON graph_relationships(target_entity_id);
CREATE INDEX IF NOT EXISTS idx_gr_type   ON graph_relationships(relationship_type);
