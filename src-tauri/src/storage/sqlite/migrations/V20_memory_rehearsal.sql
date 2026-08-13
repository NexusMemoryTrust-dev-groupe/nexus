-- V20: Memory Rehearsal — spaced-repetition цикл повторения (Система 3).
-- Каждая память получает расписание повторения: когда её нужно «освежить»
-- (перечитать/подтвердить), сколько раз она уже повторялась и когда следующий
-- заход. Память, которую не повторяют, постепенно забывается (важность падает);
-- повторенная — укрепляется. Колонки добавляются идемпотентно.
ALTER TABLE memory_records ADD COLUMN last_rehearsed_at TEXT;
ALTER TABLE memory_records ADD COLUMN rehearsal_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE memory_records ADD COLUMN next_rehearsal_at TEXT;

CREATE INDEX IF NOT EXISTS idx_memory_next_rehearsal ON memory_records(next_rehearsal_at);
