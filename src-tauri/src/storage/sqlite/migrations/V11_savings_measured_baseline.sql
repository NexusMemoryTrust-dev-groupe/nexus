-- V11: Replace the estimated savings baseline with measured values.
--
-- Why
-- ---
-- V10 stored `manual_context_tokens INTEGER NOT NULL DEFAULT 800` — a constant.
-- Every "tokens saved" figure in the UI was therefore derived from a guess,
-- while the product copy promised real data. These columns record what was
-- actually measured for each interaction so the arithmetic is verifiable:
--
--   baseline_tokens  tokens the model would have consumed reading the candidate
--                    sources in full (measured with the real BPE vocabulary)
--   context_tokens   tokens in the context package we actually sent (measured)
--   tokens_saved     baseline_tokens - context_tokens, never below zero
--
-- `token_method` records HOW the count was produced ('exact' via the BPE
-- vocabulary, or 'estimated' via the script-aware heuristic when the model
-- cache is absent), so a reader can tell measured rows from approximated ones
-- instead of having to trust a single number.
--
-- `candidate_*` capture the size of the corpus considered, which is what makes
-- the baseline reproducible: the same corpus and query must yield the same
-- baseline.

ALTER TABLE savings_log ADD COLUMN baseline_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE savings_log ADD COLUMN token_method TEXT NOT NULL DEFAULT 'estimated';
ALTER TABLE savings_log ADD COLUMN candidate_entities INTEGER NOT NULL DEFAULT 0;
ALTER TABLE savings_log ADD COLUMN candidate_memories INTEGER NOT NULL DEFAULT 0;

-- Rows written before this migration used the 800-token constant. Mark them so
-- reports can exclude unverifiable history rather than silently mixing it with
-- measured data.
UPDATE savings_log SET token_method = 'legacy_estimate' WHERE baseline_tokens = 0;

CREATE INDEX IF NOT EXISTS idx_savings_log_method ON savings_log(token_method);
