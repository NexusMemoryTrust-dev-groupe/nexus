//! Memory Rehearsal — spaced-repetition цикл повторения (Система 3).
//!
//! Как человеческая память, Nexus не хранит всё одинаково: память, которую
//! регулярно «освежают» (перечитывают, подтверждают, используют), укрепляется
//! и повторяется реже; память, которую не трогают, постепенно забывается —
//! её важность падает, и она перестаёт занимать место в контексте.
//!
//! Механика (адаптированный spaced repetition):
//! - у каждой памяти есть `next_rehearsal_at` — когда её нужно повторить;
//! - интервал до следующего повтора растёт с каждым повторением
//!   (`interval_days * 2^rehearsal_count`, с потолком), при этом важные памяти
//!   повторяются чаще (базовый интервал зависит от importance_score);
//! - `apply_rehearsal` отмечает повтор: счётчик +1, сдвиг расписания, лёгкое
//!   укрепление важности/уверенности;
//! - `build_rehearsal_plan` — чистая функция: какие памяти сейчас due;
//! - `sleep_cycle` — консолидация за «ночь»: укрепляет повторенные, мягко
//!   забывает долго неповторяемые и планирует первый повтор для новых.
//!
//! Всё вычисление — чистые функции над списком записей (`now` передаётся
//! параметром), поэтому модуль полностью юнит-тестируем без базы данных.

use chrono::{DateTime, Duration, Utc};

use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::types::MemoryState;

/// Config key holding the last rehearsal cycle timestamp (RFC3339).
pub const REHEARSAL_LAST_CYCLE_KEY: &str = "rehearsal.last_cycle_at";

/// A memory is only worth rehearsing if it is at least this important.
pub const REHEARSAL_IMPORTANCE_FLOOR: f64 = 0.35;

/// Smallest interval between two rehearsals (days).
pub const MIN_INTERVAL_DAYS: i64 = 3;

/// Largest interval we ever schedule (days).
pub const MAX_INTERVAL_DAYS: i64 = 90;

/// After this many rehearsals the interval stops growing (2^count cap).
pub const MAX_REHEARSALS_FOR_GROWTH: u32 = 5;

/// How much a rehearsal strengthens importance (capped at 1.0).
pub const REHEARSAL_IMPORTANCE_BOOST: f64 = 0.02;

/// How much a rehearsal strengthens confidence (capped at 1.0).
pub const REHEARSAL_CONFIDENCE_BOOST: f64 = 0.02;

/// Multiplier applied to importance of never-rehearsed old memories per decay
/// pass — «забывание». Importance never drops below the floor.
pub const FORGET_DECAY_FACTOR: f64 = 0.92;

/// A memory is "forgotten" when it is older than this and was never rehearsed.
pub const FORGET_NEVER_REHEARSED_DAYS: i64 = 90;

/// Something the plan says should be rehearsed right now.
#[derive(Debug, Clone)]
pub struct RehearsalItem {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub importance: f64,
    pub confidence: f64,
    pub rehearsal_count: u32,
    /// When the previous rehearsal happened (None = never rehearsed).
    pub last_rehearsed_at: Option<DateTime<Utc>>,
    /// When this memory is due (now or in the past).
    pub due_at: DateTime<Utc>,
    /// How overdue this is in days (0 = due right now).
    pub overdue_days: i64,
}

/// Aggregate counters for the whole pool.
#[derive(Debug, Clone, Default)]
pub struct RehearsalCounts {
    pub total: u64,
    pub due_now: u64,
    pub rehearsed_at_least_once: u64,
    pub never_rehearsed: u64,
    pub scheduled: u64,
}

/// The rehearsal plan: what to review, in what order, plus pool statistics.
#[derive(Debug, Clone)]
pub struct RehearsalPlan {
    pub generated_at: DateTime<Utc>,
    pub counts: RehearsalCounts,
    /// Due items, most important first (bounded by MAX_ITEMS).
    pub items: Vec<RehearsalItem>,
}

/// Maximum items returned in a plan (keeps the payload small).
pub const MAX_ITEMS: usize = 25;

