//! Why a given item ended up in the context package.
//!
//! Every competitor answers "here is your context" and stops. The interesting
//! question — the one that decides whether a user trusts the tool — is *why*
//! this note and not that one. This module records that answer.
//!
//! Design rule: **nothing here is reconstructed after the fact.** Each reason is
//! appended by the pipeline stage that actually caused the inclusion, at the
//! moment it happens. Recomputing "probably it matched the query" later would be
//! a plausible-looking guess, and a guess in an explanation is worse than no
//! explanation at all.

use serde::{Deserialize, Serialize};

/// What kind of thing is being explained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    Entity,
    Memory,
}

impl ItemKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::Memory => "memory",
        }
    }
}

/// A single cause of inclusion.
///
/// An item usually has several: it matched the query *and* it is recent. Keeping
/// them as a list rather than collapsing to one "main" reason matters, because
/// the combination is what a user recognises as correct behaviour.
// `rename_all` renames the *variants*; struct fields inside a variant need
// `rename_all_fields`, otherwise `from_title` reaches the UI as snake_case
// while everything around it is camelCase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Reason {
    /// The graph search for the user's query returned this entity directly.
    QueryMatch { query: String },
    /// A keyword extracted from the query matched.
    KeywordMatch { keyword: String },
    /// Reached by walking the graph from a seed entity.
    GraphExpansion {
        from_id: String,
        from_title: String,
        hops: u32,
    },
    /// Full-text search over memories matched this record.
    MemorySearch { query: String },
    /// Included because it was changed recently.
    RecentActivity { age_days: i64 },
    /// Included because the user marked it important.
    HighImportance { importance: f64 },
}

impl Reason {
    /// Stable identifier for the UI to look up localised copy.
    pub fn id(&self) -> &'static str {
        match self {
            Self::QueryMatch { .. } => "queryMatch",
            Self::KeywordMatch { .. } => "keywordMatch",
            Self::GraphExpansion { .. } => "graphExpansion",
            Self::MemorySearch { .. } => "memorySearch",
            Self::RecentActivity { .. } => "recentActivity",
            Self::HighImportance { .. } => "highImportance",
        }
    }
}

/// One additive component of the relevance score.
///
/// The ranker adds up weighted signals. Exposing the addends rather than only
/// the sum is the difference between "score 0.72" and "0.4 because the title
/// matched, 0.2 recency, 0.1 base" — the latter is auditable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScorePart {
    /// Stable component id: `titleMatch`, `keyword`, `importance`, `recency`, ...
    pub component: String,
    /// Points this component contributed.
    pub points: f64,
}

impl ScorePart {
    pub fn new(component: &str, points: f64) -> Self {
        Self {
            component: component.to_string(),
            points,
        }
    }
}

/// Why an item was removed again after being considered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DropCause {
    /// Scored below the caller's relevance floor.
    BelowRelevance { score: f64, floor: f64 },
    /// Cut to fit the token budget.
    TokenBudget { limit: u32 },
    /// Cut by the `max_entities` cap.
    EntityCap { cap: u32 },
}

impl DropCause {
    pub fn id(&self) -> &'static str {
        match self {
            Self::BelowRelevance { .. } => "belowRelevance",
            Self::TokenBudget { .. } => "tokenBudget",
            Self::EntityCap { .. } => "entityCap",
        }
    }
}

/// The full trace for one item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trace {
    pub id: String,
    pub kind: ItemKind,
    pub title: String,
    /// Every recorded cause of inclusion, in the order they occurred.
    pub reasons: Vec<Reason>,
    /// Final relevance score, or `None` if ranking never ran for this item.
    pub score: Option<f64>,
    /// Additive breakdown of `score`.
    pub score_parts: Vec<ScorePart>,
    /// Tokens this item contributes to the package.
    pub tokens: u32,
    /// False when the item was considered and then dropped.
    pub included: bool,
    /// Present only when `included` is false.
    pub dropped: Option<DropCause>,
}

impl Trace {
    pub fn new(id: impl Into<String>, kind: ItemKind, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            title: title.into(),
            reasons: Vec::new(),
            score: None,
            score_parts: Vec::new(),
            tokens: 0,
            included: true,
            dropped: None,
        }
    }

    /// Append a cause, ignoring an exact duplicate.
    ///
    /// The pipeline can legitimately hit the same item twice (a query match that
    /// is also reachable by expansion), and repeating an identical line in the
    /// explanation is noise, not information.
    pub fn add_reason(&mut self, reason: Reason) {
        if !self.reasons.contains(&reason) {
            self.reasons.push(reason);
        }
    }

    pub fn mark_dropped(&mut self, cause: DropCause) {
        self.included = false;
        self.dropped = Some(cause);
    }
}

/// Traces for one context build, keyed by item id.
///
/// Insertion order is preserved so the explanation reads in pipeline order:
/// seeds first, then expansion, then injected memories.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    traces: Vec<Trace>,
}

