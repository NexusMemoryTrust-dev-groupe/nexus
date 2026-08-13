//! Conflict Engine v2 verdicts — the relationship between two memories.
//!
//! Plan 2.1: instead of only "is this a conflict?", classify the *kind* of
//! relation between two memories with a deterministic, LLM-free decision:
//!
//! - [`PairVerdict::Superseded`] — an explicit supersession link
//!   (`a.supersedes_id == b` or `b.supersedes_id == a`).
//! - [`PairVerdict::Contradicted`] — same topic, divergent facts (the
//!   [`is_conflicting_pair`] detector).
//! - [`PairVerdict::Supported`] — strongly overlapping / restating the same
//!   fact; one reinforces the other.
//! - [`PairVerdict::Unrelated`] — no meaningful overlap, no shared skeleton.
//! - [`PairVerdict::Uncertain`] — partial overlap; not confident either way.
//!
//! Everything here is a pure function over two `MemoryRecord`s — unit-testable
//! without a database. Determinism: identical inputs always produce the same
//! verdict.

use crate::core::memory::memory_lifecycle::{is_conflicting_pair, text_overlap};
use crate::core::memory::memory_record::MemoryRecord;

/// The relation between a pair of memories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairVerdict {
    /// One memory explicitly replaced/superseded the other.
    Superseded,
    /// Same topic, contradictory facts.
    Contradicted,
    /// One memory reinforces/supports the other (same claim, different framing).
    Supported,
    /// No meaningful overlap on any channel.
    Unrelated,
    /// Partial overlap — not confident about the relation.
    Uncertain,
}

impl PairVerdict {
    /// Stable machine/JSON code.
    pub fn as_str(&self) -> &'static str {
        match self {
            PairVerdict::Superseded => "superseded",
            PairVerdict::Contradicted => "contradicted",
            PairVerdict::Supported => "supported",
            PairVerdict::Unrelated => "unrelated",
            PairVerdict::Uncertain => "uncertain",
        }
    }
}

/// Overlap above this is a restatement → `Supported` (when not a conflict).
const SUPPORT_OVERLAP: f64 = 0.75;

/// Below this there is effectively no shared claim.
const UNRELATED_OVERLAP: f64 = 0.20;

/// Shared significant stems at/below this → unrelated (no claim skeleton).
const UNRELATED_STEMS: usize = 1;

/// Minimum overlap for a negation flip to count as a contradiction — high
/// enough that "no budget for X" vs "not deployed" never collides, low enough
/// that "deploy on Fridays" vs "do not deploy on Fridays" (0.67) is caught.
const NEGATION_MIN_OVERLAP: f64 = 0.55;

/// Minimum overlap for temporal supersession — the two texts must be about
/// the same claim, not merely mention a year each.
const TEMPORAL_MIN_OVERLAP: f64 = 0.40;

