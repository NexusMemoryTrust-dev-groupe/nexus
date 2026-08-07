use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity_id::EntityId;
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::types::MemoryState;

pub mod team_repository;

pub use team_repository::TeamRepository;

// ── Roles ────────────────────────────────────────────────────────────────────

/// Team role — the level of trust a member has over shared memory.
///
/// `Admin` runs the team (add/remove members, change roles).
/// `Member` participates: creates memories, confirms decisions, gives feedback.
/// `Viewer` can read the shared layer but cannot change anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamRole {
    Admin,
    Member,
    Viewer,
}

impl TeamRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            TeamRole::Admin => "admin",
            TeamRole::Member => "member",
            TeamRole::Viewer => "viewer",
        }
    }

    pub fn parse(s: &str) -> TeamRole {
        match s {
            "admin" => TeamRole::Admin,
            "viewer" => TeamRole::Viewer,
            _ => TeamRole::Member,
        }
    }

    /// Whether this role may perform the given action.
    pub fn can(&self, action: TeamAction) -> bool {
        match self {
            TeamRole::Admin => true,
            TeamRole::Member => !matches!(action, TeamAction::ManageMembers),
            TeamRole::Viewer => matches!(action, TeamAction::View),
        }
    }
}

/// An action a team member may (or may not) perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamAction {
    /// Manage the roster: add/remove members, change roles (admin only).
    ManageMembers,
    /// Create a new memory record (member+).
    CreateMemory,
    /// Confirm a memory as a decision (member+).
    ConfirmMemory,
    /// Supersede a memory with a newer decision (member+).
    SupersedeMemory,
    /// Give useful/irrelevant/wrong feedback (member+).
    FeedbackMemory,
    /// Read the shared memory layer (everyone).
    View,
}

impl TeamAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            TeamAction::ManageMembers => "manage_members",
            TeamAction::CreateMemory => "create_memory",
            TeamAction::ConfirmMemory => "confirm_memory",
            TeamAction::SupersedeMemory => "supersede_memory",
            TeamAction::FeedbackMemory => "feedback_memory",
            TeamAction::View => "view",
        }
    }

    pub fn parse(s: &str) -> TeamAction {
        match s {
            "manage_members" => TeamAction::ManageMembers,
            "create_memory" => TeamAction::CreateMemory,
            "confirm_memory" => TeamAction::ConfirmMemory,
            "supersede_memory" => TeamAction::SupersedeMemory,
            "feedback_memory" => TeamAction::FeedbackMemory,
            _ => TeamAction::View,
        }
    }
}

// ── Member ───────────────────────────────────────────────────────────────────

/// A team member who shares the trusted memory layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub id: EntityId,
    pub name: String,
    pub role: TeamRole,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TeamMember {
    pub fn new(name: String, role: TeamRole) -> Self {
        let now = Utc::now();
        Self {
            id: EntityId::new(),
            name,
            role,
            active: true,
            created_at: now,
            updated_at: now,
        }
    }
}

// ── Trusted decision layer (Team Overview) ───────────────────────────────────

/// Per-member activity inside the trusted layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberActivity {
    pub member: TeamMember,
    /// Memories authored by this member.
    pub authored: u64,
    /// Memories confirmed by this member (confirmed_by matches their name).
    pub confirmed: u64,
    /// Memories last updated by this member (updated_by matches their name).
    pub updated: u64,
}

/// A single item in the trusted decision layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionItem {
    pub memory_id: String,
    pub title: String,
    /// Who performed the decision (member name, when known).
    pub by: Option<String>,
    /// When it happened (RFC3339, when known).
    pub at: Option<String>,
    /// Extra context — e.g. which memory replaced this one.
    pub detail: Option<String>,
}

/// Aggregate totals for the trusted layer.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamTotals {
    pub members: u64,
    pub active: u64,
    pub confirmed: u64,
    pub superseded: u64,
    pub conflicted: u64,
    pub authored: u64,
}

/// The trusted decision layer of the team: who confirmed what, what went stale,
/// what is in conflict — the answers teams can't get from chat history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamOverview {
    pub members: Vec<MemberActivity>,
    pub confirmed_decisions: Vec<DecisionItem>,
    pub superseded_decisions: Vec<DecisionItem>,
    pub conflicted: Vec<DecisionItem>,
    pub totals: TeamTotals,
}

