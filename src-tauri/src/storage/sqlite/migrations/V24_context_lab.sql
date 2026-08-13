-- V24: Context Lab — лаборатория качества контекста (Система 6).
-- Расширяет context-движок: один вопрос собирается несколькими стратегиями
-- (compact / balanced / rich), каждая снимает метрики и предсказание точности.
-- Эксперименты сохраняются, чтобы Nexus со временем учился выбирать стратегию.
--
-- Одна таблица: context_lab_runs — один эксперимент = один вопрос + JSON-срез
-- результатов всех стратегий (счётчики, токены, релевантность, точность).

CREATE TABLE IF NOT EXISTS context_lab_runs (
    id         TEXT PRIMARY KEY,
    query      TEXT NOT NULL,               -- вопрос, для которого собирали контекст
    results_json TEXT NOT NULL DEFAULT '[]',-- JSON-массив LabResult
    best_strategy TEXT NOT NULL DEFAULT '', -- победитель по предсказанной точности
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_context_lab_created ON context_lab_runs(created_at);
CREATE INDEX IF NOT EXISTS idx_context_lab_query ON context_lab_runs(query);