/// Deterministically classify the relation between two memories.
///
/// Order of checks (a and b are symmetric; `Semantic` similarity is optional,
/// pass through the embedding cosine when the model is loaded — otherwise the
/// detector falls back to the lexical channel only):
/// 1. Explicit supersession link wins over everything.
/// 2. Temporal supersession — the same claim restated at a later point
///    (higher `version` or a later year in the text) replaces the older one
///    rather than contradicting it (plan 2.3).
/// 3. A strong conflict (same topic, divergent facts) → `Contradicted`.
/// 4. High text overlap (restatement) that is *not* divergent → `Supported`.
/// 5. No overlap + no shared stems → `Unrelated`.
/// 6. Everything else → `Uncertain`.
pub fn classify(a: &MemoryRecord, b: &MemoryRecord, semantic: Option<f64>) -> PairVerdict {
    // 1) Explicit lifecycle link.
    let links_supersede = a.supersedes_id.as_deref() == Some(b.id.as_str())
        || b.supersedes_id.as_deref() == Some(a.id.as_str());
    if links_supersede {
        return PairVerdict::Superseded;
    }

    let text_a = format!("{} {}", a.title, a.content);
    let text_b = format!("{} {}", b.title, b.content);
    let overlap = text_overlap(&text_a, &text_b);

    // 1b) Temporal supersession — same topic, one claim is provably newer.
    //     A higher version number or a later year in the text means the newer
    //     statement replaced the older one. This must win over the
    //     contradiction channel: "v2 of a fact" is not a disagreement.
    if overlap >= TEMPORAL_MIN_OVERLAP {
        let years_a = extract_years(&text_a);
        let years_b = extract_years(&text_b);
        let later_year = matches!(
            (years_a.iter().max(), years_b.iter().max()),
            (Some(ya), Some(yb)) if ya != yb
        );
        if a.version != b.version || later_year {
            return PairVerdict::Superseded;
        }
    }

    // 2) Contradiction — same topic, divergent facts.
    if is_conflicting_pair(&text_a, &text_b, semantic) {
        return PairVerdict::Contradicted;
    }

    // 2b) Negation flip on the same claim — the strongest contradiction
    //     signal. The lexical detector misses it because "not"/"не" are stop
    //     words (no divergent significant words), but a polarity flip on a
    //     highly overlapping claim is a genuine contradiction ("deploy on
    //     Fridays" vs "do not deploy on Fridays"). Requires substantial
    //     overlap so unrelated negations ("no budget" vs "not deployed") stay
    //     out.
    if overlap >= NEGATION_MIN_OVERLAP
        && has_negation(&text_a) != has_negation(&text_b)
        && text_overlap(a.title.as_str(), b.title.as_str()) >= 0.3
    {
        return PairVerdict::Contradicted;
    }

    // 2c) Numeric divergence — both claims state concrete numbers and none of
    //     them match ("3 replicas" vs "5 replicas", port 8080 vs 9090). The
    //     lexical detector misses this because single digits/numbers are
    //     dropped as insignificant words, yet differing values on a shared
    //     topic are a textbook contradiction. Overlap requirement keeps
    //     unrelated numeric statements ("budget 2026" vs "port 8080") out.
    if overlap >= 0.4 {
        let numbers_a = extract_numbers(&text_a);
        let numbers_b = extract_numbers(&text_b);
        let shared = numbers_a.intersection(&numbers_b).count();
        if !numbers_a.is_empty() && !numbers_b.is_empty() && shared == 0 {
            return PairVerdict::Contradicted;
        }
    }

    // 3) Strong restatement that is not divergent → supported.
    if overlap >= SUPPORT_OVERLAP {
        return PairVerdict::Supported;
    }

    // 4) Effectively no overlap → unrelated.
    if overlap < UNRELATED_OVERLAP {
        return PairVerdict::Unrelated;
    }

    // 5) Partial overlap → uncertain.
    PairVerdict::Uncertain
}

/// Signals that make a contradiction *specific* enough to trust (plan 2.5):
/// negation flips and numeric disagreement are the strongest, most detectable
/// contradiction classes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClaimComparison {
    /// Dice overlap of the claim text (title + content).
    pub overlap: f64,
    /// True when the two claims are about the same entity/topic.
    pub same_entity: bool,
    /// True when one claim negates the other ("is X" vs "is not X").
    pub negation_flip: bool,
    /// True when both mention numbers that differ (port 8080 vs 9090).
    pub numeric_divergence: bool,
    /// True when one claim is provably newer than the other (later year in
    /// the text, or higher version) while both are about the same topic.
    pub temporal_supersession: bool,
    /// True when one claim is a strict supersession of the other's topic by
    /// text (the classification catches it regardless).
    pub related: bool,
}

/// Words that flip a claim's polarity.
const NEGATIONS: &[&str] = &["not", "never", "no", "without", "не", "нет", "без"];

/// A number in a text (integer or decimal).
fn extract_numbers(s: &str) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    for tok in s.split_whitespace() {
        let cleaned: String = tok
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
            .collect();
        // A token is numeric if it contains at least one digit and, stripped of
        // digits, is only separators (so "8080" and "v2.1" both count).
        if tok.contains(|c: char| c.is_ascii_digit()) && cleaned.chars().any(|c| c.is_ascii_digit())
        {
            out.insert(cleaned);
        }
    }
    out
}

/// Whether the text contains a negation word/boundary.
fn has_negation(s: &str) -> bool {
    for tok in s.split_whitespace() {
        let word = tok
            .to_lowercase()
            .chars()
            .filter(|c| !c.is_ascii_punctuation())
            .collect::<String>();
        if NEGATIONS.contains(&word.as_str()) {
            return true;
        }
    }
    false
}

/// Years mentioned in a text (4-digit numbers in a plausible calendar range).
///
/// Used by the temporal channel (plan 2.3): a claim stating a later year
/// ("deployed in 2025" vs "deployed in 2024") is a newer statement of the
/// same fact, not a contradiction. Version numbers like "v2.1" or ports like
/// "8080" are excluded by the length/range check.
fn extract_years(s: &str) -> Vec<u32> {
    let mut years = Vec::new();
    for tok in s.split_whitespace() {
        let cleaned: String = tok.chars().filter(|c| c.is_ascii_digit()).collect();
        if cleaned.len() == 4
            && let Ok(year) = cleaned.parse::<u32>()
            && (1900..=2100).contains(&year)
        {
            years.push(year);
        }
    }
    years
}

