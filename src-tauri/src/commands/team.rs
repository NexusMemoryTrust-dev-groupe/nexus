use chrono::Utc;
use serde::Serialize;

use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::team::{TeamMember, TeamRepository, TeamRole, build_team_overview};

/// Serializable team member for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamMemberDto {
    pub id: String,
    pub name: String,
    pub role: String,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Per-member activity inside the trusted layer (Tauri IPC).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberActivityDto {
    pub member: TeamMemberDto,
    pub authored: u64,
    pub confirmed: u64,
    pub updated: u64,
}

/// A single item in the trusted decision layer (Tauri IPC).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionItemDto {
    pub memory_id: String,
    pub title: String,
    pub by: Option<String>,
    pub at: Option<String>,
    pub detail: Option<String>,
}

/// Aggregate totals for the trusted layer (Tauri IPC).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamTotalsDto {
    pub members: u64,
    pub active: u64,
    pub confirmed: u64,
    pub superseded: u64,
    pub conflicted: u64,
    pub authored: u64,
}

/// The trusted decision layer of the team (Tauri IPC).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamOverviewDto {
    pub members: Vec<MemberActivityDto>,
    pub confirmed_decisions: Vec<DecisionItemDto>,
    pub superseded_decisions: Vec<DecisionItemDto>,
    pub conflicted: Vec<DecisionItemDto>,
    pub totals: TeamTotalsDto,
}

impl From<&TeamMember> for TeamMemberDto {
    fn from(m: &TeamMember) -> Self {
        Self {
            id: m.id.as_str().to_string(),
            name: m.name.clone(),
            role: m.role.as_str().to_string(),
            active: m.active,
            created_at: m.created_at.to_rfc3339(),
            updated_at: m.updated_at.to_rfc3339(),
        }
    }
}

impl From<&crate::core::team::TeamOverview> for TeamOverviewDto {
    fn from(o: &crate::core::team::TeamOverview) -> Self {
        Self {
            members: o
                .members
                .iter()
                .map(|a| MemberActivityDto {
                    member: TeamMemberDto::from(&a.member),
                    authored: a.authored,
                    confirmed: a.confirmed,
                    updated: a.updated,
                })
                .collect(),
            confirmed_decisions: o
                .confirmed_decisions
                .iter()
                .map(|d| DecisionItemDto {
                    memory_id: d.memory_id.clone(),
                    title: d.title.clone(),
                    by: d.by.clone(),
                    at: d.at.clone(),
                    detail: d.detail.clone(),
                })
                .collect(),
            superseded_decisions: o
                .superseded_decisions
                .iter()
                .map(|d| DecisionItemDto {
                    memory_id: d.memory_id.clone(),
                    title: d.title.clone(),
                    by: d.by.clone(),
                    at: d.at.clone(),
                    detail: d.detail.clone(),
                })
                .collect(),
            conflicted: o
                .conflicted
                .iter()
                .map(|d| DecisionItemDto {
                    memory_id: d.memory_id.clone(),
                    title: d.title.clone(),
                    by: d.by.clone(),
                    at: d.at.clone(),
                    detail: d.detail.clone(),
                })
                .collect(),
            totals: TeamTotalsDto {
                members: o.totals.members,
                active: o.totals.active,
                confirmed: o.totals.confirmed,
                superseded: o.totals.superseded,
                conflicted: o.totals.conflicted,
                authored: o.totals.authored,
            },
        }
    }
}

fn open_team_repo() -> Result<crate::storage::sqlite::SqliteTeamRepository, String> {
    let conn = crate::db::open_connection()?;
    crate::storage::sqlite::SqliteTeamRepository::new(conn).map_err(|e| e.to_string())
}

fn open_memory_repo() -> Result<crate::storage::sqlite::SqliteMemoryRepository, String> {
    let conn = crate::db::open_connection()?;
    crate::storage::sqlite::SqliteMemoryRepository::new(conn).map_err(|e| e.to_string())
}

/// Add a new team member to the roster.
#[tauri::command]
pub async fn team_add_member(name: String, role: Option<String>) -> Result<TeamMemberDto, String> {
    let role = TeamRole::parse(role.as_deref().unwrap_or("member"));
    let member = TeamMember::new(name, role);
    let repo = open_team_repo()?;
    repo.add_member(&member).await.map_err(|e| e.to_string())?;
    Ok(TeamMemberDto::from(&member))
}

/// List all team members.
#[tauri::command]
pub async fn team_list_members() -> Result<Vec<TeamMemberDto>, String> {
    let repo = open_team_repo()?;
    let members = repo.list_members().await.map_err(|e| e.to_string())?;
    Ok(members.iter().map(TeamMemberDto::from).collect())
}

/// Update a team member's role and/or active flag.
#[tauri::command]
pub async fn team_update_member(
    id: String,
    role: Option<String>,
    active: Option<bool>,
) -> Result<TeamMemberDto, String> {
    let member_id = crate::core::entity_id::EntityId::parse(&id).map_err(|e| e.to_string())?;
    let repo = open_team_repo()?;
    let mut member = repo
        .get_member(&member_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Team member {} not found", id))?;
    if let Some(role) = role {
        member.role = TeamRole::parse(&role);
    }
    if let Some(active) = active {
        member.active = active;
    }
    member.updated_at = Utc::now();
    repo.update_member(&member)
        .await
        .map_err(|e| e.to_string())?;
    Ok(TeamMemberDto::from(&member))
}

/// Remove a team member from the roster.
#[tauri::command]
pub async fn team_remove_member(id: String) -> Result<(), String> {
    let member_id = crate::core::entity_id::EntityId::parse(&id).map_err(|e| e.to_string())?;
    let repo = open_team_repo()?;
    repo.remove_member(&member_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Build the trusted decision layer: who confirmed what, what went stale,
/// what is in conflict, and per-member activity.
#[tauri::command]
pub async fn get_team_overview() -> Result<TeamOverviewDto, String> {
    let team_repo = open_team_repo()?;
    let members = team_repo.list_members().await.map_err(|e| e.to_string())?;

    let memory_repo = open_memory_repo()?;
    let records = memory_repo
        .list(100_000, 0)
        .await
        .map_err(|e| e.to_string())?;

    let overview = build_team_overview(members, &records);
    Ok(TeamOverviewDto::from(&overview))
}