/// Base interval between rehearsals for a memory of given importance (days).
///
/// Important memories are rehearsed more often: importance 1.0 → 3 days,
/// importance 0.5 → ~12 days, importance 0.0 → ~48 days.
pub fn base_interval_days(importance: f64) -> i64 {
    let clamped = importance.clamp(0.0, 1.0);
    let days = (1.0 - clamped) * 45.0 + MIN_INTERVAL_DAYS as f64;
    (days.round() as i64).clamp(MIN_INTERVAL_DAYS, MAX_INTERVAL_DAYS)
}

/// Interval before the *next* rehearsal after the given rehearsal count.
///
/// Spaced repetition: each successful rehearsal doubles the wait, but growth
/// stops after `MAX_REHEARSALS_FOR_GROWTH` and the result never exceeds
/// `MAX_INTERVAL_DAYS`.
pub fn next_interval_days(importance: f64, rehearsal_count: u32) -> i64 {
    let growth = rehearsal_count.min(MAX_REHEARSALS_FOR_GROWTH);
    let doubled = base_interval_days(importance) * 2i64.pow(growth);
    doubled.clamp(MIN_INTERVAL_DAYS, MAX_INTERVAL_DAYS)
}

/// When a memory should next be rehearsed, given its state and `now`.
///
/// - Never rehearsed → `created_at + base_interval` (first review is scheduled
///   from creation, so a fresh important memory is reviewed quickly).
/// - Rehearsed → `last_rehearsed_at + next_interval`.
pub fn next_rehearsal_at(record: &MemoryRecord, now: DateTime<Utc>) -> DateTime<Utc> {
    let anchor = record
        .last_rehearsed_at
        .unwrap_or_else(|| record.created_at.min(now));
    let interval = next_interval_days(record.importance_score, record.rehearsal_count);
    anchor + Duration::days(interval)
}

/// True when the memory should appear in the rehearsal plan right now.
///
/// A memory is due when its scheduled `next_rehearsal_at` is now or in the
/// past. Conflicted/Superseded records are never rehearsed — they have their
/// own lifecycle (resolve, not repeat).
pub fn is_due(record: &MemoryRecord, now: DateTime<Utc>) -> bool {
    if !matches!(
        record.memory_state,
        MemoryState::Current | MemoryState::UserConfirmed
    ) {
        return false;
    }
    if record.importance_score < REHEARSAL_IMPORTANCE_FLOOR {
        return false;
    }
    let next = record
        .next_rehearsal_at
        .unwrap_or_else(|| next_rehearsal_at(record, now));
    next <= now
}

/// Mark a memory as rehearsed: bump the counter, reschedule, strengthen.
///
/// The reschedule uses the *new* count (interval grows), so a memory that has
/// been rehearsed three times waits 2^3 = 8× longer before the next review.
/// Importance and confidence gain a small boost, capped at 1.0 — a rehearsed
/// memory is one the system keeps trusting.
pub fn apply_rehearsal(record: &mut MemoryRecord, now: DateTime<Utc>) {
    record.rehearsal_count += 1;
    record.last_rehearsed_at = Some(now);
    record.next_rehearsal_at = Some(
        now + Duration::days(next_interval_days(
            record.importance_score,
            record.rehearsal_count,
        )),
    );
    record.importance_score = (record.importance_score + REHEARSAL_IMPORTANCE_BOOST).min(1.0);
    record.confidence_score = (record.confidence_score + REHEARSAL_CONFIDENCE_BOOST).min(1.0);
    record.touch();
}

/// Schedule the first rehearsal for a fresh memory (called at creation).
///
/// Idempotent: if the memory already has a schedule (e.g. a migration or an
/// import restored it), it is left untouched.
pub fn schedule_first_rehearsal(record: &mut MemoryRecord, now: DateTime<Utc>) {
    if record.next_rehearsal_at.is_none() {
        record.next_rehearsal_at = Some(next_rehearsal_at(record, now));
    }
}

