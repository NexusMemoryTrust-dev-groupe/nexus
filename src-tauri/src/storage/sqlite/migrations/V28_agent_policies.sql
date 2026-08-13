-- Agent-level memory permissions (Firewall, System 4):
-- локальный policy engine — какой агент какую память может видеть.
CREATE TABLE IF NOT EXISTS agent_policies (
    id TEXT PRIMARY KEY,
    agent TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'assistant',
    allowed_visibility TEXT NOT NULL DEFAULT '[]',  -- JSON array of visibility names
    allowed_layers TEXT NOT NULL DEFAULT '[]',       -- JSON array of layer names
    deny_patterns TEXT NOT NULL DEFAULT '[]',        -- JSON array of patterns
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_policies_agent
    ON agent_policies (agent);
