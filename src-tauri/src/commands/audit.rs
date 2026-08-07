use serde::Serialize;

use crate::core::audit::{
    AuditEvent, AuditEventType, AuditRepository, AuditTrail, AuditVersion, DecisionAlternative,
    build_audit_trail,
};
use crate::core::entity_id::EntityId;
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::versioning::commit_service::CommitService;

/// One row of the decision journal (Tauri IPC).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEventDto {
    pub id: String,
    pub memory_id: String,
    pub event_type: String,
    pub actor: String,
    pub detail: Option<String>,
    pub related_memory_id: Option<String>,
    pub created_at: String,
}

/// An alternative that was considered (Tauri IPC).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionAlternativeDto {
    pub title: String,
    pub reason: String,
}

/// One version-history entry (Tauri IPC).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditVersionDto {
    pub version: u32,
    pub change_type: String,
    pub by: String,
    pub at: String,
    pub reason: Option<String>,
}

/// The full reconstructable decision chain for one memory (Tauri IPC).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditTrailDto {
    pub memory_id: String,
    pub title: String,
    pub state: String,
    pub author: String,
    pub created_at: String,
    pub updated_at: String,
    pub reason: Option<String>,
    pub confirmed_by: Option<String>,
    pub confirmed_at: Option<String>,
    pub supersedes: Option<String>,
    pub superseded_by: Option<String>,
    pub alternatives: Vec<DecisionAlternativeDto>,
    pub events: Vec<AuditEventDto>,
    pub versions: Vec<AuditVersionDto>,
}

impl From<&AuditEvent> for AuditEventDto {
    fn from(e: &AuditEvent) -> Self {
        Self {
            id: e.id.as_str().to_string(),
            memory_id: e.memory_id.as_str().to_string(),
            event_type: e.event_type.as_str().to_string(),
            actor: e.actor.clone(),
            detail: e.detail.clone(),
            related_memory_id: e.related_memory_id.clone(),
            created_at: e.created_at.to_rfc3339(),
        }
    }
}

impl From<&AuditVersion> for AuditVersionDto {
    fn from(v: &AuditVersion) -> Self {
        Self {
            version: v.version,
            change_type: v.change_type.clone(),
            by: v.by.clone(),
            at: v.at.clone(),
            reason: v.reason.clone(),
        }
    }
}

impl From<&AuditTrail> for AuditTrailDto {
    fn from(t: &AuditTrail) -> Self {
        Self {
            memory_id: t.memory_id.clone(),
            title: t.title.clone(),
            state: t.state.clone(),
            author: t.author.clone(),
            created_at: t.created_at.clone(),
            updated_at: t.updated_at.clone(),
            reason: t.reason.clone(),
            confirmed_by: t.confirmed_by.clone(),
            confirmed_at: t.confirmed_at.clone(),
            supersedes: t.supersedes.clone(),
            superseded_by: t.superseded_by.clone(),
            alternatives: t
                .alternatives
                .iter()
                .map(|a| DecisionAlternativeDto {
                    title: a.title.clone(),
                    reason: a.reason.clone(),
                })
                .collect(),
            events: t.events.iter().map(AuditEventDto::from).collect(),
            versions: t.versions.iter().map(AuditVersionDto::from).collect(),
        }
    }
}

fn open_audit_repo() -> Result<crate::storage::sqlite::SqliteAuditRepository, String> {
    let conn = crate::db::open_connection()?;
    crate::storage::sqlite::SqliteAuditRepository::new(conn).map_err(|e| e.to_string())
}

fn open_memory_repo() -> Result<crate::storage::sqlite::SqliteMemoryRepository, String> {
    let conn = crate::db::open_connection()?;
    crate::storage::sqlite::SqliteMemoryRepository::new(conn).map_err(|e| e.to_string())
}

fn open_versioning_repo() -> Result<crate::storage::sqlite::SqliteVersioningRepository, String> {
    let conn = crate::db::open_connection()?;
    crate::storage::sqlite::SqliteVersioningRepository::new(conn).map_err(|e| e.to_string())
}

/// Reconstruct the full decision chain for one memory: why it exists, which
/// alternatives were considered, who confirmed it, and what replaced it.
///
/// The answer to "Why did we choose PostgreSQL in March?" — context,
/// alternatives, confirmation, supersession, and the version history.
#[tauri::command]
pub async fn get_audit_trail(memory_id: String) -> Result<AuditTrailDto, String> {
    let id = EntityId::parse(&memory_id).map_err(|e| e.to_string())?;

    let memory_repo = open_memory_repo()?;
    let record = memory_repo
        .get_by_id(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Memory {} not found", memory_id))?;

    let audit_repo = open_audit_repo()?;
    let events = audit_repo
        .list_events(&id)
        .await
        .map_err(|e| e.to_string())?;

    let versioning_repo = open_versioning_repo()?;
    let versions = versioning_repo
        .get_entity_history("MemoryRecord", &id)
        .await
        .map_err(|e| e.to_string())?;

    let trail = build_audit_trail(&record, events, versions);
    Ok(AuditTrailDto::from(&trail))
}

/// Append a raw event to the decision journal (Created/Confirmed/Superseded/Note).
#[tauri::command]
pub async fn audit_add_event(
    memory_id: String,
    event_type: String,
    actor: String,
    detail: Option<String>,
    related_memory_id: Option<String>,
) -> Result<AuditEventDto, String> {
    let id = EntityId::parse(&memory_id).map_err(|e| e.to_string())?;
    let memory_repo = open_memory_repo()?;
    if memory_repo
        .get_by_id(&id)
        .await
        .map_err(|e| e.to_string())?
        .is_none()
    {
        return Err(format!(
            "Memory {} not found — cannot record an audit event for it",
            memory_id
        ));
    }
    let event = AuditEvent::new(
        id,
        AuditEventType::parse(&event_type),
        actor,
        detail,
        related_memory_id,
    );
    let repo = open_audit_repo()?;
    repo.add_event(&event).await.map_err(|e| e.to_string())?;
    Ok(AuditEventDto::from(&event))
}

/// Record that an alternative was considered for a decision (and rejected).
/// Appends an `Alternative` event whose detail is `{ title, reason }` JSON.
#[tauri::command]
pub async fn audit_alternative(
    memory_id: String,
    title: String,
    reason: String,
    actor: String,
) -> Result<AuditEventDto, String> {
    if title.trim().is_empty() {
        return Err("Alternative title must not be empty".to_string());
    }
    let id = EntityId::parse(&memory_id).map_err(|e| e.to_string())?;
    let memory_repo = open_memory_repo()?;
    if memory_repo
        .get_by_id(&id)
        .await
        .map_err(|e| e.to_string())?
        .is_none()
    {
        return Err(format!(
            "Memory {} not found — cannot record an alternative for it",
            memory_id
        ));
    }
    let alt = DecisionAlternative { title, reason };
    let event = AuditEvent::new(
        id,
        AuditEventType::Alternative,
        actor,
        Some(alt.to_detail_json()),
        None,
    );
    let repo = open_audit_repo()?;
    repo.add_event(&event).await.map_err(|e| e.to_string())?;
    Ok(AuditEventDto::from(&event))
}
