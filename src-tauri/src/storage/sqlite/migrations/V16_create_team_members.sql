-- V16: Team Memory — участники команды и их роли.
-- Доверенный слой решений команды: кто подтвердил, что устарело, что в конфликте.
-- Роли: admin (управляет командой), member (создаёт/подтверждает), viewer (только чтение).
CREATE TABLE IF NOT EXISTS team_members (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    role        TEXT NOT NULL DEFAULT 'member',
    active      INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_team_members_role ON team_members(role);
CREATE INDEX IF NOT EXISTS idx_team_members_active ON team_members(active);
