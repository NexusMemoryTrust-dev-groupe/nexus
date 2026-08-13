-- Canonical consolidation (Memory Rehearsal, System 3):
-- синтез канонических записей из повторяющихся событий с сохранением provenance.
CREATE TABLE IF NOT EXISTS canonical_memories (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    content TEXT NOT NULL,
    author TEXT NOT NULL,
    member_ids TEXT NOT NULL,          -- JSON array of source memory ids
    member_count INTEGER NOT NULL,
    cohesion REAL NOT NULL DEFAULT 0.0, -- avg pairwise similarity inside the cluster
    importance_score REAL NOT NULL DEFAULT 0.5,
    confidence_score REAL NOT NULL DEFAULT 0.5,
    layer TEXT NOT NULL DEFAULT 'Episodic',
    created_at TEXT NOT NULL,
    source_memory_id TEXT               -- the canonical MemoryRecord id in the memory table
);

CREATE INDEX IF NOT EXISTS idx_canonical_memories_created
    ON canonical_memories (created_at DESC);
