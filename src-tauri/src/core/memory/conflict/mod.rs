//! Conflict groups — Memory Conflict Engine (Система 2).
//!
//! The conflict detector (V12) marks both sides of a semantic contradiction
//! `Conflicted`. A conflict group ties those records together into one
//! resolvable unit: participants, status (open/resolved), and — once the
//! Current Truth Engine picks a winner or a human answers "which one is
//! correct?" — the resolution itself (winner, confidence, human-readable
//! reasons, who decided and when).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity_id::EntityId;

pub mod conflict_repository;
pub mod conflict_service;
pub mod engine;
pub mod truth;
pub mod verdict;

pub use conflict_repository::ConflictRepository;
pub use conflict_service::ConflictService;
pub use verdict::{PairVerdict, classify};

/// Lifecycle of a conflict group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictStatus {
    /// Detected but not yet resolved — the trust UI / MCP surfaces it and, when
    /// the engine is unsure, asks the user "Which one is correct?".
    Open,
    /// A winner was chosen (by the engine or by a human) and the losers were
    /// marked `Superseded`.
    Resolved,
}

impl ConflictStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConflictStatus::Open => "open",
            ConflictStatus::Resolved => "resolved",
        }
    }

    pub fn parse(s: &str) -> ConflictStatus {
        match s {
            "resolved" => ConflictStatus::Resolved,
            _ => ConflictStatus::Open,
        }
    }
}

/// The verdict of the Current Truth Engine for one conflict: which memory wins
/// right now, with how much confidence, and why (human-readable reasons).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruthVerdict {
    pub winner_id: EntityId,
    /// Normalized 0.0–1.0 — how much more plausible the winner is than the
    /// closest competitor.
    pub confidence: f64,
    /// Human-readable reasons, e.g. "+ recent source", "+ user confirmation".
    pub reasons: Vec<String>,
}

/// The stored outcome of a resolved conflict.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConflictResolution {
    /// The memory that won (now `Current` or `UserConfirmed`).
    pub winner_id: EntityId,
    pub confidence: f64,
    pub reasons: Vec<String>,
    /// Who decided: "user" or "engine".
    pub by: String,
    pub at: DateTime<Utc>,
}

/// A conflict group — one resolvable semantic contradiction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConflictGroup {
    pub id: EntityId,
    /// Normalized topic used to group related contradictions.
    pub topic: String,
    /// Ids of the memory records participating in the conflict.
    pub member_ids: Vec<EntityId>,
    pub detected_at: DateTime<Utc>,
    pub status: ConflictStatus,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolution: Option<ConflictResolution>,
}

impl ConflictGroup {
    pub fn new(topic: String, member_ids: Vec<EntityId>) -> Self {
        Self {
            id: EntityId::new(),
            topic,
            member_ids,
            detected_at: Utc::now(),
            status: ConflictStatus::Open,
            resolved_at: None,
            resolution: None,
        }
    }

    /// True when the given memory participates in this group.
    pub fn contains(&self, memory_id: &EntityId) -> bool {
        self.member_ids.contains(memory_id)
    }

    /// Serialize member ids to the JSON form stored in `conflict_groups.member_ids`.
    pub fn member_ids_to_json(&self) -> String {
        let ids: Vec<String> = self
            .member_ids
            .iter()
            .map(|id| id.as_str().to_string())
            .collect();
        serde_json::to_string(&ids).unwrap_or_else(|_| "[]".to_string())
    }

    /// Parse member ids from the JSON form stored in the database.
    pub fn member_ids_from_json(json: &str) -> Vec<EntityId> {
        let ids: Vec<String> = serde_json::from_str(json).unwrap_or_default();
        ids.iter().filter_map(|s| EntityId::parse(s).ok()).collect()
    }

    /// Serialize the resolution to the JSON form stored in `conflict_groups.resolution`.
    pub fn resolution_to_json(resolution: &ConflictResolution) -> String {
        serde_json::to_string(resolution).unwrap_or_else(|_| "{}".to_string())
    }

    /// Parse the resolution from the JSON form stored in the database.
    pub fn resolution_from_json(json: &str) -> Option<ConflictResolution> {
        serde_json::from_str(json).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_status_roundtrip() {
        assert_eq!(ConflictStatus::parse("open"), ConflictStatus::Open);
        assert_eq!(ConflictStatus::parse("resolved"), ConflictStatus::Resolved);
        assert_eq!(ConflictStatus::parse("bogus"), ConflictStatus::Open);
        assert_eq!(ConflictStatus::Open.as_str(), "open");
        assert_eq!(ConflictStatus::Resolved.as_str(), "resolved");
    }

    #[test]
    fn group_new_defaults() {
        let g = ConflictGroup::new("database".to_string(), vec![]);
        assert_eq!(g.status, ConflictStatus::Open);
        assert!(g.resolved_at.is_none());
        assert!(g.resolution.is_none());
        assert!(g.member_ids.is_empty());
    }

    #[test]
    fn group_contains_member() {
        let m1 = EntityId::new();
        let m2 = EntityId::new();
        let g = ConflictGroup::new("db".to_string(), vec![m1.clone()]);
        assert!(g.contains(&m1));
        assert!(!g.contains(&m2));
    }

    #[test]
    fn member_ids_json_roundtrip() {
        let ids = vec![EntityId::new(), EntityId::new()];
        let g = ConflictGroup::new("db".to_string(), ids.clone());
        let json = g.member_ids_to_json();
        let parsed = ConflictGroup::member_ids_from_json(&json);
        assert_eq!(parsed, ids);
        // Invalid JSON falls back to empty list, never panics.
        assert!(ConflictGroup::member_ids_from_json("not-json").is_empty());
    }

    #[test]
    fn resolution_json_roundtrip() {
        let res = ConflictResolution {
            winner_id: EntityId::new(),
            confidence: 0.94,
            reasons: vec![
                "+ recent source".to_string(),
                "+ user confirmation".to_string(),
            ],
            by: "engine".to_string(),
            at: Utc::now(),
        };
        let json = ConflictGroup::resolution_to_json(&res);
        let parsed = ConflictGroup::resolution_from_json(&json).unwrap();
        assert_eq!(parsed, res);
        assert!(ConflictGroup::resolution_from_json("not-json").is_none());
    }
}
