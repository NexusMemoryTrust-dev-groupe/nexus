-- V18: Cognitive memory layers — provenance columns and legacy mapping
--
-- V1 stored a 4-rung ladder (Raw/Knowledge/Decision/Wisdom). The cognitive
-- taxonomy has six layers (Working/Episodic/Semantic/Procedural/Decision/
-- Strategic). Existing rows are mapped onto the new taxonomy:
--   Raw       -> Episodic   (raw capture is an event)
--   Knowledge -> Semantic   (facts about the system/world)
--   Wisdom    -> Strategic  (principles, long-term direction)
-- Decision stays Decision.
--
-- Provenance columns record *why* a memory sits on a layer:
--   layer_confidence  : classifier score 0..1 (1.0 when user-set)
--   layer_reason      : short human-readable explanation (RU or EN)
--   layer_updated_at  : ISO-8601 timestamp of last layer change
--   layer_history_json: JSON array of {layer, confidence, reason, at, by}
ALTER TABLE memory_records ADD COLUMN layer_confidence REAL NOT NULL DEFAULT 0.5;
ALTER TABLE memory_records ADD COLUMN layer_reason TEXT NOT NULL DEFAULT '';
ALTER TABLE memory_records ADD COLUMN layer_updated_at TEXT;
ALTER TABLE memory_records ADD COLUMN layer_history_json TEXT NOT NULL DEFAULT '[]';

UPDATE memory_records SET layer = 'Episodic' WHERE layer = 'Raw';
UPDATE memory_records SET layer = 'Semantic' WHERE layer = 'Knowledge';
UPDATE memory_records SET layer = 'Strategic' WHERE layer = 'Wisdom';
UPDATE memory_records SET layer = 'Episodic' WHERE layer NOT IN (
    'Working', 'Episodic', 'Semantic', 'Procedural', 'Decision', 'Strategic'
);

-- Existing rows have no recorded provenance - mark them as migrated.
UPDATE memory_records
SET layer_reason = 'migrated to cognitive taxonomy (V18)',
    layer_updated_at = created_at
WHERE layer_updated_at IS NULL;
