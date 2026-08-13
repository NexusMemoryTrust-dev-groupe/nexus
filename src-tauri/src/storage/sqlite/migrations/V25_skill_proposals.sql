-- V25: Skill Genesis — кандидаты в скиллы, обнаруженные по паттернам
-- повторяющихся действий из журнала полёта (Система 7).
--
-- Nexus замечает: «это действие выполняется уже N раз» — и предлагает
-- превратить его в скилл. Кандидаты хранятся здесь, пока человек (или агент)
-- не одобрит или не отклонит их.

CREATE TABLE IF NOT EXISTS skill_proposals (
    id           TEXT PRIMARY KEY,
    category     TEXT NOT NULL,               -- memory | conflict | firewall | ...
    action       TEXT NOT NULL,               -- create_memory | resolve_conflict | ...
    occurrences  INTEGER NOT NULL DEFAULT 0,  -- сколько раз встретилось в журнале
    name         TEXT NOT NULL,               -- кандидатское имя скилла (kebab-case)
    description  TEXT NOT NULL DEFAULT '',    -- сгенерированное описание
    status       TEXT NOT NULL DEFAULT 'proposed', -- proposed | approved | rejected
    created_at   TEXT NOT NULL,
    UNIQUE (category, action)
);

CREATE INDEX IF NOT EXISTS idx_skill_proposals_status ON skill_proposals(status);