/// Compare two claims and surface the signals that make a contradiction
/// concrete (plan 2.2: entity extraction + claim comparison).
///
/// Pure and deterministic. Used by the benchmark to separate paraphrase /
/// negation / numbers / architecture failure modes so the detection rate can
/// be measured per class (plan 2.5).
pub fn compare_claims(a: &MemoryRecord, b: &MemoryRecord) -> ClaimComparison {
    let text_a = format!("{} {}", a.title, a.content);
    let text_b = format!("{} {}", b.title, b.content);
    let overlap = text_overlap(&text_a, &text_b);

    let numbers_a = extract_numbers(&text_a);
    let numbers_b = extract_numbers(&text_b);
    let shared_numbers = numbers_a.intersection(&numbers_b).count();
    // Both have at least one number but none of them match -> divergent values.
    let numeric_divergence = !numbers_a.is_empty() && !numbers_b.is_empty() && shared_numbers == 0;

    // Temporal supersession signal: same topic, but one claim is provably
    // newer (a later year in the text or a higher version number).
    let years_a = extract_years(&text_a);
    let years_b = extract_years(&text_b);
    let later_year = matches!(
        (years_a.iter().max(), years_b.iter().max()),
        (Some(ya), Some(yb)) if ya != yb
    );
    let temporal_supersession =
        overlap >= TEMPORAL_MIN_OVERLAP && (a.version != b.version || later_year);

    ClaimComparison {
        overlap,
        same_entity: overlap >= 0.35,
        negation_flip: has_negation(&text_a) != has_negation(&text_b),
        numeric_divergence,
        temporal_supersession,
        related: overlap >= 0.35,
    }
}

/// Source-trust scoring for a single memory (plan 2.4): how much we trust the
/// *origin* of a claim, combining the source reliability, the trust state and
/// an explicit human confirmation. Ranges 0.0 (lowest) — 1.0 (highest).
///
/// This is exposure of the existing `TruthScorer` signals as a standalone,
/// testable trust value for the conflict v2 verdict pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustLevel {
    Low,
    Medium,
    High,
}

impl TrustLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            TrustLevel::Low => "low",
            TrustLevel::Medium => "medium",
            TrustLevel::High => "high",
        }
    }
}

