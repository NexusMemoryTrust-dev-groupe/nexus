-- V26: Predictive Context — история запросов для предсказания следующего
-- шага (Система 8).
--
-- Каждый запрос, прошедший через контекстный движок, оседает здесь вместе с
-- интентом и задействованными сущностями. Марковская цепь переходов по этой
-- таблице позволяет предсказать следующий запрос и прогреть кэш контекста.

CREATE TABLE IF NOT EXISTS query_history (
    id          TEXT PRIMARY KEY,
    query       TEXT NOT NULL,               -- вопрос как был задан
    intent_type TEXT NOT NULL DEFAULT '',    -- explain | explore | change | ...
    entities_json TEXT NOT NULL DEFAULT '[]',-- JSON-массив id задействованных сущностей
    created_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_query_history_created ON query_history(created_at);
