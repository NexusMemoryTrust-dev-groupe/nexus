-- V2: Add attached_files_json column to memory_records
ALTER TABLE memory_records ADD COLUMN attached_files_json TEXT NOT NULL DEFAULT '[]';
