use serde::Serialize;
use std::sync::Arc;

use crate::core::entity_id::EntityId;
use crate::core::memory::conflict::{
    ConflictGroup, ConflictResolution, ConflictService, ConflictStatus, TruthVerdict,
};

/// Serializable conflict group for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictGroupDto {
    pub id: String,
    pub topic: String,
    pub member_ids: Vec<String>,
    pub detected_at: String,
    pub status: String,
    pub resolved_at: Option<String>,
    pub resolution: Option<ConflictResolutionDto>,
}

/// Serializable resolution for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolutionDto {
    pub winner_id: String,
    pub confidence: f64,
    pub reasons: Vec<String>,
    pub by: String,
    pub at: String,
}

/// Serializable engine verdict for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TruthVerdictDto {
    pub winner_id: String,
    pub confidence: f64,
    pub reasons: Vec<String>,
}

impl From<&ConflictGroup> for ConflictGroupDto {
    fn from(g: &ConflictGroup) -> Self {
        Self {
            id: g.id.as_str().to_string(),
            topic: g.topic.clone(),
            member_ids: g
                .member_ids
                .iter()
                .map(|id| id.as_str().to_string())
                .collect(),
            detected_at: g.detected_at.to_rfc3339(),
            status: g.status.as_str().to_string(),
            resolved_at: g.resolved_at.map(|dt| dt.to_rfc3339()),
            resolution: g.resolution.as_ref().map(|r| ConflictResolutionDto {
                winner_id: r.winner_id.as_str().to_string(),
                confidence: r.confidence,
                reasons: r.reasons.clone(),
                by: r.by.clone(),
                at: r.at.to_rfc3339(),
            }),
        }
    }
}

impl From<&TruthVerdict> for TruthVerdictDto {
    fn from(v: &TruthVerdict) -> Self {
        Self {
            winner_id: v.winner_id.as_str().to_string(),
            confidence: v.confidence,
            reasons: v.reasons.clone(),
        }
    }
}

fn open_service() -> Result<ConflictService, String> {
    let memory_conn = crate::db::open_connection()?;
    let memory: Arc<dyn crate::core::memory::memory_repository::MemoryRepository> = Arc::new(
        crate::storage::sqlite::SqliteMemoryRepository::new(memory_conn)
            .map_err(|e| e.to_string())?,
    );

    let conflict_conn = crate::db::open_connection()?;
    let conflict: Arc<dyn crate::core::memory::conflict::ConflictRepository> = Arc::new(
        crate::storage::sqlite::SqliteConflictRepository::new(conflict_conn)
            .map_err(|e| e.to_string())?,
    );

    let audit_conn = crate::db::open_connection()?;
    let audit: Arc<dyn crate::core::audit::AuditRepository> = Arc::new(
        crate::storage::sqlite::SqliteAuditRepository::new(audit_conn)
            .map_err(|e| e.to_string())?,
    );

    Ok(ConflictService::new(conflict, memory, audit))
}

/// List conflict groups, optionally filtered by status ("open" | "resolved").
#[tauri::command]
pub async fn get_conflicts(status: Option<String>) -> Result<Vec<ConflictGroupDto>, String> {
    let service = open_service()?;
    let parsed = status.as_deref().map(ConflictStatus::parse);
    let groups = service
        .get_conflicts(parsed)
        .await
        .map_err(|e| e.to_string())?;
    Ok(groups.iter().map(ConflictGroupDto::from).collect())
}

/// Get one conflict group by id (with its resolution, when resolved).
#[tauri::command]
pub async fn get_conflict(id: String) -> Result<ConflictGroupDto, String> {
    let service = open_service()?;
    let entity_id = EntityId::parse(&id).map_err(|e| e.to_string())?;
    let group = service
        .get_conflict(&entity_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ConflictGroupDto::from(&group))
}

/// Run the Current Truth Engine over a conflict group's members. Read-only:
/// returns the winner + confidence + reasons without resolving anything.
#[tauri::command]
pub async fn get_conflict_truth(id: String) -> Result<TruthVerdictDto, String> {
    let service = open_service()?;
    let entity_id = EntityId::parse(&id).map_err(|e| e.to_string())?;
    let verdict = service
        .get_conflict_truth(&entity_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(TruthVerdictDto::from(&verdict))
}

/// Resolve a conflict: winner becomes Current (engine) / UserConfirmed (user),
/// losers become Superseded, the group is marked resolved, and audit events
/// are written for every loser.
#[tauri::command]
pub async fn resolve_conflict(
    id: String,
    winner_id: String,
    by: String,
    reason: Option<String>,
) -> Result<ConflictResolutionDto, String> {
    let service = open_service()?;
    let group_id = EntityId::parse(&id).map_err(|e| e.to_string())?;
    let winner = EntityId::parse(&winner_id).map_err(|e| e.to_string())?;
    let resolution: ConflictResolution = service
        .resolve_conflict(&group_id, &winner, &by, reason.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    Ok(ConflictResolutionDto {
        winner_id: resolution.winner_id.as_str().to_string(),
        confidence: resolution.confidence,
        reasons: resolution.reasons,
        by: resolution.by,
        at: resolution.at.to_rfc3339(),
    })
}

/// Reconcile the group table with reality: cluster all Conflicted records into
/// open groups (reusing existing ones). Returns how many groups were
/// created/updated. Used after the detector flags new contradictions.
#[tauri::command]
pub async fn sync_conflict_groups() -> Result<u64, String> {
    let service = open_service()?;
    let touched = service
        .sync_conflict_groups()
        .await
        .map_err(|e| e.to_string())?;
    Ok(touched as u64)
}
