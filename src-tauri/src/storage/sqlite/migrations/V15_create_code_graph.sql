-- V15: Code graph — structured layer over source files.
--
-- The existing interpreter parsers (code_parser.rs) already extract classes,
-- functions, structs, traits, interfaces from 7+ languages. This migration
-- adds the missing piece: a persisted, queryable structure that ties those
-- symbols to their files and — the part parsers never produced — records the
-- dependency edges between files (import / require / use / #include / mod).
--
--   * code_files         one row per indexed source file (path, language, checksum)
--   * code_symbols       symbols extracted by the language parsers, tied to a file
--   * code_dependencies  edges: file → target module/file (internal or external)
--
-- The layer is separate from the memory graph and from project_documents so
-- queries stay fast and the AI can ask "what depends on X?" without polluting
-- semantic memory with code structure.

CREATE TABLE IF NOT EXISTS code_files (
    id          TEXT PRIMARY KEY NOT NULL,
    path        TEXT NOT NULL,
    title       TEXT NOT NULL DEFAULT '',
    language    TEXT NOT NULL DEFAULT '',
    checksum    TEXT NOT NULL DEFAULT '',
    line_count  INTEGER NOT NULL DEFAULT 0,
    symbol_count INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_code_files_path ON code_files(path);

CREATE TABLE IF NOT EXISTS code_symbols (
    id          TEXT PRIMARY KEY NOT NULL,
    file_id     TEXT NOT NULL,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL DEFAULT 'symbol',
    language    TEXT NOT NULL DEFAULT '',
    signature   TEXT NOT NULL DEFAULT '',
    line        INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    FOREIGN KEY (file_id) REFERENCES code_files(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_code_symbols_file ON code_symbols(file_id);
CREATE INDEX IF NOT EXISTS idx_code_symbols_name ON code_symbols(name);

CREATE TABLE IF NOT EXISTS code_dependencies (
    id              TEXT PRIMARY KEY NOT NULL,
    file_id         TEXT NOT NULL,
    target          TEXT NOT NULL,
    kind            TEXT NOT NULL DEFAULT 'import',
    is_external     INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL,
    FOREIGN KEY (file_id) REFERENCES code_files(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_code_dependencies_file ON code_dependencies(file_id);
CREATE INDEX IF NOT EXISTS idx_code_dependencies_target ON code_dependencies(target);
