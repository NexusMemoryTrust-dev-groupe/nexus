//! Current Truth Engine — deterministic winner selection inside a conflict.
//!
//! Pure functions only: score every candidate with [`TruthScorer`], pick the
//! highest, and report the margin between winner and runner-up as confidence.
//! When confidence clears [`TRUTH_AUTO_RESOLVE`] the service may auto-resolve;
//! below it the conflict stays open and a human is asked "which one is correct?".

use chrono::{DateTime, Utc};

use crate::core::entity_id::EntityId;
use crate::core::memory::conflict::TruthVerdict;
use crate::core::memory::conflict::truth::{TruthContext, TruthScorer};
use crate::core::memory::memory_record::MemoryRecord;

/// Confidence threshold (0.0–1.0) above which the engine may auto-resolve a
/// conflict without asking a human. Below it the group stays `Open`.
pub const TRUTH_AUTO_RESOLVE: f64 = 0.70;

/// Result of the engine pass over one conflict's candidates.
#[derive(Debug, Clone, PartialEq)]
pub struct TruthDecision {
    /// The current-truth verdict for the whole group.
    pub verdict: TruthVerdict,
    /// True when `verdict.confidence >= TRUTH_AUTO_RESOLVE`.
    pub auto_resolvable: bool,
}

/// Candidate with its computed score, sorted so the first is the winner.
#[derive(Debug, Clone)]
struct Scored {
    record: MemoryRecord,
    score: f64,
    reasons: Vec<String>,
}

/// Deterministic winner selection.
///
/// Deterministic: identical candidates + identical context always produce the
/// same verdict.
pub fn determine_truth(candidates: &[MemoryRecord], ctx: &TruthContext) -> Option<TruthVerdict> {
    if candidates.is_empty() {
        return None;
    }

    // 1) Score every candidate.
    let mut scored: Vec<Scored> = candidates
        .iter()
        .map(|record| {
            let s = TruthScorer::score(record, ctx);
            Scored {
                record: record.clone(),
                score: s.score,
                reasons: s.reasons,
            }
        })
        .collect();

    // 2) Sort descending by score; ties break by record id for determinism.
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.record.id.as_str().cmp(b.record.id.as_str()))
    });

    let winner = &scored[0];

    // 3) Confidence: normalised margin between winner and runner-up.
    //    Base 0.5; +0.5 * (margin / total) capped at 1.0. A single candidate
    //    is trivially true (1.0).
    let confidence = if scored.len() == 1 {
        1.0
    } else {
        let runner_up = &scored[1];
        let total: f64 = scored.iter().map(|s| s.score).sum();
        if total <= 0.0 {
            0.5
        } else {
            let margin = winner.score - runner_up.score;
            (0.5 + 0.5 * (margin / total)).min(1.0)
        }
    };

    Some(TruthVerdict {
        winner_id: winner.record.id.clone(),
        confidence,
        reasons: winner.reasons.clone(),
    })
}

/// Shorthand: verdict with the auto-resolvable flag already computed.
pub fn decide(candidates: &[MemoryRecord], ctx: &TruthContext) -> Option<TruthDecision> {
    determine_truth(candidates, ctx).map(|verdict| TruthDecision {
        auto_resolvable: verdict.confidence >= TRUTH_AUTO_RESOLVE,
        verdict,
    })
}

