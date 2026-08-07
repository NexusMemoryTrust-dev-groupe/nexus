//! Audit Memory — проверяемая память.
//!
//! The enterprise door: "Why did we choose PostgreSQL in March?" — with the
//! full chain: context, alternatives considered, who confirmed, what replaced
//! it. Compliance selling: prove the team knew and why it decided so.
//!
//! The audit trail for a memory is built from three sources:
//!   1. `MemoryRecord` — author, reason, derived_from, confirmed_by/at,
//!      supersedes/superseded_by, memory_state (the decision state).
//!   2. `audit_events` — the append-only decision journal (alternatives
//!      considered, notes, confirmations, supersessions).
//!   3. `automatic_commits` (versioning) — who changed what and when, with
//!      diffs and change reasons (the version history).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity_id::EntityId;
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::versioning::automatic_commit::{AutomaticCommit, ChangeType};

pub mod audit_repository;

pub use audit_repository::AuditRepository;

fn change_type_str(ct: &ChangeType) -> &'static str {
    match ct {
        ChangeType::Created => "Created",
        ChangeType::Modified => "Modified",
        ChangeType::Deleted => "Deleted",
    }
}

/// Type of an auditable decision event on a memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEventType {
    /// The memory was created — the decision was born.
    Created,
    /// An alternative was considered (and usually rejected). `detail` holds
    /// the JSON `{ "title": ..., "reason": ... }`.
    Alternative,
    /// A human explicitly confirmed this decision.
    Confirmed,
    /// This decision was superseded by a newer one. `related_memory_id`
    /// points at the memory that replaced it.
    Superseded,
    /// A free-form note attached to the decision trail.
    Note,
}

impl AuditEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditEventType::Created => "Created",
            AuditEventType::Alternative => "Alternative",
            AuditEventType::Confirmed => "Confirmed",
            AuditEventType::Superseded => "Superseded",
            AuditEventType::Note => "Note",
        }
    }

    pub fn parse(s: &str) -> AuditEventType {
        match s {
            "Alternative" => AuditEventType::Alternative,
            "Confirmed" => AuditEventType::Confirmed,
            "Superseded" => AuditEventType::Superseded,
            "Note" => AuditEventType::Note,
            _ => AuditEventType::Created,
        }
    }
}

/// A single row in the append-only decision journal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: EntityId,
    pub memory_id: EntityId,
    pub event_type: AuditEventType,
    /// Who performed the action (member name / user / system).
    pub actor: String,
    /// Free text; for Alternative events this is `{ "title", "reason" }` JSON.
    pub detail: Option<String>,
    /// The memory this event points at (e.g. the one that superseded this one).
    pub related_memory_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl AuditEvent {
    pub fn new(
        memory_id: EntityId,
        event_type: AuditEventType,
        actor: String,
        detail: Option<String>,
        related_memory_id: Option<String>,
    ) -> Self {
        Self {
            id: EntityId::new(),
            memory_id,
            event_type,
            actor,
            detail,
            related_memory_id,
            created_at: Utc::now(),
        }
    }
}

/// An alternative that was considered for a decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecisionAlternative {
    pub title: String,
    /// Why it was not chosen.
    pub reason: String,
}

impl DecisionAlternative {
    /// Serialize to the JSON form stored in `audit_events.detail`.
    pub fn to_detail_json(&self) -> String {
        serde_json::json!({ "title": self.title, "reason": self.reason }).to_string()
    }

    /// Parse from the JSON form stored in `audit_events.detail`.
    pub fn from_detail_json(detail: &str) -> Option<DecisionAlternative> {
        let v: serde_json::Value = serde_json::from_str(detail).ok()?;
        Some(DecisionAlternative {
            title: v.get("title")?.as_str()?.to_string(),
            reason: v.get("reason")?.as_str()?.to_string(),
        })
    }
}

/// One entry of the version history for the memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditVersion {
    pub version: u32,
    pub change_type: String,
    pub by: String,
    pub at: String,
    pub reason: Option<String>,
}

impl From<&AutomaticCommit> for AuditVersion {
    fn from(c: &AutomaticCommit) -> Self {
        Self {
            version: c.version_number,
            change_type: change_type_str(&c.change_type).to_string(),
            by: c.created_by.clone(),
            at: c.created_at.to_rfc3339(),
            reason: c.change_reason.clone(),
        }
    }
}

