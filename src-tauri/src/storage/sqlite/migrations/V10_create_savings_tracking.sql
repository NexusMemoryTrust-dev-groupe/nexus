-- Tracks token savings from context engine usage.
-- Each row = one context build interaction.
CREATE TABLE IF NOT EXISTS savings_log (
    id              TEXT PRIMARY KEY,
    -- Context engine output
    context_tokens  INTEGER NOT NULL DEFAULT 0,
    entities_count  INTEGER NOT NULL DEFAULT 0,
    memories_count  INTEGER NOT NULL DEFAULT 0,
    relationships_count INTEGER NOT NULL DEFAULT 0,
    -- Without Nexus the user would need to manually type/paste context.
    -- Estimate: avg tokens a user would provide manually per interaction.
    manual_context_tokens INTEGER NOT NULL DEFAULT 800,
    -- Tokens saved = manual context that Nexus provided automatically.
    tokens_saved    INTEGER NOT NULL DEFAULT 0,
    -- Cost saved in USD (based on avg LLM pricing).
    cost_saved_usd  REAL NOT NULL DEFAULT 0.0,
    -- Metadata
    query_text      TEXT,
    intent_type     TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Index for fast aggregation queries (daily/weekly/monthly stats).
CREATE INDEX IF NOT EXISTS idx_savings_log_created ON savings_log(created_at);
