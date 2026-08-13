use rusqlite::{Connection, OptionalExtension, params};
use std::sync::Mutex;

use crate::core::knowledge::skill_genesis::{PatternSignature, ProposalStatus, SkillProposal};
use crate::core::result::Result;
use crate::storage::sqlite::schema;

/// SQLite-backed repository for Skill Genesis proposals (System 7).
///
/// Хранит кандидатов в `skill_proposals`: сигнатура паттерна, число повторений,
/// сгенерированное имя и описание, статус. Повторный скан не создаёт дубликаты
/// (UNIQUE (category, action)) и не трогает уже одобренные/отклонённые.
pub struct SqliteSkillProposalRepository {
    conn: Mutex<Connection>,
}

impl SqliteSkillProposalRepository {
    pub fn new(conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        schema::apply_migrations(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    #[cfg(test)]
    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::new(conn)
    }

    /// Сохранить кандидата. Если сигнатура уже есть — обновить счётчик и
    /// описание, статус не трогаем (одобренное не откатывается сканом).
    pub fn upsert_proposal(&self, p: &SkillProposal) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM skill_proposals WHERE category = ?1 AND action = ?2",
                params![p.signature.category, p.signature.action],
                |r| r.get(0),
            )
            .optional()?;
        match existing {
            Some(id) => {
                conn.execute(
                    "UPDATE skill_proposals
                     SET occurrences = ?2, name = ?3, description = ?4
                     WHERE id = ?1",
                    params![id, p.occurrences as i64, p.name, p.description],
                )?;
            }
            None => {
                conn.execute(
                    "INSERT INTO skill_proposals
                     (id, category, action, occurrences, name, description, status, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        p.id,
                        p.signature.category,
                        p.signature.action,
                        p.occurrences as i64,
                        p.name,
                        p.description,
                        p.status.as_str(),
                        p.created_at,
                    ],
                )?;
            }
        }
        Ok(())
    }

    /// Известные сигнатуры (для filter_existing при скане): категория + действие.
    pub fn known_signatures(&self) -> Result<Vec<PatternSignature>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT category, action FROM skill_proposals")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = Vec::new();
        for r in rows {
            let (category, action) = r?;
            out.push(PatternSignature::new(&category, &action));
        }
        Ok(out)
    }

    /// Кандидаты со статусом (по умолчанию все).
    pub fn list(&self, status: Option<ProposalStatus>) -> Result<Vec<SkillProposal>> {
        let conn = self.conn.lock().unwrap();
        let sql = match status {
            Some(_) => {
                "SELECT id, category, action, occurrences, name, description, status, created_at
                 FROM skill_proposals WHERE status = ?1 ORDER BY occurrences DESC"
            }
            None => {
                "SELECT id, category, action, occurrences, name, description, status, created_at
                 FROM skill_proposals ORDER BY occurrences DESC"
            }
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = match status {
            Some(s) => stmt.query_map([s.as_str()], Self::row_to_proposal)?,
            None => stmt.query_map([], Self::row_to_proposal)?,
        };
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Обновить статус кандидата по id.
    pub fn set_status(&self, id: &str, status: ProposalStatus) -> Result<Option<SkillProposal>> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE skill_proposals SET status = ?2 WHERE id = ?1",
            params![id, status.as_str()],
        )?;
        if n == 0 {
            return Ok(None);
        }
        let p = conn
            .query_row(
                "SELECT id, category, action, occurrences, name, description, status, created_at
                 FROM skill_proposals WHERE id = ?1",
                params![id],
                Self::row_to_proposal,
            )
            .optional()?;
        Ok(p)
    }

    fn row_to_proposal(row: &rusqlite::Row<'_>) -> rusqlite::Result<SkillProposal> {
        let id: String = row.get(0)?;
        let category: String = row.get(1)?;
        let action: String = row.get(2)?;
        let occurrences: i64 = row.get(3)?;
        let name: String = row.get(4)?;
        let description: String = row.get(5)?;
        let status: String = row.get(6)?;
        let created_at: String = row.get(7)?;
        Ok(SkillProposal {
            id,
            signature: PatternSignature::new(&category, &action),
            occurrences: occurrences as usize,
            name,
            description,
            status: ProposalStatus::parse(&status),
            created_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::flight::flight_recorder::{FlightCategory, FlightRecord};
    use crate::core::knowledge::skill_genesis::propose;

    fn sample_proposal(name: &str, action: &str) -> SkillProposal {
        let record = FlightRecord::success(
            None,
            "agent",
            FlightCategory::Memory,
            action,
            "MemoryRecord",
            "id",
            "created",
            serde_json::json!({}),
            3,
        );
        let mut proposals = propose(crate::core::knowledge::skill_genesis::detect_patterns(
            &[record.clone(), record],
            2,
        ));
        proposals[0].name = name.to_string();
        proposals[0].id = format!("prop-{name}");
        proposals.remove(0)
    }

    #[test]
    fn upsert_and_list_roundtrip() {
        let repo = SqliteSkillProposalRepository::new_in_memory().unwrap();
        let p = sample_proposal("create-memory", "create_memory");
        repo.upsert_proposal(&p).unwrap();
        let all = repo.list(None).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "create-memory");
        assert_eq!(all[0].status, ProposalStatus::Proposed);
    }

    #[test]
    fn upsert_is_unique_by_signature() {
        let repo = SqliteSkillProposalRepository::new_in_memory().unwrap();
        let mut p1 = sample_proposal("create-memory", "create_memory");
        repo.upsert_proposal(&p1).unwrap();
        p1.occurrences = 10;
        repo.upsert_proposal(&p1).unwrap();
        assert_eq!(repo.list(None).unwrap().len(), 1);
        assert_eq!(repo.list(None).unwrap()[0].occurrences, 10);
    }

    #[test]
    fn known_signatures_roundtrip() {
        let repo = SqliteSkillProposalRepository::new_in_memory().unwrap();
        repo.upsert_proposal(&sample_proposal("a", "create_memory"))
            .unwrap();
        repo.upsert_proposal(&sample_proposal("b", "resolve_conflict"))
            .unwrap();
        let sigs = repo.known_signatures().unwrap();
        assert_eq!(sigs.len(), 2);
        assert!(sigs.contains(&PatternSignature::new("memory", "create_memory")));
    }

    #[test]
    fn set_status_and_filter_by_status() {
        let repo = SqliteSkillProposalRepository::new_in_memory().unwrap();
        repo.upsert_proposal(&sample_proposal("a", "create_memory"))
            .unwrap();
        repo.upsert_proposal(&sample_proposal("b", "resolve_conflict"))
            .unwrap();
        let approved = repo.set_status("prop-a", ProposalStatus::Approved).unwrap();
        assert!(approved.is_some());
        let approved_list = repo.list(Some(ProposalStatus::Approved)).unwrap();
        assert_eq!(approved_list.len(), 1);
        assert_eq!(approved_list[0].name, "a");
        let rejected_list = repo.list(Some(ProposalStatus::Rejected)).unwrap();
        assert!(rejected_list.is_empty());
    }

    #[test]
    fn set_status_unknown_id_returns_none() {
        let repo = SqliteSkillProposalRepository::new_in_memory().unwrap();
        let r = repo.set_status("nope", ProposalStatus::Approved).unwrap();
        assert!(r.is_none());
    }
}