impl Provenance {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a reason for `id`, creating the trace when first seen.
    pub fn record(&mut self, id: &str, kind: ItemKind, title: &str, reason: Reason) {
        match self.traces.iter_mut().find(|t| t.id == id) {
            Some(existing) => existing.add_reason(reason),
            None => {
                let mut trace = Trace::new(id, kind, title);
                trace.add_reason(reason);
                self.traces.push(trace);
            }
        }
    }

    /// Attach a score and its breakdown to an already-recorded item.
    /// Unknown ids are ignored: scoring something we never traced is not an
    /// error worth failing a context build over.
    pub fn set_score(&mut self, id: &str, score: f64, parts: Vec<ScorePart>) {
        if let Some(t) = self.traces.iter_mut().find(|t| t.id == id) {
            t.score = Some(score);
            t.score_parts = parts;
        }
    }

    pub fn set_tokens(&mut self, id: &str, tokens: u32) {
        if let Some(t) = self.traces.iter_mut().find(|t| t.id == id) {
            t.tokens = tokens;
        }
    }

    pub fn mark_dropped(&mut self, id: &str, cause: DropCause) {
        if let Some(t) = self.traces.iter_mut().find(|t| t.id == id) {
            t.mark_dropped(cause);
        }
    }

    /// Mark every traced item that is absent from `surviving_ids` as dropped.
    ///
    /// Called after a pruning stage: the stage knows *what* it kept, and this
    /// turns that into a per-item explanation without each stage having to
    /// bookkeep removals itself.
    pub fn reconcile(&mut self, surviving_ids: &[String], cause: DropCause) {
        for t in self.traces.iter_mut() {
            if t.included && !surviving_ids.iter().any(|id| id == &t.id) {
                t.mark_dropped(cause.clone());
            }
        }
    }

    pub fn traces(&self) -> &[Trace] {
        &self.traces
    }

    /// Traces that survived to the final package.
    pub fn included(&self) -> impl Iterator<Item = &Trace> {
        self.traces.iter().filter(|t| t.included)
    }

    /// Traces that were considered and discarded.
    pub fn dropped(&self) -> impl Iterator<Item = &Trace> {
        self.traces.iter().filter(|t| !t.included)
    }

