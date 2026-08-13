-- Context Chain Recording (Flight Recorder, System 5):
-- полная цепочка построения контекста для объяснимости ответов («Why did AI say this?»).
CREATE TABLE IF NOT EXISTS context_chains (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    actor TEXT NOT NULL DEFAULT 'user',
    query TEXT NOT NULL,
    intent TEXT NOT NULL DEFAULT '',
    answer_confidence REAL NOT NULL DEFAULT 0.0,
    answer TEXT NOT NULL DEFAULT '',
    seeds_json TEXT NOT NULL DEFAULT '[]',   -- JSON array of ContextSeed
    stages_json TEXT NOT NULL DEFAULT '[]',  -- JSON array of StageRecord
    total_tokens INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_context_chains_created
    ON context_chains (created_at DESC);

CREATE INDEX IF NOT EXISTS idx_context_chains_session
    ON context_chains (session_id);