/// Helper for callers that hold `DateTime<Utc>`: score "now" explicitly.
pub fn determine_truth_at(candidates: &[MemoryRecord], now: DateTime<Utc>) -> Option<TruthVerdict> {
    determine_truth(candidates, &TruthContext { now })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::types::{MemoryLayer, MemorySource, MemoryState};

    fn record(title: &str, author: &str) -> MemoryRecord {
        MemoryRecord::new(
            title.to_string(),
            "content".to_string(),
            author.to_string(),
            MemorySource::Manual,
        )
        .unwrap()
    }

    fn fresh() -> TruthContext {
        TruthContext::now()
    }

    #[test]
    fn picks_highest_scored_candidate() {
        let mut winner = record("db", "alice");
        winner.memory_state = MemoryState::UserConfirmed;
        winner.confirmed_by = Some("bob".to_string());
        let loser = record("db", "carol");
        let v = determine_truth(&[loser, winner.clone()], &fresh()).unwrap();
        assert_eq!(v.winner_id, winner.id);
    }

    #[test]
    fn empty_candidates_yield_none() {
        assert!(determine_truth(&[], &fresh()).is_none());
    }

    #[test]
    fn single_candidate_trivially_true() {
        let r = record("db", "alice");
        let v = determine_truth(&[r], &fresh()).unwrap();
        assert_eq!(v.confidence, 1.0);
    }

    #[test]
    fn clear_winner_clears_threshold() {
        let mut winner = record("db", "alice");
        winner.memory_state = MemoryState::UserConfirmed;
        winner.confirmed_by = Some("bob".to_string());
        winner.layer = MemoryLayer::Decision;
        winner.updated_at = Utc::now();
        let mut loser = record("db", "carol");
        loser.source = MemorySource::AiGenerated;
        loser.updated_at = Utc::now() - chrono::Duration::days(200);
        let d = decide(&[loser, winner], &fresh()).unwrap();
        assert!(d.auto_resolvable, "confidence {}", d.verdict.confidence);
        assert!(d.verdict.confidence >= TRUTH_AUTO_RESOLVE);
    }

    #[test]
    fn tied_candidates_stay_below_threshold() {
        let a = record("db", "alice");
        let b = record("db", "bob");
        let v = determine_truth(&[a, b], &fresh()).unwrap();
        assert!(v.confidence < TRUTH_AUTO_RESOLVE, "{}", v.confidence);
    }

    #[test]
    fn verdict_reasons_come_from_winner() {
        let mut winner = record("db", "alice");
        winner.memory_state = MemoryState::UserConfirmed;
        winner.confirmed_by = Some("bob".to_string());
        let loser = record("db", "carol");
        let v = determine_truth(&[loser, winner], &fresh()).unwrap();
        assert!(v.reasons.contains(&"+ user confirmation".to_string()));
    }

    #[test]
    fn deterministic_same_input_same_verdict() {
        let a = record("db", "alice");
        let b = record("db", "bob");
        let v1 = determine_truth(&[a.clone(), b.clone()], &fresh()).unwrap();
        let v2 = determine_truth(&[a, b], &fresh()).unwrap();
        assert_eq!(v1, v2);
    }

    #[test]
    fn confidence_bounded() {
        let a = record("db", "alice");
        let b = record("db", "bob");
        let v = determine_truth(&[a, b], &fresh()).unwrap();
        assert!((0.5..=1.0).contains(&v.confidence));
    }

    #[test]
    fn newer_record_wins_tie_on_freshness() {
        let mut old = record("db", "alice");
        old.updated_at = Utc::now() - chrono::Duration::days(60);
        let mut new = record("db", "bob");
        new.updated_at = Utc::now();
        let v = determine_truth(&[old, new.clone()], &fresh()).unwrap();
        assert_eq!(v.winner_id, new.id);
    }

    #[test]
    fn decide_marks_low_confidence_not_auto() {
        let a = record("db", "alice");
        let b = record("db", "bob");
        let d = decide(&[a, b], &fresh()).unwrap();
        assert!(!d.auto_resolvable);
    }

    #[test]
    fn determine_truth_at_uses_given_now() {
        let mut winner = record("db", "alice");
        winner.updated_at = Utc::now();
        let mut old = record("db", "bob");
        old.updated_at = Utc::now() - chrono::Duration::days(100);
        let now = Utc::now();
        let v = determine_truth_at(&[old, winner.clone()], now).unwrap();
        assert_eq!(v.winner_id, winner.id);
    }

    #[test]
    fn aged_everything_below_threshold() {
        let mut a = record("db", "alice");
        a.source = MemorySource::AiGenerated;
        a.updated_at = Utc::now() - chrono::Duration::days(400);
        let mut b = record("db", "bob");
        b.source = MemorySource::AiGenerated;
        b.updated_at = Utc::now() - chrono::Duration::days(450);
        let d = decide(&[a, b], &fresh()).unwrap();
        assert!(!d.auto_resolvable);
    }
}
