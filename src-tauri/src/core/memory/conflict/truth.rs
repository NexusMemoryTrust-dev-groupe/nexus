//! Truth scoring — deterministic, LLM-free plausibility scoring of a memory.
//!
//! Each candidate in a conflict gets a score from conservative weighted
//! signals; the engine picks the winner and reports the margin as confidence.
//! Everything here is a pure function — unit-testable without a database.

use chrono::{DateTime, Utc};

use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::types::{MemoryLayer, MemorySource, MemoryState};

/// How many days until a memory's age fully decays its freshness signal.
pub const FRESHNESS_DECAY_DAYS: f64 = 90.0;

/// Weight of the freshness signal (full weight when brand new).
pub const WEIGHT_FRESHNESS: f64 = 3.0;

/// Weight of an explicit human confirmation. Deliberately higher than
/// freshness at its peak: a human-confirmed decision must outrank any mere
/// recency, otherwise the newest capture would flip settled facts.
pub const WEIGHT_USER_CONFIRMED: f64 = 4.0;

/// Weight of a reliable source (Manual / Git).
pub const WEIGHT_RELIABLE_SOURCE: f64 = 1.0;

/// Weight of a decision/strategic layer — decisions are deliberate.
pub const WEIGHT_DECISION_LAYER: f64 = 1.0;

/// Weight of being part of a dependency chain (derived / supersedes).
pub const WEIGHT_DEPENDENCY_GRAPH: f64 = 1.0;

/// Weight per useful-feedback vote (capped so spam can't dominate).
pub const WEIGHT_USEFUL_VOTE: f64 = 0.5;
pub const USEFUL_VOTE_CAP: u32 = 3;

/// Weight of the author's own confidence score (weak, can be negative).
pub const WEIGHT_AUTHOR_CONFIDENCE: f64 = 1.0;

/// The score of one conflict candidate plus why it scored that way.
#[derive(Debug, Clone, PartialEq)]
pub struct TruthScore {
    pub score: f64,
    /// Human-readable reasons, e.g. "+ recent source", "+ user confirmation".
    pub reasons: Vec<String>,
}

/// Context passed to scoring (currently just "now" for freshness).
#[derive(Debug, Clone, Copy)]
pub struct TruthContext {
    pub now: DateTime<Utc>,
}

impl TruthContext {
    pub fn now() -> Self {
        Self { now: Utc::now() }
    }
}

/// Deterministic plausibility scorer over memory metadata.
pub struct TruthScorer;