/// Build the trusted decision layer from the member roster and the memory pool.
///
/// Pure function (unit-testable without a database): given the members and all
/// memory records it produces the overview. Members are matched to memories by
/// name (author / confirmed_by / updated_by are free-text on MemoryRecord).
pub fn build_team_overview(members: Vec<TeamMember>, records: &[MemoryRecord]) -> TeamOverview {
    let mut confirmed: Vec<DecisionItem> = Vec::new();
    let mut superseded: Vec<DecisionItem> = Vec::new();
    let mut conflicted: Vec<DecisionItem> = Vec::new();

    for r in records {
        match r.memory_state {
            MemoryState::UserConfirmed => confirmed.push(DecisionItem {
                memory_id: r.id.as_str().to_string(),
                title: r.title.clone(),
                by: r.confirmed_by.clone().or(Some(r.author.clone())),
                at: r
                    .confirmed_at
                    .map(|dt| dt.to_rfc3339())
                    .or(Some(r.updated_at.to_rfc3339())),
                detail: None,
            }),
            MemoryState::Superseded => {
                let detail = Some(
                    r.superseded_by_id
                        .as_ref()
                        .map(|id| format!("replaced by {}", id))
                        .unwrap_or_else(|| "superseded".to_string()),
                );
                superseded.push(DecisionItem {
                    memory_id: r.id.as_str().to_string(),
                    title: r.title.clone(),
                    by: r.updated_by.clone().or(Some(r.author.clone())),
                    at: Some(r.updated_at.to_rfc3339()),
                    detail,
                });
            }
            MemoryState::Conflicted => conflicted.push(DecisionItem {
                memory_id: r.id.as_str().to_string(),
                title: r.title.clone(),
                by: r.updated_by.clone().or(Some(r.author.clone())),
                at: Some(r.updated_at.to_rfc3339()),
                detail: None,
            }),
            _ => {}
        }
    }

    // Activity per member, matched by name.
    let mut activity: Vec<MemberActivity> = Vec::with_capacity(members.len());
    for m in &members {
        let mut authored = 0u64;
        let mut confirmed = 0u64;
        let mut updated = 0u64;
        for r in records {
            if r.author == m.name {
                authored += 1;
            }
            if r.confirmed_by.as_deref() == Some(m.name.as_str()) {
                confirmed += 1;
            }
            if r.updated_by.as_deref() == Some(m.name.as_str()) {
                updated += 1;
            }
        }
        activity.push(MemberActivity {
            member: m.clone(),
            authored,
            confirmed,
            updated,
        });
    }

    let totals = TeamTotals {
        members: members.len() as u64,
        active: members.iter().filter(|m| m.active).count() as u64,
        confirmed: confirmed.len() as u64,
        superseded: superseded.len() as u64,
        conflicted: conflicted.len() as u64,
        authored: records.iter().filter(|r| r.author != "user").count() as u64,
    };

    TeamOverview {
        members: activity,
        confirmed_decisions: confirmed,
        superseded_decisions: superseded,
        conflicted,
        totals,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::memory_record::MemoryRecord;
    use crate::core::memory::types::MemorySource;

    fn member(name: &str, role: TeamRole) -> TeamMember {
        TeamMember::new(name.to_string(), role)
    }

    fn record(title: &str, author: &str, state: MemoryState) -> MemoryRecord {
        let mut r = MemoryRecord::new(
            title.to_string(),
            "content".to_string(),
            author.to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        r.memory_state = state;
        r
    }

    #[test]
    fn role_as_str_roundtrip() {
        for role in [TeamRole::Admin, TeamRole::Member, TeamRole::Viewer] {
            assert_eq!(TeamRole::parse(role.as_str()), role);
        }
    }

    #[test]
    fn role_from_str_unknown_defaults_to_member() {
        assert_eq!(TeamRole::parse("boss"), TeamRole::Member);
    }

    #[test]
    fn admin_can_everything() {
        for action in [
            TeamAction::ManageMembers,
            TeamAction::CreateMemory,
            TeamAction::ConfirmMemory,
            TeamAction::SupersedeMemory,
            TeamAction::FeedbackMemory,
            TeamAction::View,
        ] {
            assert!(TeamRole::Admin.can(action), "admin should do {:?}", action);
        }
    }

    #[test]
    fn member_cannot_manage_roster() {
        assert!(!TeamRole::Member.can(TeamAction::ManageMembers));
        assert!(TeamRole::Member.can(TeamAction::ConfirmMemory));
        assert!(TeamRole::Member.can(TeamAction::CreateMemory));
    }

    #[test]
    fn viewer_only_reads() {
        assert!(TeamRole::Viewer.can(TeamAction::View));
        assert!(!TeamRole::Viewer.can(TeamAction::ConfirmMemory));
        assert!(!TeamRole::Viewer.can(TeamAction::FeedbackMemory));
        assert!(!TeamRole::Viewer.can(TeamAction::ManageMembers));
    }

    #[test]
    fn action_as_str_roundtrip() {
        for a in [
            TeamAction::ManageMembers,
            TeamAction::CreateMemory,
            TeamAction::ConfirmMemory,
            TeamAction::SupersedeMemory,
            TeamAction::FeedbackMemory,
            TeamAction::View,
        ] {
            assert_eq!(TeamAction::parse(a.as_str()), a);
        }
    }

    #[test]
    fn member_new_defaults() {
        let m = member("Alice", TeamRole::Admin);
        assert!(m.active);
        assert_eq!(m.name, "Alice");
        assert_eq!(m.role, TeamRole::Admin);
        assert!(!m.id.as_str().is_empty());
    }

    #[test]
    fn overview_counts_states() {
        let members = vec![
            member("Alice", TeamRole::Admin),
            member("Bob", TeamRole::Member),
        ];
        let records = vec![
            record("Decision A", "Alice", MemoryState::UserConfirmed),
            record("Decision B", "Alice", MemoryState::Superseded),
            record("Decision C", "Bob", MemoryState::Conflicted),
            record("Plain", "user", MemoryState::Current),
        ];
        let overview = build_team_overview(members, &records);
        assert_eq!(overview.totals.confirmed, 1);
        assert_eq!(overview.totals.superseded, 1);
        assert_eq!(overview.totals.conflicted, 1);
        assert_eq!(overview.confirmed_decisions.len(), 1);
        assert_eq!(overview.confirmed_decisions[0].by.as_deref(), Some("Alice"));
    }

    #[test]
    fn overview_activity_per_member() {
        let members = vec![
            member("Alice", TeamRole::Admin),
            member("Bob", TeamRole::Member),
        ];
        let records = vec![
            record("A1", "Alice", MemoryState::Current),
            record("A2", "Alice", MemoryState::Current),
            record("B1", "Bob", MemoryState::Current),
        ];
        let overview = build_team_overview(members, &records);
        let alice = overview
            .members
            .iter()
            .find(|a| a.member.name == "Alice")
            .unwrap();
        let bob = overview
            .members
            .iter()
            .find(|a| a.member.name == "Bob")
            .unwrap();
        assert_eq!(alice.authored, 2);
        assert_eq!(bob.authored, 1);
        assert_eq!(overview.totals.authored, 3);
    }

    #[test]
    fn overview_empty_roster() {
        let overview = build_team_overview(vec![], &[]);
        assert_eq!(overview.totals.members, 0);
        assert!(overview.members.is_empty());
        assert!(overview.confirmed_decisions.is_empty());
    }

    #[test]
    fn overview_superseded_has_detail() {
        let members = vec![member("Alice", TeamRole::Admin)];
        let mut r = record("Old", "Alice", MemoryState::Superseded);
        r.superseded_by_id = Some(crate::core::entity_id::EntityId::new().as_str().to_string());
        let overview = build_team_overview(members, &[r]);
        let item = &overview.superseded_decisions[0];
        assert_eq!(item.title, "Old");
        assert!(item.detail.as_deref().unwrap().starts_with("replaced by"));
    }

    #[test]
    fn inactive_member_still_counts_activity() {
        let mut m = member("Carol", TeamRole::Member);
        m.active = false;
        let members = vec![m];
        let records = vec![record("C1", "Carol", MemoryState::Current)];
        let overview = build_team_overview(members, &records);
        assert_eq!(overview.totals.active, 0);
        assert_eq!(overview.totals.members, 1);
        assert_eq!(overview.members[0].authored, 1);
    }
}
