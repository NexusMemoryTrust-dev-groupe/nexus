//! Memory Radar — proactive recall layer (Фаза 1 флагмана).
//!
//! Instead of waiting for a query, the radar scans the memory pool and answers
//! "what needs my attention right now?": unresolved conflicts, expiring
//! memories, unconfirmed inferences, and what changed since the last scan.
//!
//! The scan checkpoint is stored in `configuration_kv` under the key
//! `radar.last_seen_at` (RFC3339). `build_snapshot` is a pure function over
//! the record list so it is unit-testable without a database.

use chrono::{DateTime, Duration, Utc};

use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::types::MemoryState;

/// Config key holding the timestamp of the last radar scan (RFC3339).
pub const RADAR_LAST_SEEN_KEY: &str = "radar.last_seen_at";

/// A memory is "expiring" when its expires_at is within this window.
pub const EXPIRY_WINDOW_DAYS: i64 = 7;

/// Memories below this importance are never surfaced as "new" noise.
pub const MIN_SURFACE_IMPORTANCE: f64 = 0.6;

/// Maximum items returned in a snapshot (keeps the payload small).
pub const MAX_ITEMS: usize = 25;

/// What the radar wants a human to do with an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadarAction {
    /// Resolve the conflict — two memories say different things.
    Resolve,
    /// Re-check / extend before the expiry date passes.
    Recheck,
    /// Confirm or reject an inferred memory.
    Confirm,
    /// Review what is new or changed.
    Review,
}

impl RadarAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            RadarAction::Resolve => "resolve",
            RadarAction::Recheck => "recheck",
            RadarAction::Confirm => "confirm",
            RadarAction::Review => "review",
        }
    }
}

/// A single thing the radar surfaced.
#[derive(Debug, Clone)]
pub struct RadarItem {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub action: RadarAction,
    pub importance: f64,
    pub confidence: f64,
    pub memory_state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    /// Human-readable reason why this item is on the radar.
    pub reason: String,
}

/// Aggregate counters for the whole pool (not just surfaced items).
#[derive(Debug, Clone, Default)]
pub struct RadarCounts {
    pub total: u64,
    pub new_since_last_scan: u64,
    pub updated_since_last_scan: u64,
    pub conflicted: u64,
    pub superseded: u64,
    pub inferred: u64,
    pub expiring: u64,
    pub unconfirmed: u64,
}

/// The full radar output: what changed, what is broken, what needs a decision.
#[derive(Debug, Clone)]
pub struct RadarSnapshot {
    pub generated_at: DateTime<Utc>,
    /// When the previous scan happened (None on first run).
    pub since: Option<DateTime<Utc>>,
    pub counts: RadarCounts,
    /// Actionable items, highest priority first (bounded by MAX_ITEMS).
    pub items: Vec<RadarItem>,
    /// 0–100: how much of the user's attention the pool currently demands.
    pub attention_score: u8,
}