impl TruthScorer {
    /// Score a single conflict candidate. Higher is more plausible.
    ///
    /// Deterministic: identical record + identical context always produce
    /// the same score and the same reasons.
    pub fn score(record: &MemoryRecord, ctx: &TruthContext) -> TruthScore {
        let mut score = 0.0;
        let mut reasons = Vec::new();

        // 1) Freshness — exponential-ish decay with age.
        let age_days = (ctx.now - record.updated_at).num_seconds().max(0) as f64 / 86_400.0;
        let freshness = (1.0 - age_days / FRESHNESS_DECAY_DAYS).clamp(0.0, 1.0);
        if freshness > 0.0 {
            score += WEIGHT_FRESHNESS * freshness;
            reasons.push("+ recent source".to_string());
        }

        // 2) Explicit human confirmation — the strongest single signal.
        if record.memory_state == MemoryState::UserConfirmed || record.confirmed_by.is_some() {
            score += WEIGHT_USER_CONFIRMED;
            reasons.push("+ user confirmation".to_string());
        }

        // 3) Source reliability.
        let reliable = matches!(record.source, MemorySource::Manual | MemorySource::Git);
        if reliable {
            score += WEIGHT_RELIABLE_SOURCE;
            reasons.push("+ reliable source".to_string());
        }

        // 4) Decision / strategic layer — deliberate, not incidental.
        if matches!(record.layer, MemoryLayer::Decision | MemoryLayer::Strategic) {
            score += WEIGHT_DECISION_LAYER;
            reasons.push("+ decision layer".to_string());
        }

        // 5) Dependency graph — derived-from or replaced-someone records are
        //    anchors: the record stands in a chain of decisions.
        if !record.derived_from.is_empty() || record.supersedes_id.is_some() {
            score += WEIGHT_DEPENDENCY_GRAPH;
            reasons.push("+ dependency graph".to_string());
        }

        // 6) Repeated useful use — the memory proved itself in context.
        let useful = record.feedback.useful.min(USEFUL_VOTE_CAP) as f64;
        if useful > 0.0 {
            score += WEIGHT_USEFUL_VOTE * useful;
            reasons.push("+ repeated usage".to_string());
        }

        // 7) Author confidence — weak, can subtract.
        score += WEIGHT_AUTHOR_CONFIDENCE * (record.confidence_score - 0.5);

        if reasons.is_empty() {
            reasons.push("baseline".to_string());
        }

        TruthScore {
            score: score.max(0.0),
            reasons,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::types::MemoryFeedback;

    fn record(author: &str) -> MemoryRecord {
        MemoryRecord::new(
            "Database".to_string(),
            "Use PostgreSQL".to_string(),
            author.to_string(),
            MemorySource::Manual,
        )
        .unwrap()
    }

    fn ctx(days_ago: i64) -> TruthContext {
        TruthContext {
            now: Utc::now() + chrono::Duration::days(days_ago),
        }
    }

    #[test]
    fn freshness_prefers_newer_record() {
        let old = {
            let mut r = record("alice");
            r.updated_at = Utc::now() - chrono::Duration::days(80);
            r
        };
        let new = record("bob");
        let s_old = TruthScorer::score(&old, &ctx(0));
        let s_new = TruthScorer::score(&new, &ctx(0));
        assert!(s_new.score > s_old.score, "newer must score higher");
        assert!(s_new.reasons.contains(&"+ recent source".to_string()));
    }

    #[test]
    fn user_confirmed_beats_recent() {
        let mut confirmed = record("alice");
        confirmed.memory_state = MemoryState::UserConfirmed;
        confirmed.confirmed_by = Some("bob".to_string());
        confirmed.updated_at = Utc::now() - chrono::Duration::days(70);
        let recent = record("bob");
        let s_c = TruthScorer::score(&confirmed, &ctx(0));
        let s_r = TruthScorer::score(&recent, &ctx(0));
        assert!(s_c.score > s_r.score);
        assert!(s_c.reasons.contains(&"+ user confirmation".to_string()));
    }

    #[test]
    fn unreliable_source_gets_no_boost() {
        let mut gpt = record("alice");
        gpt.source = MemorySource::AiGenerated;
        let manual = record("bob");
        let s_g = TruthScorer::score(&gpt, &ctx(0));
        let s_m = TruthScorer::score(&manual, &ctx(0));
        assert!(s_m.score > s_g.score);
        assert!(!s_g.reasons.contains(&"+ reliable source".to_string()));
    }

    #[test]
    fn decision_layer_boosts() {
        let mut decision = record("alice");
        decision.layer = MemoryLayer::Decision;
        let episodic = record("bob");
        let s_d = TruthScorer::score(&decision, &ctx(0));
        let s_e = TruthScorer::score(&episodic, &ctx(0));
        assert!(s_d.score > s_e.score);
        assert!(s_d.reasons.contains(&"+ decision layer".to_string()));
    }

    #[test]
    fn dependency_graph_boosts() {
        let mut derived = record("alice");
        derived.derived_from = vec!["mem-old".to_string()];
        let standalone = record("bob");
        let s_d = TruthScorer::score(&derived, &ctx(0));
        let s_s = TruthScorer::score(&standalone, &ctx(0));
        assert!(s_d.score > s_s.score);
    }

    #[test]
    fn useful_feedback_boosts_up_to_cap() {
        let mut loved = record("alice");
        loved.feedback = MemoryFeedback {
            useful: 5,
            ..Default::default()
        };
        let plain = record("bob");
        let s_l = TruthScorer::score(&loved, &ctx(0));
        let s_p = TruthScorer::score(&plain, &ctx(0));
        assert!(s_l.score > s_p.score);
        assert!(s_l.reasons.contains(&"+ repeated usage".to_string()));
    }

    #[test]
    fn author_confidence_weak_signal() {
        let mut confident = record("alice");
        confident.confidence_score = 0.95;
        let mut unsure = record("bob");
        unsure.confidence_score = 0.2;
        let s_c = TruthScorer::score(&confident, &ctx(0));
        let s_u = TruthScorer::score(&unsure, &ctx(0));
        assert!(s_c.score > s_u.score);
    }

    #[test]
    fn score_never_negative() {
        let mut r = record("alice");
        r.source = MemorySource::AiGenerated;
        r.confidence_score = 0.0;
        r.updated_at = Utc::now() - chrono::Duration::days(500);
        let s = TruthScorer::score(&r, &ctx(0));
        assert!(s.score >= 0.0);
    }

    #[test]
    fn deterministic_same_input_same_score() {
        let r = record("alice");
        let a = TruthScorer::score(&r, &ctx(0));
        let b = TruthScorer::score(&r, &ctx(0));
        assert_eq!(a, b);
    }

    #[test]
    fn baseline_reason_when_no_signals() {
        let mut r = record("alice");
        r.source = MemorySource::AiGenerated;
        r.updated_at = Utc::now() - chrono::Duration::days(500);
        r.layer = MemoryLayer::Episodic;
        let s = TruthScorer::score(&r, &ctx(0));
        assert!(s.reasons.contains(&"baseline".to_string()));
    }
}