/// Compute the trust level of a memory's origin.
///
/// * `High` — explicitly user-confirmed, or a reliable source (Manual/Git)
///   with a `Current`/`UserConfirmed` state.
/// * `Low` — AI-generated, `Inferred`/`Conflicted`, low confidence.
/// * `Medium` — everything else.
pub fn source_trust(record: &MemoryRecord) -> TrustLevel {
    use crate::core::memory::types::{MemorySource, MemoryState};

    let confirmed = record.memory_state == MemoryState::UserConfirmed
        || record.confirmed_by.is_some()
        || record.confirmed_at.is_some();
    let reliable = matches!(record.source, MemorySource::Manual | MemorySource::Git);

    if confirmed && (reliable || record.confidence_score >= 0.6) {
        return TrustLevel::High;
    }
    let weak = matches!(record.source, MemorySource::AiGenerated)
        || matches!(
            record.memory_state,
            MemoryState::Inferred | MemoryState::Conflicted
        )
        || record.confidence_score < 0.3;
    if weak {
        return TrustLevel::Low;
    }
    if reliable {
        return TrustLevel::High;
    }
    TrustLevel::Medium
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::types::MemorySource;

    fn record(title: &str, content: &str) -> MemoryRecord {
        MemoryRecord::new(
            title.to_string(),
            content.to_string(),
            "t".to_string(),
            MemorySource::Manual,
        )
        .unwrap()
    }

    #[test]
    fn explicit_supersede_link_wins() {
        let old = record("Database", "Use PostgreSQL as the primary database");
        let mut new = record("Database", "Use SQLite as the primary database");
        new.supersedes_id = Some(old.id.as_str().to_string());
        assert_eq!(classify(&old, &new, None), PairVerdict::Superseded);
        // Symmetric: ordering must not matter.
        assert_eq!(classify(&new, &old, None), PairVerdict::Superseded);
    }

    #[test]
    fn contradiction_detected() {
        let a = record("Database", "Use PostgreSQL as the primary database");
        let b = record("Database", "Use MySQL as the primary database");
        // High lexical overlap (same skeleton) but divergent value -> conflict.
        assert_eq!(classify(&a, &b, None), PairVerdict::Contradicted);
    }

    #[test]
    fn paraphrase_conflict_via_semantic_channel() {
        let a = record("Deploy", "We deploy the service to AWS");
        let b = record("Deploy", "The service currently runs on Azure");
        // Low lexical overlap; semantic model confirms they are about the same
        // claim with contradictory values -> Contradicted.
        assert_eq!(classify(&a, &b, Some(0.9)), PairVerdict::Contradicted);
    }

    #[test]
    fn restatement_is_supported() {
        let a = record("Database", "Use PostgreSQL as the primary database");
        let b = record("Database", "Use PostgreSQL as our primary database system");
        // Near-identical restatement, no divergence -> supported, not conflict.
        assert_eq!(classify(&a, &b, Some(0.99)), PairVerdict::Supported);
    }

    #[test]
    fn unrelated_memories() {
        let a = record("Database", "Use PostgreSQL for persistence");
        let b = record("Onboarding", "Welcome email is sent to new hires");
        assert_eq!(classify(&a, &b, None), PairVerdict::Unrelated);
    }

    #[test]
    fn partial_overlap_is_uncertain() {
        // Shares the entity but not a clear claim or divergence.
        let a = record(
            "PostgreSQL",
            "PostgreSQL is used for the analytics warehouse",
        );
        let b = record("PostgreSQL", "PostgreSQL is the primary database");
        // Overlap is moderate (between unrelated and supported) and not a
        // conflict -> uncertain.
        let v = classify(&a, &b, None);
        assert!(
            matches!(v, PairVerdict::Uncertain),
            "expected uncertain, got {:?}",
            v
        );
        // Determinism.
        assert_eq!(v, classify(&b, &a, None));
    }

    #[test]
    fn verdict_codes_are_stable() {
        assert_eq!(PairVerdict::Superseded.as_str(), "superseded");
        assert_eq!(PairVerdict::Contradicted.as_str(), "contradicted");
        assert_eq!(PairVerdict::Supported.as_str(), "supported");
        assert_eq!(PairVerdict::Unrelated.as_str(), "unrelated");
        assert_eq!(PairVerdict::Uncertain.as_str(), "uncertain");
    }

    #[test]
    fn compare_claims_detects_negation_flip() {
        let a = record("Deploy", "Deploy the service to production on Fridays");
        let b = record(
            "Deploy",
            "Do not deploy the service to production on Fridays",
        );
        let c = compare_claims(&a, &b);
        assert!(
            c.negation_flip,
            "polarity flip (with/without 'not') must be flagged"
        );
        assert!(c.same_entity);
    }

    #[test]
    fn compare_claims_detects_numeric_divergence() {
        let a = record("Port", "Run the API on port 8080");
        let b = record("Port", "Run the API on port 9090");
        let c = compare_claims(&a, &b);
        assert!(
            c.numeric_divergence,
            "8080 vs 9090 must be marked divergent"
        );
        assert!(!a.content.contains("9090"));
        assert!(!b.content.contains("8080"));
    }

    #[test]
    fn compare_claims_matching_numbers_not_divergent() {
        let a = record("Port", "Run the API on port 8080");
        let b = record("Port", "The API listens on 8080");
        let c = compare_claims(&a, &b);
        assert!(
            !c.numeric_divergence,
            "same port 8080 is not a numeric divergence"
        );
    }

    #[test]
    fn compare_claims_no_negation_no_numbers() {
        let a = record("Database", "Use PostgreSQL as the primary database");
        let b = record("Database", "Use MySQL as the primary database");
        let c = compare_claims(&a, &b);
        assert!(!c.negation_flip);
        assert!(!c.numeric_divergence);
        assert!(c.same_entity);
    }

    #[test]
    fn source_trust_user_confirmed_is_high() {
        use crate::core::memory::types::MemorySource;
        let mut r =
            MemoryRecord::new("t".into(), "c".into(), "u".into(), MemorySource::Manual).unwrap();
        r.confirmed_by = Some("alice".to_string());
        r.confirmed_at = Some(chrono::Utc::now());
        assert_eq!(source_trust(&r), TrustLevel::High);
    }

    #[test]
    fn source_trust_ai_generated_is_low() {
        use crate::core::memory::types::MemorySource;
        let r = MemoryRecord::new(
            "t".into(),
            "c".into(),
            "u".into(),
            MemorySource::AiGenerated,
        )
        .unwrap();
        assert_eq!(source_trust(&r), TrustLevel::Low);
    }

    #[test]
    fn source_trust_manual_reliable_is_high() {
        use crate::core::memory::types::MemorySource;
        let r =
            MemoryRecord::new("t".into(), "c".into(), "u".into(), MemorySource::Manual).unwrap();
        assert_eq!(source_trust(&r), TrustLevel::High);
    }

    #[test]
    fn source_trust_inferred_conflicted_is_low() {
        use crate::core::memory::types::{MemorySource, MemoryState};
        let mut r =
            MemoryRecord::new("t".into(), "c".into(), "u".into(), MemorySource::Git).unwrap();
        r.memory_state = MemoryState::Conflicted;
        assert_eq!(source_trust(&r), TrustLevel::Low);
    }

    #[test]
    fn source_trust_medium_fallback() {
        use crate::core::memory::types::MemorySource;
        // Unknown/unreliable-but-not-weak source (Telegram), current state,
        // neutral confidence -> medium.
        let r =
            MemoryRecord::new("t".into(), "c".into(), "u".into(), MemorySource::Telegram).unwrap();
        assert_eq!(source_trust(&r), TrustLevel::Medium);
    }

    #[test]
    fn trust_codes_are_stable() {
        assert_eq!(TrustLevel::Low.as_str(), "low");
        assert_eq!(TrustLevel::Medium.as_str(), "medium");
        assert_eq!(TrustLevel::High.as_str(), "high");
    }

    #[test]
    fn compare_claims_is_deterministic() {
        let a = record("Port", "Run the API on port 8080");
        let b = record("Port", "Run the API on port 9090");
        assert_eq!(compare_claims(&a, &b), compare_claims(&a, &b));
    }

    #[test]
    fn temporal_later_year_is_superseded_not_conflict() {
        // Same claim, but one states a later year: the newer statement
        // replaces the older one — it is NOT a contradiction.
        let a = record("Deploy", "The service runs on AWS since 2024");
        let b = record("Deploy", "The service runs on AWS since 2025");
        let v = classify(&a, &b, None);
        assert_eq!(v, PairVerdict::Superseded);
        // Symmetric ordering must not matter.
        assert_eq!(classify(&b, &a, None), PairVerdict::Superseded);
    }

    #[test]
    fn temporal_higher_version_is_superseded() {
        let a = record("Database", "Use PostgreSQL as the primary database");
        let mut b = record(
            "Database",
            "Use PostgreSQL as the primary database with replication",
        );
        b.version = 2;
        let v = classify(&a, &b, None);
        assert_eq!(v, PairVerdict::Superseded);
        assert_eq!(classify(&b, &a, None), PairVerdict::Superseded);
    }

    #[test]
    fn temporal_years_differ_but_unrelated_topics() {
        // Both mention years but the topics are unrelated: no supersession.
        let a = record("Budget", "Annual budget for 2024 was approved");
        let b = record("Deploy", "Deploy to production on Fridays");
        assert_eq!(classify(&a, &b, None), PairVerdict::Unrelated);
    }

    #[test]
    fn temporal_same_year_is_not_superseded() {
        // Same year on both sides, no version delta: falls through to the
        // normal channels (supported, because it is a restatement).
        let a = record("Database", "Use PostgreSQL as the primary database");
        let b = record("Database", "Use PostgreSQL as our primary database storage");
        assert_eq!(classify(&a, &b, Some(0.99)), PairVerdict::Supported);
    }

    #[test]
    fn compare_claims_detects_temporal_supersession() {
        let a = record("Deploy", "The service runs on AWS since 2024");
        let b = record("Deploy", "The service runs on AWS since 2025");
        let c = compare_claims(&a, &b);
        assert!(
            c.temporal_supersession,
            "later year must flag temporal supersession"
        );
        assert!(c.same_entity);
    }

    #[test]
    fn extract_years_filters_ports_and_versions() {
        assert_eq!(extract_years("run on port 8080"), Vec::<u32>::new());
        assert_eq!(extract_years("version 2.1 released"), Vec::<u32>::new());
        assert_eq!(extract_years("deployed in 2024"), vec![2024]);
        assert_eq!(extract_years("budget 2024 then 2025"), vec![2024, 2025]);
    }
}
