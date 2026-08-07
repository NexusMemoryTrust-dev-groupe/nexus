-- V14: Project knowledge base — RAG documents, AGENTS.md-style instructions, skills.
--
-- Implements the "RAG + AGENTS.md + skills" ideas:
--
--   * project_documents        imported .md/.txt docs from the project (RAG corpus)
--   * document_fingerprints    semantic embeddings for those documents, stored
--                              separately from memory fingerprints so vector
--                              search can query docs and memories independently
--   * agents_documents         AGENTS.md-style instruction files, one per name
--                              (e.g. 'AGENTS.md'), content is injected into
--                              context packages so the AI follows project rules
--   * skills                   runnable commands (JS scripts etc.) that agents
--                              can invoke without the full MCP tool surface
--
-- Content is stored as TEXT and checksummed on import so a re-import of an
-- unchanged file is a no-op instead of a rewrite.

CREATE TABLE IF NOT EXISTS project_documents (
    id          TEXT PRIMARY KEY NOT NULL,
    path        TEXT NOT NULL,
    title       TEXT NOT NULL DEFAULT '',
    content     TEXT NOT NULL DEFAULT '',
    doc_type    TEXT NOT NULL DEFAULT 'markdown',
    source      TEXT NOT NULL DEFAULT 'manual',
    checksum    TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_project_documents_path ON project_documents(path);

CREATE TABLE IF NOT EXISTS document_fingerprints (
    document_id     TEXT PRIMARY KEY NOT NULL,
    keywords_json   TEXT NOT NULL DEFAULT '{}',
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS agents_documents (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    content     TEXT NOT NULL DEFAULT '',
    path        TEXT NOT NULL DEFAULT '',
    checksum    TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_agents_documents_name ON agents_documents(name);

CREATE TABLE IF NOT EXISTS skills (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    command     TEXT NOT NULL,
    script_path TEXT NOT NULL DEFAULT '',
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_skills_name ON skills(name);
