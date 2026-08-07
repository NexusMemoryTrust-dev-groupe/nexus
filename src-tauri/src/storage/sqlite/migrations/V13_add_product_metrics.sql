-- V13: Product metrics columns on savings_log.
--
-- The task list asks for product metrics that prove value:
--   latency, precision, used fragments, irrelevant fragments, manual context,
--   stale memories, memory fixes, memory reuse across sessions.
--
-- V10/V11 already measure tokens saved against a baseline. This migration adds
-- the remaining columns so reports can answer those questions without inventing
-- numbers:
--
--   latency_ms           context build latency, measured by the caller (ms)
--   precision            included / (included + dropped) from the provenance
--                        trace — the engine's own accounting of relevance
--   used_fragments       fragments the calling agent reports as actually used
--                        in the final answer
--   irrelevant_fragments fragments the ranker dropped as below the relevance
--                        floor
--   manual_context       1 when the user pasted context manually this round,
--                        so "share of queries without manual context" is a
--                        measured ratio, not a guess
--
-- `context_memory_usage` records which memories each interaction delivered, so
-- "memory reuse across sessions" is a count: memories that appeared in more
-- than one interaction.

ALTER TABLE savings_log ADD COLUMN latency_ms INTEGER NOT NULL DEFAULT 0;
ALTER TABLE savings_log ADD COLUMN precision REAL NOT NULL DEFAULT 0.0;
ALTER TABLE savings_log ADD COLUMN used_fragments INTEGER NOT NULL DEFAULT 0;
ALTER TABLE savings_log ADD COLUMN irrelevant_fragments INTEGER NOT NULL DEFAULT 0;
ALTER TABLE savings_log ADD COLUMN manual_context INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS context_memory_usage (
    interaction_id TEXT NOT NULL,
    memory_id      TEXT NOT NULL,
    PRIMARY KEY (interaction_id, memory_id)
);

CREATE INDEX IF NOT EXISTS idx_context_memory_usage_memory ON context_memory_usage(memory_id);
