-- V31: Create backup_history table (Production Readiness Gate 3.1)
-- Append-only journal of every backup created and every restore applied.
-- Enables the UI to render "backups", and gives the audit trail a durable
-- record that survives file-system reshuffling.
CREATE TABLE IF NOT EXISTS backup_history (
    id             TEXT PRIMARY KEY,        -- uuid
    path           TEXT NOT NULL,           -- absolute path of the .nexusbackup file
    created_at     TEXT NOT NULL,           -- RFC3339, when the backup was created
    schema_version INTEGER NOT NULL,        -- DB schema version captured in the backup
    size_bytes     INTEGER NOT NULL,        -- size of the .nexusbackup file
    sha256         TEXT NOT NULL,           -- hex digest of the snapshot payload
    status         TEXT NOT NULL DEFAULT 'active', -- active | restored | deleted
    restored_at    TEXT,                    -- RFC3339 when a restore was applied from it
    note           TEXT                     -- free-form (e.g. "auto pre-migration safety")
);

CREATE INDEX IF NOT EXISTS idx_bh_created ON backup_history(created_at);
CREATE INDEX IF NOT EXISTS idx_bh_status  ON backup_history(status);