/// The full, reconstructable decision chain for one memory — the answer to
/// "why did we decide this, who confirmed it, and what replaced it".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditTrail {
    pub memory_id: String,
    pub title: String,
    pub state: String,
    pub author: String,
    pub created_at: String,
    pub updated_at: String,
    /// Why this memory exists (the decision context, when recorded).
    pub reason: Option<String>,
    pub confirmed_by: Option<String>,
    pub confirmed_at: Option<String>,
    /// The memory this one replaced.
    pub supersedes: Option<String>,
    /// The memory that replaced this one.
    pub superseded_by: Option<String>,
    pub alternatives: Vec<DecisionAlternative>,
    pub events: Vec<AuditEvent>,
    pub versions: Vec<AuditVersion>,
}

/// Build the audit trail for one memory from its record, its decision-journal
/// events and its version history.
///
/// Pure function (unit-testable without a database). Events are returned
/// chronological; alternatives are extracted from `Alternative` events; the
/// final memory state / confirmation / supersession comes from the record.
pub fn build_audit_trail(
    record: &MemoryRecord,
    events: Vec<AuditEvent>,
    versions: Vec<AutomaticCommit>,
) -> AuditTrail {
    let mut alternatives = Vec::new();
    let mut event_log = Vec::new();
    for ev in events {
        match ev.event_type {
            AuditEventType::Alternative => {
                if let Some(detail) = &ev.detail
                    && let Some(alt) = DecisionAlternative::from_detail_json(detail)
                {
                    alternatives.push(alt);
                    continue; // alternatives live in their own list, not the raw log
                }
                event_log.push(ev);
            }
            _ => event_log.push(ev),
        }
    }
    event_log.sort_by_key(|a| a.created_at);

    let mut version_log: Vec<AuditVersion> = versions.iter().map(AuditVersion::from).collect();
    version_log.sort_by_key(|a| a.version);

    AuditTrail {
        memory_id: record.id.as_str().to_string(),
        title: record.title.clone(),
        state: record.memory_state.as_str().to_string(),
        author: record.author.clone(),
        created_at: record.created_at.to_rfc3339(),
        updated_at: record.updated_at.to_rfc3339(),
        reason: record.reason.clone(),
        confirmed_by: record.confirmed_by.clone(),
        confirmed_at: record.confirmed_at.map(|dt| dt.to_rfc3339()),
        supersedes: record.supersedes_id.clone(),
        superseded_by: record.superseded_by_id.clone(),
        alternatives,
        events: event_log,
        versions: version_log,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::types::{MemorySource, MemoryState};

    fn record(title: &str, author: &str) -> MemoryRecord {
        MemoryRecord::new(
            title.to_string(),
            "content".to_string(),
            author.to_string(),
            MemorySource::Manual,
        )
        .unwrap()
    }

    fn event(
        memory_id: EntityId,
        event_type: AuditEventType,
        actor: &str,
        detail: Option<&str>,
    ) -> AuditEvent {
        AuditEvent::new(
            memory_id,
            event_type,
            actor.to_string(),
            detail.map(|s| s.to_string()),
            None,
        )
    }

    fn commit(
        version: u32,
        change_type: crate::core::versioning::automatic_commit::ChangeType,
        by: &str,
        reason: Option<&str>,
    ) -> AutomaticCommit {
        AutomaticCommit {
            id: format!("c{}", version),
            hash: format!("h{}", version),
            version_number: version,
            entity_type: "MemoryRecord".to_string(),
            entity_id: EntityId::new(),
            change_type,
            diff: None,
            baseline_snapshot_id: None,
            is_baseline: version == 1,
            created_at: Utc::now(),
            created_by: by.to_string(),
            triggering_event_type: "EntityCreated".to_string(),
            triggering_event_id: format!("evt-{}", version),
            change_reason: reason.map(|s| s.to_string()),
            linked_entity_ids: Vec::new(),
            linked_decision_ids: Vec::new(),
            is_indexed: false,
            is_archived: false,
            size_bytes: 0,
        }
    }

    #[test]
    fn event_type_roundtrip() {
        for t in [
            AuditEventType::Created,
            AuditEventType::Alternative,
            AuditEventType::Confirmed,
            AuditEventType::Superseded,
            AuditEventType::Note,
        ] {
            assert_eq!(AuditEventType::parse(t.as_str()), t);
        }
        assert_eq!(AuditEventType::parse("bogus"), AuditEventType::Created);
    }

    #[test]
    fn alternative_json_roundtrip() {
        let alt = DecisionAlternative {
            title: "MySQL".to_string(),
            reason: "License costs".to_string(),
        };
        let json = alt.to_detail_json();
        let parsed = DecisionAlternative::from_detail_json(&json).unwrap();
        assert_eq!(parsed, alt);
        assert!(DecisionAlternative::from_detail_json("not json").is_none());
    }

    #[test]
    fn trail_includes_record_fields() {
        let mut r = record("PostgreSQL choice", "alice");
        r.reason = Some("scale-out for Q3".to_string());
        r.confirmed_by = Some("bob".to_string());
        r.memory_state = MemoryState::UserConfirmed;
        let trail = build_audit_trail(&r, vec![], vec![]);
        assert_eq!(trail.title, "PostgreSQL choice");
        assert_eq!(trail.reason.as_deref(), Some("scale-out for Q3"));
        assert_eq!(trail.confirmed_by.as_deref(), Some("bob"));
        assert_eq!(trail.state, "UserConfirmed");
        assert!(trail.events.is_empty());
        assert!(trail.alternatives.is_empty());
    }

    #[test]
    fn alternatives_extracted_from_events() {
        let r = record("PostgreSQL choice", "alice");
        let events = vec![
            event(
                r.id.clone(),
                AuditEventType::Alternative,
                "alice",
                Some(
                    &DecisionAlternative {
                        title: "MySQL".to_string(),
                        reason: "license costs".to_string(),
                    }
                    .to_detail_json(),
                ),
            ),
            event(
                r.id.clone(),
                AuditEventType::Alternative,
                "alice",
                Some(
                    &DecisionAlternative {
                        title: "SQLite".to_string(),
                        reason: "concurrency ceiling".to_string(),
                    }
                    .to_detail_json(),
                ),
            ),
            event(r.id.clone(), AuditEventType::Confirmed, "bob", None),
        ];
        let trail = build_audit_trail(&r, events, vec![]);
        assert_eq!(trail.alternatives.len(), 2);
        assert_eq!(trail.alternatives[0].title, "MySQL");
        assert_eq!(trail.alternatives[1].reason, "concurrency ceiling");
        // The confirmation stays in the raw log; alternatives do not.
        assert_eq!(trail.events.len(), 1);
        assert_eq!(trail.events[0].event_type, AuditEventType::Confirmed);
    }

    #[test]
    fn versions_ordered_by_version_number() {
        let r = record("PostgreSQL choice", "alice");
        let versions = vec![
            commit(
                2,
                crate::core::versioning::automatic_commit::ChangeType::Modified,
                "carol",
                Some("tighten wording"),
            ),
            commit(
                1,
                crate::core::versioning::automatic_commit::ChangeType::Created,
                "alice",
                None,
            ),
        ];
        let trail = build_audit_trail(&r, vec![], versions);
        assert_eq!(trail.versions.len(), 2);
        assert_eq!(trail.versions[0].version, 1);
        assert_eq!(trail.versions[0].by, "alice");
        assert_eq!(trail.versions[1].version, 2);
        assert_eq!(trail.versions[1].reason.as_deref(), Some("tighten wording"));
    }

    #[test]
    fn trail_reports_supersession_chain() {
        let mut r = record("PostgreSQL choice", "alice");
        r.supersedes_id = Some("mem-old".to_string());
        r.superseded_by_id = Some("mem-cockroach".to_string());
        let trail = build_audit_trail(&r, vec![], vec![]);
        assert_eq!(trail.supersedes.as_deref(), Some("mem-old"));
        assert_eq!(trail.superseded_by.as_deref(), Some("mem-cockroach"));
    }

    #[test]
    fn events_chronological_regardless_of_input_order() {
        let r = record("PostgreSQL choice", "alice");
        let mut later = event(
            r.id.clone(),
            AuditEventType::Note,
            "bob",
            Some("revisit in Q4"),
        );
        later.created_at = Utc::now() + chrono::Duration::hours(2);
        let mut earlier = event(r.id.clone(), AuditEventType::Confirmed, "alice", None);
        earlier.created_at = Utc::now();
        let trail = build_audit_trail(&r, vec![later, earlier], vec![]);
        assert_eq!(trail.events.len(), 2);
        assert_eq!(trail.events[0].event_type, AuditEventType::Confirmed);
        assert_eq!(trail.events[1].event_type, AuditEventType::Note);
    }
}