/// Build the radar snapshot from a memory list.
///
/// `since` is the previous scan checkpoint; items created/updated after it are
/// flagged as new/changed. Pass `None` for a first run — nothing is "new"
/// relative to nothing, but structural problems (conflicts, expiring,
/// unconfirmed) are still surfaced.
pub fn build_snapshot(records: &[MemoryRecord], since: Option<DateTime<Utc>>) -> RadarSnapshot {
    let now = Utc::now();
    let mut counts = RadarCounts {
        total: records.len() as u64,
        ..RadarCounts::default()
    };
    let mut items: Vec<RadarItem> = Vec::new();

    for r in records {
        match r.memory_state {
            MemoryState::Conflicted => counts.conflicted += 1,
            MemoryState::Superseded => counts.superseded += 1,
            MemoryState::Inferred => counts.inferred += 1,
            _ => {}
        }
        if r.confirmed_at.is_none() {
            counts.unconfirmed += 1;
        }
        if let Some(exp) = r.expires_at
            && exp <= now + Duration::days(EXPIRY_WINDOW_DAYS)
        {
            counts.expiring += 1;
        }

        let is_new = since.is_some_and(|s| r.created_at > s);
        let is_updated = !is_new && since.is_some_and(|s| r.updated_at > s);
        if is_new {
            counts.new_since_last_scan += 1;
        }
        if is_updated {
            counts.updated_since_last_scan += 1;
        }

        // ── What deserves a slot in the actionable list? ──
        // Conflicts always surface; expiring/unconfirmed surface unless trivia;
        // new/updated surface only when the item is important enough.
        let candidate = if r.memory_state == MemoryState::Conflicted {
            Some((
                RadarAction::Resolve,
                "Conflicted: two memories disagree — resolve which one is true",
            ))
        } else if r
            .expires_at
            .is_some_and(|exp| exp <= now + Duration::days(EXPIRY_WINDOW_DAYS))
        {
            Some((
                RadarAction::Recheck,
                "Expiring soon: re-check or extend before the date passes",
            ))
        } else if r.memory_state == MemoryState::Inferred && r.confirmed_at.is_none() {
            Some((
                RadarAction::Confirm,
                "Inferred but never confirmed by a human",
            ))
        } else if is_new && r.importance_score >= MIN_SURFACE_IMPORTANCE {
            Some((RadarAction::Review, "New since the last radar scan"))
        } else if is_updated && r.importance_score >= MIN_SURFACE_IMPORTANCE {
            Some((RadarAction::Review, "Changed since the last radar scan"))
        } else {
            None
        };

        if let Some((action, reason)) = candidate {
            items.push(RadarItem {
                id: r.id.as_str().to_string(),
                title: r.title.clone(),
                summary: r.summary.clone(),
                action,
                importance: r.importance_score,
                confidence: r.confidence_score,
                memory_state: r.memory_state.as_str().to_string(),
                created_at: r.created_at,
                updated_at: r.updated_at,
                expires_at: r.expires_at,
                reason: reason.to_string(),
            });
        }
    }

    // Priority: conflicts first, then rechecks, then confirms, then reviews;
    // within a group, higher importance first, then more recent.
    let priority = |a: &RadarItem| -> u8 {
        match a.action {
            RadarAction::Resolve => 0,
            RadarAction::Recheck => 1,
            RadarAction::Confirm => 2,
            RadarAction::Review => 3,
        }
    };
    items.sort_by(|a, b| {
        priority(a)
            .cmp(&priority(b))
            .then(
                b.importance
                    .partial_cmp(&a.importance)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(b.updated_at.cmp(&a.updated_at))
    });
    items.truncate(MAX_ITEMS);

    // Attention score: weighted urgency, capped at 100.
    let score = (counts.conflicted * 15
        + counts.expiring * 8
        + counts.inferred.saturating_sub(0) * 5
        + counts.new_since_last_scan * 2
        + counts.updated_since_last_scan) as u8;
    let attention_score = score.min(100);

    RadarSnapshot {
        generated_at: now,
        since,
        counts,
        items,
        attention_score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::memory_record::MemoryRecord;
    use crate::core::memory::types::MemorySource;

    fn record(title: &str, importance: f64) -> MemoryRecord {
        let mut r = MemoryRecord::new(
            title.to_string(),
            "content".to_string(),
            "tester".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        r.importance_score = importance;
        r
    }

    #[test]
    fn empty_pool_produces_empty_snapshot() {
        let snap = build_snapshot(&[], None);
        assert_eq!(snap.counts.total, 0);
        assert!(snap.items.is_empty());
        assert!(snap.since.is_none());
        assert_eq!(snap.attention_score, 0);
    }

    #[test]
    fn conflict_surfaces_regardless_of_since() {
        let mut r = record("Decision A", 0.5);
        r.memory_state = MemoryState::Conflicted;
        let snap = build_snapshot(&[r], None);
        assert_eq!(snap.counts.conflicted, 1);
        assert_eq!(snap.items.len(), 1);
        assert_eq!(snap.items[0].action, RadarAction::Resolve);
    }

    #[test]
    fn expiring_surfaces_with_recheck_action() {
        let mut r = record("Licence renewal", 0.5);
        r.expires_at = Some(Utc::now() + Duration::days(2));
        let snap = build_snapshot(&[r], None);
        assert_eq!(snap.counts.expiring, 1);
        assert_eq!(snap.items[0].action, RadarAction::Recheck);
    }

    #[test]
    fn non_expiring_does_not_surface_as_recheck() {
        let mut r = record("Long term", 0.5);
        r.expires_at = Some(Utc::now() + Duration::days(90));
        let snap = build_snapshot(&[r], None);
        assert_eq!(snap.counts.expiring, 0);
        assert!(snap.items.is_empty());
    }

    #[test]
    fn inferred_unconfirmed_surfaces_as_confirm() {
        let mut r = record("Guess", 0.5);
        r.memory_state = MemoryState::Inferred;
        let snap = build_snapshot(&[r], None);
        assert_eq!(snap.items[0].action, RadarAction::Confirm);
    }

    #[test]
    fn confirmed_inferred_does_not_surface() {
        let mut r = record("Guess", 0.5);
        r.memory_state = MemoryState::Inferred;
        r.confirmed_at = Some(Utc::now());
        let snap = build_snapshot(&[r], None);
        assert!(snap.items.is_empty());
    }

    #[test]
    fn new_important_surfaces_with_since() {
        let since = Utc::now() - Duration::days(1);
        let mut r = record("Fresh decision", 0.9);
        r.created_at = Utc::now();
        let snap = build_snapshot(&[r], Some(since));
        assert_eq!(snap.counts.new_since_last_scan, 1);
        assert_eq!(snap.items[0].action, RadarAction::Review);
    }

    #[test]
    fn new_trivial_does_not_surface() {
        let since = Utc::now() - Duration::days(1);
        let mut r = record("Trivia", 0.2);
        r.created_at = Utc::now();
        let snap = build_snapshot(&[r], Some(since));
        assert_eq!(snap.counts.new_since_last_scan, 1);
        assert!(snap.items.is_empty());
    }

    #[test]
    fn nothing_new_when_first_run_with_since_none() {
        let r = record("Old", 0.9);
        let snap = build_snapshot(&[r], None);
        assert_eq!(snap.counts.new_since_last_scan, 0);
        assert!(snap.items.is_empty());
    }

    #[test]
    fn priority_orders_conflicts_first() {
        let since = Utc::now() - Duration::days(1);

        let mut conflict = record("Conflict", 0.3);
        conflict.memory_state = MemoryState::Conflicted;

        let mut expiring = record("Expiring", 0.9);
        expiring.expires_at = Some(Utc::now() + Duration::days(1));

        let mut fresh = record("Fresh", 0.9);
        fresh.created_at = Utc::now();

        let snap = build_snapshot(&[fresh, expiring, conflict], Some(since));
        assert_eq!(snap.items[0].action, RadarAction::Resolve);
        assert_eq!(snap.items[1].action, RadarAction::Recheck);
        assert_eq!(snap.items[2].action, RadarAction::Review);
    }

    #[test]
    fn attention_score_caps_at_100() {
        let mut records = Vec::new();
        for i in 0..30 {
            let mut r = record(&format!("Conflict {}", i), 0.9);
            r.memory_state = MemoryState::Conflicted;
            records.push(r);
        }
        let snap = build_snapshot(&records, None);
        assert_eq!(snap.attention_score, 100);
        assert_eq!(snap.counts.conflicted, 30);
        assert_eq!(snap.items.len(), MAX_ITEMS);
    }

    #[test]
    fn updated_since_last_scan_counted() {
        let since = Utc::now() - Duration::days(1);
        let mut r = record("Touched", 0.9);
        r.created_at = Utc::now() - Duration::days(10);
        r.updated_at = Utc::now();
        let snap = build_snapshot(&[r], Some(since));
        assert_eq!(snap.counts.updated_since_last_scan, 1);
        assert_eq!(snap.counts.new_since_last_scan, 0);
        assert_eq!(snap.items[0].action, RadarAction::Review);
    }
}
