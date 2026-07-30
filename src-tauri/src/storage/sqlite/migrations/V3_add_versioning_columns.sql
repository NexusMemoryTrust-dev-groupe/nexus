-- V3: Add versioning columns to memory_records
ALTER TABLE memory_records ADD COLUMN derived_from_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE memory_records ADD COLUMN reason TEXT;
ALTER TABLE memory_records ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE memory_records ADD COLUMN updated_by TEXT;
