use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use std::sync::Mutex;

use crate::core::audit::audit_repository::AuditRepository;
use crate::core::audit::{AuditEvent, AuditEventType};
use crate::core::entity_id::EntityId;
use crate::core::result::Result;
use crate::storage::sqlite::schema;

/// SQLite-backed implementation of the decision journal.
/// Same Mutex<Connection> pattern as the other repositories.
pub struct SqliteAuditRepository {
    conn: Mutex<Connection>,
}

impl SqliteAuditRepository {
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
}

fn row_to_event(row: &rusqlite::Row) -> rusqlite::Result<AuditEvent> {
    let id_str: String = row.get(0)?;
    let memory_id_str: String = row.get(1)?;
    let event_type_str: String = row.get(2)?;
    let actor: String = row.get(3)?;
    let detail: Option<String> = row.get(4)?;
    let related_memory_id: Option<String> = row.get(5)?;
    let created_at: String = row.get(6)?;
    Ok(AuditEvent {
        id: EntityId::parse(&id_str)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        memory_id: EntityId::parse(&memory_id_str)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        event_type: AuditEventType::parse(&event_type_str),
        actor,
        detail,
        related_memory_id,
        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
    })
}

#[async_trait]
impl AuditRepository for SqliteAuditRepository {
    async fn add_event(&self, event: &AuditEvent) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO audit_events (id, memory_id, event_type, actor, detail, related_memory_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.id.as_str(),
                event.memory_id.as_str(),
                event.event_type.as_str(),
                event.actor,
                event.detail,
                event.related_memory_id,
                event.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    async fn list_events(&self, memory_id: &EntityId) -> Result<Vec<AuditEvent>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, memory_id, event_type, actor, detail, related_memory_id, created_at
             FROM audit_events WHERE memory_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![memory_id.as_str()], row_to_event)?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }

    async fn list_all_events(&self) -> Result<Vec<AuditEvent>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, memory_id, event_type, actor, detail, related_memory_id, created_at
             FROM audit_events ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], row_to_event)?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::audit::DecisionAlternative;

    #[tokio::test]
    async fn add_and_list_events() {
        let repo = SqliteAuditRepository::new_in_memory().unwrap();
        let memory_id = EntityId::new();
        let e1 = AuditEvent::new(
            memory_id.clone(),
            AuditEventType::Alternative,
            "alice".to_string(),
            Some(
                DecisionAlternative {
                    title: "MySQL".to_string(),
                    reason: "license costs".to_string(),
                }
                .to_detail_json(),
            ),
            None,
        );
        let e2 = AuditEvent::new(
            memory_id.clone(),
            AuditEventType::Confirmed,
            "bob".to_string(),
            None,
            None,
        );
        repo.add_event(&e1).await.unwrap();
        repo.add_event(&e2).await.unwrap();

        let events = repo.list_events(&memory_id).await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, AuditEventType::Alternative);
        assert_eq!(events[1].event_type, AuditEventType::Confirmed);
        assert_eq!(events[0].actor, "alice");
    }

    #[tokio::test]
    async fn events_isolated_per_memory() {
        let repo = SqliteAuditRepository::new_in_memory().unwrap();
        let m1 = EntityId::new();
        let m2 = EntityId::new();
        repo.add_event(&AuditEvent::new(
            m1.clone(),
            AuditEventType::Created,
            "alice".into(),
            None,
            None,
        ))
        .await
        .unwrap();
        repo.add_event(&AuditEvent::new(
            m2.clone(),
            AuditEventType::Note,
            "bob".into(),
            Some("note".into()),
            None,
        ))
        .await
        .unwrap();

        let m1_events = repo.list_events(&m1).await.unwrap();
        let m2_events = repo.list_events(&m2).await.unwrap();
        assert_eq!(m1_events.len(), 1);
        assert_eq!(m2_events.len(), 1);
        assert_eq!(m1_events[0].event_type, AuditEventType::Created);
        assert_eq!(m2_events[0].event_type, AuditEventType::Note);
    }

    #[tokio::test]
    async fn no_events_for_unknown_memory() {
        let repo = SqliteAuditRepository::new_in_memory().unwrap();
        let events = repo.list_events(&EntityId::new()).await.unwrap();
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn audit_is_append_only_updates_forbidden() {
        // Plan 4.5: the journal is immutable. After an event is inserted,
        // UPDATE must fail — otherwise the trail could be rewritten.
        let repo = SqliteAuditRepository::new_in_memory().unwrap();
        let memory_id = EntityId::new();
        repo.add_event(&AuditEvent::new(
            memory_id.clone(),
            AuditEventType::Note,
            "alice".into(),
            Some("original".into()),
            None,
        ))
        .await
        .unwrap();

        let conn = repo.conn.lock().unwrap();
        let err = conn.execute(
            "UPDATE audit_events SET detail = 'tampered' WHERE memory_id = ?1",
            rusqlite::params![memory_id.as_str()],
        );
        assert!(
            err.is_err(),
            "UPDATE on audit_events must be rejected by the append-only trigger"
        );
        let msg = err.unwrap_err().to_string();
        assert!(
            msg.contains("append-only"),
            "trigger must say append-only, got: {msg}"
        );

        // The original row is untouched.
        let original: Option<String> = conn
            .query_row(
                "SELECT detail FROM audit_events WHERE memory_id = ?1",
                rusqlite::params![memory_id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(original.as_deref(), Some("original"));
    }

    #[tokio::test]
    async fn audit_is_append_only_deletes_forbidden() {
        let repo = SqliteAuditRepository::new_in_memory().unwrap();
        let memory_id = EntityId::new();
        repo.add_event(&AuditEvent::new(
            memory_id.clone(),
            AuditEventType::Created,
            "alice".into(),
            None,
            None,
        ))
        .await
        .unwrap();

        let conn = repo.conn.lock().unwrap();
        let err = conn.execute(
            "DELETE FROM audit_events WHERE memory_id = ?1",
            rusqlite::params![memory_id.as_str()],
        );
        assert!(
            err.is_err(),
            "DELETE on audit_events must be rejected by the append-only trigger"
        );
        assert!(err.unwrap_err().to_string().contains("append-only"));

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audit_events WHERE memory_id = ?1",
                rusqlite::params![memory_id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "the event must survive the failed delete");
    }

    #[tokio::test]
    async fn permission_and_firewall_events_roundtrip() {
        // Plan 4.5: full event coverage — permission changes and firewall
        // denials are auditable events like any decision.
        let repo = SqliteAuditRepository::new_in_memory().unwrap();
        let memory_id = EntityId::new();

        let perm = AuditEvent::new(
            memory_id.clone(),
            AuditEventType::PermissionChanged,
            "admin".into(),
            Some(r#"{"agent_id":"claude-code","change":"revoke:secrets"}"#.into()),
            None,
        );
        let fw = AuditEvent::new(
            memory_id.clone(),
            AuditEventType::FirewallDenied,
            "firewall".into(),
            Some(r#"{"pattern":"password","memory_id":"m1"}"#.into()),
            None,
        );
        repo.add_event(&perm).await.unwrap();
        repo.add_event(&fw).await.unwrap();

        let events = repo.list_events(&memory_id).await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, AuditEventType::PermissionChanged);
        assert_eq!(events[1].event_type, AuditEventType::FirewallDenied);
        assert_eq!(events[0].actor, "admin");
        assert!(events[1].detail.as_deref().unwrap().contains("password"));
    }
}