    pub fn len(&self) -> usize {
        self.traces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.traces.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&Trace> {
        self.traces.iter().find(|t| t.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qm(q: &str) -> Reason {
        Reason::QueryMatch { query: q.into() }
    }

    #[test]
    fn recording_creates_a_trace() {
        let mut p = Provenance::new();
        p.record("e1", ItemKind::Entity, "Alpha", qm("alpha"));

        assert_eq!(p.len(), 1);
        let t = p.get("e1").unwrap();
        assert_eq!(t.title, "Alpha");
        assert_eq!(t.kind, ItemKind::Entity);
        assert_eq!(t.reasons.len(), 1);
        assert!(t.included, "a freshly recorded item is included by default");
    }

    #[test]
    fn second_reason_for_same_item_appends_not_replaces() {
        let mut p = Provenance::new();
        p.record("e1", ItemKind::Entity, "Alpha", qm("alpha"));
        p.record(
            "e1",
            ItemKind::Entity,
            "Alpha",
            Reason::RecentActivity { age_days: 2 },
        );

        assert_eq!(p.len(), 1, "same id must not create a second trace");
        assert_eq!(p.get("e1").unwrap().reasons.len(), 2);
    }

    #[test]
    fn identical_reason_is_not_duplicated() {
        let mut p = Provenance::new();
        p.record("e1", ItemKind::Entity, "Alpha", qm("alpha"));
        p.record("e1", ItemKind::Entity, "Alpha", qm("alpha"));

        assert_eq!(p.get("e1").unwrap().reasons.len(), 1);
    }

    #[test]
    fn distinct_keywords_are_both_kept() {
        let mut p = Provenance::new();
        p.record(
            "e1",
            ItemKind::Entity,
            "Alpha",
            Reason::KeywordMatch {
                keyword: "rust".into(),
            },
        );
        p.record(
            "e1",
            ItemKind::Entity,
            "Alpha",
            Reason::KeywordMatch {
                keyword: "async".into(),
            },
        );

        assert_eq!(p.get("e1").unwrap().reasons.len(), 2);
    }

    #[test]
    fn insertion_order_is_preserved() {
        let mut p = Provenance::new();
        for id in ["a", "b", "c"] {
            p.record(id, ItemKind::Entity, id, qm("q"));
        }
        let ids: Vec<&str> = p.traces().iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids,
            ["a", "b", "c"],
            "explanation must read in pipeline order"
        );
    }

    #[test]
    fn score_is_attached_with_its_breakdown() {
        let mut p = Provenance::new();
        p.record("e1", ItemKind::Entity, "Alpha", qm("alpha"));
        p.set_score(
            "e1",
            0.7,
            vec![
                ScorePart::new("titleMatch", 0.4),
                ScorePart::new("recency", 0.3),
            ],
        );

        let t = p.get("e1").unwrap();
        assert_eq!(t.score, Some(0.7));
        assert_eq!(t.score_parts.len(), 2);
        // The parts must actually add up to the score, or the explanation lies.
        let sum: f64 = t.score_parts.iter().map(|s| s.points).sum();
        assert!(
            (sum - 0.7).abs() < 1e-9,
            "parts must sum to the score, got {sum}"
        );
    }

    #[test]
    fn scoring_an_unknown_id_is_a_no_op() {
        let mut p = Provenance::new();
        p.set_score("ghost", 1.0, vec![]);
        assert!(p.is_empty(), "must not invent a trace from a score");
    }

    #[test]
    fn dropping_records_the_cause() {
        let mut p = Provenance::new();
        p.record("e1", ItemKind::Entity, "Alpha", qm("alpha"));
        p.mark_dropped(
            "e1",
            DropCause::BelowRelevance {
                score: 0.1,
                floor: 0.3,
            },
        );

        let t = p.get("e1").unwrap();
        assert!(!t.included);
        assert_eq!(
            t.dropped,
            Some(DropCause::BelowRelevance {
                score: 0.1,
                floor: 0.3
            })
        );
    }

    #[test]
    fn reconcile_drops_only_the_missing_items() {
        let mut p = Provenance::new();
        for id in ["keep", "cut"] {
            p.record(id, ItemKind::Entity, id, qm("q"));
        }
        p.reconcile(&["keep".to_string()], DropCause::TokenBudget { limit: 100 });

        assert!(p.get("keep").unwrap().included);
        assert!(!p.get("cut").unwrap().included);
        assert_eq!(p.included().count(), 1);
        assert_eq!(p.dropped().count(), 1);
    }

    #[test]
    fn reconcile_does_not_overwrite_an_earlier_cause() {
        let mut p = Provenance::new();
        p.record("e1", ItemKind::Entity, "Alpha", qm("q"));
        p.mark_dropped(
            "e1",
            DropCause::BelowRelevance {
                score: 0.1,
                floor: 0.3,
            },
        );
        // A later stage reconciles; the original, more specific cause must win.
        p.reconcile(&[], DropCause::TokenBudget { limit: 10 });

        assert_eq!(
            p.get("e1").unwrap().dropped,
            Some(DropCause::BelowRelevance {
                score: 0.1,
                floor: 0.3
            }),
            "the first recorded cause is the true one"
        );
    }

    #[test]
    fn reason_ids_are_stable_for_the_ui() {
        assert_eq!(qm("x").id(), "queryMatch");
        assert_eq!(
            Reason::GraphExpansion {
                from_id: "a".into(),
                from_title: "A".into(),
                hops: 1
            }
            .id(),
            "graphExpansion"
        );
        assert_eq!(
            Reason::RecentActivity { age_days: 1 }.id(),
            "recentActivity"
        );
        assert_eq!(
            Reason::HighImportance { importance: 0.9 }.id(),
            "highImportance"
        );
        assert_eq!(
            Reason::MemorySearch { query: "q".into() }.id(),
            "memorySearch"
        );
        assert_eq!(
            Reason::KeywordMatch {
                keyword: "k".into()
            }
            .id(),
            "keywordMatch"
        );
    }

    #[test]
    fn drop_cause_ids_are_stable_for_the_ui() {
        assert_eq!(
            DropCause::BelowRelevance {
                score: 0.0,
                floor: 0.3
            }
            .id(),
            "belowRelevance"
        );
        assert_eq!(DropCause::TokenBudget { limit: 1 }.id(), "tokenBudget");
        assert_eq!(DropCause::EntityCap { cap: 1 }.id(), "entityCap");
    }

    #[test]
    fn serialises_with_camel_case_and_a_tagged_kind() {
        let mut p = Provenance::new();
        p.record(
            "e1",
            ItemKind::Entity,
            "Alpha",
            Reason::GraphExpansion {
                from_id: "seed".into(),
                from_title: "Seed".into(),
                hops: 2,
            },
        );
        p.set_tokens("e1", 42);

        let json = serde_json::to_string(&p).unwrap();
        // The frontend depends on these exact spellings.
        assert!(json.contains("\"scoreParts\""), "got {json}");
        assert!(json.contains("\"kind\":\"graphExpansion\""), "got {json}");
        assert!(json.contains("\"fromTitle\":\"Seed\""), "got {json}");
        assert!(json.contains("\"tokens\":42"), "got {json}");

        let back: Provenance = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p, "round trip must be lossless");
    }

    #[test]
    fn tokens_default_to_zero_until_measured() {
        let mut p = Provenance::new();
        p.record("e1", ItemKind::Entity, "Alpha", qm("q"));
        assert_eq!(p.get("e1").unwrap().tokens, 0);
        p.set_tokens("e1", 10);
        assert_eq!(p.get("e1").unwrap().tokens, 10);
    }

    #[test]
    fn empty_provenance_reports_empty() {
        let p = Provenance::new();
        assert!(p.is_empty());
        assert_eq!(p.len(), 0);
        assert_eq!(p.included().count(), 0);
        assert_eq!(p.dropped().count(), 0);
        assert!(p.get("nope").is_none());
    }
}