/// Build the rehearsal plan: what is due right now, most important first.
///
/// Pure function — no database access, fully unit-testable. `MAX_ITEMS` bounds
/// the actionable list; counters cover the whole pool.
pub fn build_rehearsal_plan(records: &[MemoryRecord], now: DateTime<Utc>) -> RehearsalPlan {
    let mut counts = RehearsalCounts {
        total: records.len() as u64,
        ..RehearsalCounts::default()
    };
    let mut items: Vec<RehearsalItem> = Vec::new();

    for r in records {
        if r.last_rehearsed_at.is_some() {
            counts.rehearsed_at_least_once += 1;
        } else {
            counts.never_rehearsed += 1;
        }
        if r.next_rehearsal_at.is_some() {
            counts.scheduled += 1;
        }
        if !is_due(r, now) {
            continue;
        }
        counts.due_now += 1;

        let due_at = r
            .next_rehearsal_at
            .unwrap_or_else(|| next_rehearsal_at(r, now));
        let overdue_days = (now - due_at).num_days().max(0);
        items.push(RehearsalItem {
            id: r.id.as_str().to_string(),
            title: r.title.clone(),
            summary: r.summary.clone(),
            importance: r.importance_score,
            confidence: r.confidence_score,
            rehearsal_count: r.rehearsal_count,
            last_rehearsed_at: r.last_rehearsed_at,
            due_at,
            overdue_days,
        });
    }

    // Most important first, then most overdue, then most recently touched.
    items.sort_by(|a, b| {
        b.importance
            .partial_cmp(&a.importance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.overdue_days.cmp(&a.overdue_days))
    });
    items.truncate(MAX_ITEMS);

    RehearsalPlan {
        generated_at: now,
        counts,
        items,
    }
}

