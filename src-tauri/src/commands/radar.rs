use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::core::memory::memory_radar::{RADAR_LAST_SEEN_KEY, build_snapshot};
use crate::core::memory::memory_repository::MemoryRepository;

/// Serializable radar item for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RadarItemDto {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub action: String,
    pub importance: f64,
    pub confidence: f64,
    pub memory_state: String,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: Option<String>,
    pub reason: String,
}

/// Serializable aggregate counters for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RadarCountsDto {
    pub total: u64,
    pub new_since_last_scan: u64,
    pub updated_since_last_scan: u64,
    pub conflicted: u64,
    pub superseded: u64,
    pub inferred: u64,
    pub expiring: u64,
    pub unconfirmed: u64,
}

/// Serializable radar snapshot for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RadarSnapshotDto {
    pub generated_at: String,
    pub since: Option<String>,
    pub counts: RadarCountsDto,
    pub items: Vec<RadarItemDto>,
    pub attention_score: u8,
}

impl From<&crate::core::memory::memory_radar::RadarSnapshot> for RadarSnapshotDto {
    fn from(s: &crate::core::memory::memory_radar::RadarSnapshot) -> Self {
        Self {
            generated_at: s.generated_at.to_rfc3339(),
            since: s.since.map(|dt| dt.to_rfc3339()),
            counts: RadarCountsDto {
                total: s.counts.total,
                new_since_last_scan: s.counts.new_since_last_scan,
                updated_since_last_scan: s.counts.updated_since_last_scan,
                conflicted: s.counts.conflicted,
                superseded: s.counts.superseded,
                inferred: s.counts.inferred,
                expiring: s.counts.expiring,
                unconfirmed: s.counts.unconfirmed,
            },
            items: s
                .items
                .iter()
                .map(|i| RadarItemDto {
                    id: i.id.clone(),
                    title: i.title.clone(),
                    summary: i.summary.clone(),
                    action: i.action.as_str().to_string(),
                    importance: i.importance,
                    confidence: i.confidence,
                    memory_state: i.memory_state.clone(),
                    created_at: i.created_at.to_rfc3339(),
                    updated_at: i.updated_at.to_rfc3339(),
                    expires_at: i.expires_at.map(|dt| dt.to_rfc3339()),
                    reason: i.reason.clone(),
                })
                .collect(),
            attention_score: s.attention_score,
        }
    }
}

fn open_repo() -> Result<crate::storage::sqlite::SqliteMemoryRepository, String> {
    let conn = crate::db::open_connection()?;
    crate::storage::sqlite::SqliteMemoryRepository::new(conn).map_err(|e| e.to_string())
}

/// Build the memory radar snapshot.
///
/// Reads the previous scan checkpoint from `configuration_kv`, scans the whole
/// pool, and returns what needs attention. Does NOT advance the checkpoint —
/// call `radar_mark_seen` after the user has seen it.
#[tauri::command]
pub async fn get_radar_snapshot() -> Result<RadarSnapshotDto, String> {
    let repo = open_repo()?;
    let records = repo.list(100_000, 0).await.map_err(|e| e.to_string())?;

    let since = crate::commands::config::get_config_sync(RADAR_LAST_SEEN_KEY.to_string())?
        .and_then(|v| DateTime::parse_from_rfc3339(&v).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let snapshot = build_snapshot(&records, since);
    Ok(RadarSnapshotDto::from(&snapshot))
}

/// Advance the radar checkpoint to "now".
///
/// Call this once the user has reviewed the current snapshot so the next scan
/// only reports what changed afterwards.
#[tauri::command]
pub async fn radar_mark_seen() -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    crate::commands::config::set_config(RADAR_LAST_SEEN_KEY.to_string(), now).await
}

/// Build the radar snapshot and immediately advance the checkpoint.
///
/// Convenience for callers (MCP / copilot) that want a scan-and-checkpoint in
/// one step.
#[tauri::command]
pub async fn radar_scan_and_seen() -> Result<RadarSnapshotDto, String> {
    let snapshot = get_radar_snapshot().await?;
    radar_mark_seen().await?;
    Ok(snapshot)
}
