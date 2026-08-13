-- V33: Index on memory_records.created_at (plan 5.5 load bounds)
-- list() orders by created_at DESC over the full table; without an index
-- SQLite performs a full sort of every row on each page load. At 100k rows
-- that alone costs ~200ms and grows superlinearly. The index makes page
-- loads (list LIMIT n) a straight index walk: constant time per page.
CREATE INDEX IF NOT EXISTS idx_memory_created_at
ON memory_records(created_at DESC);
