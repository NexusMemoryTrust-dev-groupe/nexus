use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::graph::entity::Entity;
use crate::core::graph::relationship::Relationship;
use crate::core::memory::memory_record::MemoryRecord;

/// Time window for the context slice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalSlice {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

/// Classification of user intent behind a query.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IntentType {
    Search,
    Analysis,
    Decision,
    Creation,
    Update,
    Exploration,
}

/// Detected user intent with confidence score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserIntent {
    pub query: String,
    pub intent_type: IntentType,
    pub confidence: f64,
    pub keywords: Vec<String>,
    pub temporal: Option<String>,
}

/// A computed context package — the core output of the Context Engine.
/// Contains entities, relationships, memory records, scores, and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPackage {
    pub id: String,
    pub entities: Vec<Entity>,
    pub relationships: Vec<Relationship>,
    pub memory_records: Vec<MemoryRecord>,
    pub temporal_slice: TemporalSlice,
    pub relevance_scores: HashMap<String, f64>,
    pub user_intent: UserIntent,
    pub created_at: DateTime<Utc>,
    pub token_count: u32,
    pub compressed_size: u32,

    // ── Measured savings baseline ──
    //
    // Captured by the builder *before* compression, while the full candidate
    // set is still present. `baseline_tokens` is what the model would have
    // consumed had we handed it everything we found, uncompressed; comparing it
    // with `token_count` after compression gives a saving that is measured
    // rather than assumed.
    /// Tokens in the uncompressed candidate set. 0 when not measured.
    #[serde(default)]
    pub baseline_tokens: u32,
    /// Entities considered before pruning.
    #[serde(default)]
    pub candidate_entities: u32,
    /// Memory records considered before pruning.
    #[serde(default)]
    pub candidate_memories: u32,

    // ── Why each item is here ──
    //
    // Filled in as the pipeline runs: the seeder records what matched, the
    // expander records which hop pulled a neighbour in, the ranker attaches the
    // score breakdown, and the compressor records what it had to drop and why.
    //
    // Without this the package is a black box — a user sees a list and has to
    // trust it. With it, "why is this here?" has an answer that comes from the
    // engine itself rather than from a guess.
    #[serde(default)]
    pub provenance: crate::core::context::provenance::Provenance,

    // ── Agent instructions (AGENTS.md) ──
    //
    // Project instruction file content (conventionally AGENTS.md) attached to
    // the package so the AI sees the project's rules in the same payload as
    // the context itself. Absent when the project has not defined one.
    #[serde(default)]
    pub agent_instructions: Option<String>,

    // ── Conflict firewall (Система 2) ──
    //
    // How many memory records the builder excluded because they participate in
    // an unresolved contradiction (Conflicted) or were superseded by a resolved
    // conflict (Superseded). The package carries only the Current Truth; this
    // counter keeps the exclusion observable instead of silent.
    #[serde(default)]
    pub conflicts_excluded: u32,
}

impl ContextPackage {
    pub fn new(user_intent: UserIntent) -> Self {
        let now = Utc::now();
        Self {
            id: crate::core::EntityId::new().as_str().to_string(),
            entities: Vec::new(),
            relationships: Vec::new(),
            memory_records: Vec::new(),
            temporal_slice: TemporalSlice { from: now, to: now },
            relevance_scores: HashMap::new(),
            user_intent,
            created_at: now,
            token_count: 0,
            compressed_size: 0,
            baseline_tokens: 0,
            candidate_entities: 0,
            candidate_memories: 0,
            provenance: crate::core::context::provenance::Provenance::new(),
            agent_instructions: None,
            conflicts_excluded: 0,
        }
    }

    /// Validate package invariants.
    pub fn validate(&self) -> crate::core::Result<()> {
        if self.user_intent.query.is_empty() {
            return Err(crate::core::AppError::Validation(
                "ContextPackage user_intent query cannot be empty".into(),
            ));
        }
        if self.user_intent.confidence < 0.0 || self.user_intent.confidence > 1.0 {
            return Err(crate::core::AppError::Validation(
                "Confidence must be between 0.0 and 1.0".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_intent() -> UserIntent {
        UserIntent {
            query: "find project status".to_string(),
            intent_type: IntentType::Search,
            confidence: 0.8,
            keywords: vec![
                "find".to_string(),
                "project".to_string(),
                "status".to_string(),
            ],
            temporal: None,
        }
    }

    fn sample_package() -> ContextPackage {
        ContextPackage::new(sample_intent())
    }

    #[test]
    fn new_package_defaults() {
        let p = sample_package();
        assert!(!p.id.is_empty());
        assert!(p.entities.is_empty());
        assert!(p.relationships.is_empty());
        assert!(p.memory_records.is_empty());
        assert_eq!(p.user_intent.query, "find project status");
        assert_eq!(p.token_count, 0);
    }

    #[test]
    fn validate_valid_package() {
        assert!(sample_package().validate().is_ok());
    }

    #[test]
    fn validate_empty_query_fails() {
        let mut p = sample_package();
        p.user_intent.query = String::new();
        assert!(p.validate().is_err());
    }

    #[test]
    fn validate_confidence_too_high() {
        let mut p = sample_package();
        p.user_intent.confidence = 1.5;
        assert!(p.validate().is_err());
    }

    #[test]
    fn validate_confidence_negative() {
        let mut p = sample_package();
        p.user_intent.confidence = -0.1;
        assert!(p.validate().is_err());
    }

    #[test]
    fn intent_type_serialization() {
        for it in [
            IntentType::Search,
            IntentType::Analysis,
            IntentType::Decision,
            IntentType::Creation,
            IntentType::Update,
            IntentType::Exploration,
        ] {
            let json = serde_json::to_string(&it).unwrap();
            let decoded: IntentType = serde_json::from_str(&json).unwrap();
            assert_eq!(it, decoded);
        }
    }

    #[test]
    fn package_serialization() {
        let p = sample_package();
        let json = serde_json::to_string(&p).unwrap();
        let decoded: ContextPackage = serde_json::from_str(&json).unwrap();
        assert_eq!(p.id, decoded.id);
        assert_eq!(p.user_intent.query, decoded.user_intent.query);
    }

    #[test]
    fn package_clone() {
        let p = sample_package();
        let cloned = p.clone();
        assert_eq!(p.id, cloned.id);
    }
}
