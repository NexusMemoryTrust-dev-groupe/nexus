-- V23: Agent Passport — компактная идентификационная карточка агента (Система 6).
-- Расширяет AGENTS.md (knowledge/agents) и скиллы (knowledge/skills): помимо
-- длинных инструкций, каждый агент получает машиночитаемый «паспорт» —
-- роль, стек скиллов, разрешённые инструменты, ограничения и уровень
-- доверия. Паспорт прикрепляется к контекстному пакету, чтобы ИИ знал,
-- кто он, что ему разрешено и во что он верит.
--
-- Одна таблица: agent_passports — по одному паспорту на агента (name = id).

CREATE TABLE IF NOT EXISTS agent_passports (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,      -- идентичность агента (например "opencode-primary")
    display_name    TEXT NOT NULL DEFAULT '',  -- человекочитаемое имя
    role            TEXT NOT NULL DEFAULT 'generalist', -- generalist | coder | researcher | reviewer | orchestrator | memory-keeper
    description     TEXT NOT NULL DEFAULT '',  -- чем занимается агент (1-2 предложения)
    skills_json     TEXT NOT NULL DEFAULT '[]',-- JSON-массив имён доступных скиллов
    tools_json      TEXT NOT NULL DEFAULT '[]',-- JSON-массив разрешённых MCP-инструментов
    constraints_json TEXT NOT NULL DEFAULT '[]',-- JSON-массив ограничений (чего НЕ делать)
    trust_level     INTEGER NOT NULL DEFAULT 5, -- 1..10: насколько памяти агента можно верить
    memory_scope    TEXT NOT NULL DEFAULT 'project', -- personal | project | team | global
    is_active       INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_passports_role ON agent_passports(role);
CREATE INDEX IF NOT EXISTS idx_agent_passports_active ON agent_passports(is_active);