/// The "sleep cycle": consolidate the pool in one pass.
///
/// 1. Schedule first rehearsals for memories that have none.
/// 2. Forget memories that are old and were never rehearsed: their importance
///    decays toward `REHEARSAL_IMPORTANCE_FLOOR` (they stop competing for
///    context space until someone reviews them).
///
/// Returns how many records were touched (scheduled or decayed).
pub fn sleep_cycle(records: &mut [MemoryRecord], now: DateTime<Utc>) -> u64 {
    let mut touched: u64 = 0;

    for r in records.iter_mut() {
        let had_schedule = r.next_rehearsal_at.is_some();
        schedule_first_rehearsal(r, now);
        if !had_schedule {
            touched += 1;
        }

        let never_rehearsed = r.last_rehearsed_at.is_none();
        let old = (now - r.created_at).num_days() >= FORGET_NEVER_REHEARSED_DAYS;
        let can_forget = matches!(
            r.memory_state,
            MemoryState::Current | MemoryState::UserConfirmed | MemoryState::Inferred
        );
        if never_rehearsed && old && can_forget {
            let decayed =
                (r.importance_score * FORGET_DECAY_FACTOR).max(REHEARSAL_IMPORTANCE_FLOOR);
            if (decayed - r.importance_score).abs() > f64::EPSILON {
                r.importance_score = decayed;
                r.touch();
                touched += 1;
            }
        }
    }

    touched
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::memory::memory_record::MemoryRecord;
    use crate::core::memory::types::MemorySource;

    fn record(title: &str, importance: f64, age_days: i64) -> MemoryRecord {
        let mut r = MemoryRecord::new(
            title.to_string(),
            "content".to_string(),
            "tester".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        r.importance_score = importance;
        r.created_at = Utc::now() - Duration::days(age_days);
        r
    }

    #[test]
    fn base_interval_scales_with_importance() {
        assert!(base_interval_days(1.0) <= base_interval_days(0.5));
        assert!(base_interval_days(0.5) <= base_interval_days(0.0));
        // Boundaries are clamped.
        assert_eq!(base_interval_days(2.0), MIN_INTERVAL_DAYS);
        assert!(base_interval_days(0.0) <= MAX_INTERVAL_DAYS);
    }

    #[test]
    fn interval_grows_with_rehearsals() {
        let b = base_interval_days(0.8);
        let once = next_interval_days(0.8, 1);
        let thrice = next_interval_days(0.8, 3);
        assert!(once > b);
        assert!(thrice > once);
        // Growth caps: 2^5 = 32× would blow the ceiling.
        assert!(next_interval_days(0.8, 100) <= MAX_INTERVAL_DAYS);
    }

    #[test]
    fn fresh_memory_is_scheduled_from_creation() {
        let mut r = record("Fresh", 0.9, 0);
        let now = Utc::now();
        schedule_first_rehearsal(&mut r, now);
        let due = next_rehearsal_at(&r, now);
        assert!(r.next_rehearsal_at.is_some());
        // Fresh important memory is due one base interval after creation. The
        // anchor is created_at, which is a few microseconds before `now`, so
        // allow that sub-millisecond drift instead of demanding exact equality.
        let expected = now + Duration::days(base_interval_days(0.9));
        assert!(due <= expected);
        assert!(
            expected - due < Duration::seconds(1),
            "first review is scheduled from creation with the base interval"
        );
    }

    #[test]
    fn apply_rehearsal_bumps_count_and_strengthens() {
        let mut r = record("Important", 0.8, 30);
        let now = Utc::now();
        r.next_rehearsal_at = Some(now - Duration::days(1));

        let importance_before = r.importance_score;
        let confidence_before = r.confidence_score;
        apply_rehearsal(&mut r, now);

        assert_eq!(r.rehearsal_count, 1);
        assert!(r.last_rehearsed_at.is_some());
        assert!(r.importance_score >= importance_before);
        assert!(r.confidence_score >= confidence_before);
        // Next review is in the future, farther than the base interval.
        let next = r.next_rehearsal_at.unwrap();
        assert!(next > now + Duration::days(base_interval_days(0.8)));
        assert_eq!(r.version, 2, "rehearsal must touch the record");
    }

    #[test]
    fn importance_boost_is_capped_at_one() {
        let mut r = record("Perfect", 0.999, 10);
        apply_rehearsal(&mut r, Utc::now());
        assert!(r.importance_score <= 1.0);
    }

    #[test]
    fn is_due_requires_importance_and_current_state() {
        let now = Utc::now();

        let mut important = record("Due", 0.9, 0);
        important.next_rehearsal_at = Some(now - Duration::days(1));
        assert!(is_due(&important, now));

        // Below the floor: never surfaced for rehearsal.
        let mut trivial = record("Trivial", 0.2, 0);
        trivial.next_rehearsal_at = Some(now - Duration::days(1));
        assert!(!is_due(&trivial, now));

        // Conflicted memories resolve, they don't rehearse.
        let mut conflicted = record("Conflicted", 0.9, 0);
        conflicted.memory_state = MemoryState::Conflicted;
        conflicted.next_rehearsal_at = Some(now - Duration::days(1));
        assert!(!is_due(&conflicted, now));

        // Not yet due.
        let mut future = record("Future", 0.9, 0);
        future.next_rehearsal_at = Some(now + Duration::days(10));
        assert!(!is_due(&future, now));
    }

    #[test]
    fn plan_lists_due_items_most_important_first() {
        let now = Utc::now();

        let mut a = record("A", 0.9, 5);
        a.next_rehearsal_at = Some(now - Duration::days(1));
        let mut b = record("B", 0.7, 5);
        b.next_rehearsal_at = Some(now - Duration::days(2));
        let mut not_due = record("C", 0.95, 5);
        not_due.next_rehearsal_at = Some(now + Duration::days(5));

        let plan = build_rehearsal_plan(&[not_due, b, a], now);
        assert_eq!(plan.counts.total, 3);
        assert_eq!(plan.counts.due_now, 2);
        assert_eq!(plan.items.len(), 2);
        assert_eq!(plan.items[0].title, "A", "most important due item first");
        assert_eq!(plan.items[0].overdue_days, 1);
        assert!(plan.items[0].overdue_days <= plan.items[1].overdue_days);
    }

    #[test]
    fn never_rehearsed_old_memories_decay_in_sleep_cycle() {
        let now = Utc::now();
        let old = record("Old", 0.9, FORGET_NEVER_REHEARSED_DAYS + 10);
        let new = record("New", 0.9, 5);
        let mut rehearsed_old = record("Kept", 0.9, FORGET_NEVER_REHEARSED_DAYS + 10);
        rehearsed_old.last_rehearsed_at = Some(now - Duration::days(1));

        let importance_before = old.importance_score;
        let mut records = vec![old, new, rehearsed_old];
        let touched = sleep_cycle(&mut records, now);

        assert!(
            touched >= 3,
            "all three should be scheduled/decayed at least once"
        );
        assert!(
            records[0].importance_score < importance_before,
            "never-rehearsed old memory must be forgotten (importance drops)"
        );
        assert_eq!(
            records[1].importance_score, 0.9,
            "fresh memory is not forgotten"
        );
        assert_eq!(
            records[2].importance_score, 0.9,
            "rehearsed memory is not forgotten"
        );
        // Decay never drops below the floor.
        for r in &records {
            assert!(r.importance_score >= REHEARSAL_IMPORTANCE_FLOOR);
        }
        // Everyone got a schedule.
        for r in &records {
            assert!(r.next_rehearsal_at.is_some());
        }
    }
}
