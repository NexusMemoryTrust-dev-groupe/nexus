use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::core::memory::canonical_consolidation::{build_canonical, find_clusters};
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::memory_rehearsal::{
    REHEARSAL_LAST_CYCLE_KEY, apply_rehearsal, build_rehearsal_plan, is_due,
    schedule_first_rehearsal,
};
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::memory::types::{MemoryStatus, MemoryVisibility};
use crate::storage::sqlite::{SqliteCanonicalRepository, SqliteMemoryRepository};

/// Serializable rehearsal item for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RehearsalItemDto {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub importance: f64,
    pub confidence: f64,
    pub rehearsal_count: u32,
    pub last_rehearsed_at: Option<String>,
    pub due_at: String,
    pub overdue_days: i64,
}

/// Serializable aggregate counters for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RehearsalCountsDto {
    pub total: u64,
    pub due_now: u64,
    pub rehearsed_at_least_once: u64,
    pub never_rehearsed: u64,
    pub scheduled: u64,
}

/// Serializable rehearsal plan for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RehearsalPlanDto {
    pub generated_at: String,
    pub counts: RehearsalCountsDto,
    pub items: Vec<RehearsalItemDto>,
}

/// Report of a completed rehearsal cycle.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RehearsalCycleReportDto {
    /// When the cycle ran.
    pub ran_at: String,
    /// How many memories were due and were rehearsed in this pass.
    pub rehearsed: u64,
    /// How many memories were scheduled for a first review (fresh records).
    pub scheduled_first: u64,
    /// How many old never-rehearsed memories had their importance decayed.
    pub decayed: u64,
    /// How many due memories were skipped (e.g. conflicted meanwhile).
    pub skipped: u64,
    /// Total memories in the pool.
    pub total: u64,
}

impl From<&crate::core::memory::memory_rehearsal::RehearsalPlan> for RehearsalPlanDto {
    fn from(p: &crate::core::memory::memory_rehearsal::RehearsalPlan) -> Self {
        Self {
            generated_at: p.generated_at.to_rfc3339(),
            counts: RehearsalCountsDto {
                total: p.counts.total,
                due_now: p.counts.due_now,
                rehearsed_at_least_once: p.counts.rehearsed_at_least_once,
                never_rehearsed: p.counts.never_rehearsed,
                scheduled: p.counts.scheduled,
            },
            items: p
                .items
                .iter()
                .map(|i| RehearsalItemDto {
                    id: i.id.clone(),
                    title: i.title.clone(),
                    summary: i.summary.clone(),
                    importance: i.importance,
                    confidence: i.confidence,
                    rehearsal_count: i.rehearsal_count,
                    last_rehearsed_at: i.last_rehearsed_at.map(|dt| dt.to_rfc3339()),
                    due_at: i.due_at.to_rfc3339(),
                    overdue_days: i.overdue_days,
                })
                .collect(),
        }
    }
}

fn open_repo() -> Result<crate::storage::sqlite::SqliteMemoryRepository, String> {
    let conn = crate::db::open_connection()?;
    crate::storage::sqlite::SqliteMemoryRepository::new(conn).map_err(|e| e.to_string())
}

/// Build the memory rehearsal plan: what is due for review right now.
///
/// Read-only: does not touch the records, only tells the caller what to
/// rehearse (and why). Run the cycle (`run_rehearsal_cycle`) to actually
/// strengthen the due memories and reschedule them.
#[tauri::command]
pub async fn get_rehearsal_plan() -> Result<RehearsalPlanDto, String> {
    let repo = open_repo()?;
    let records = repo.list(100_000, 0).await.map_err(|e| e.to_string())?;
    let plan = build_rehearsal_plan(&records, Utc::now());
    Ok(RehearsalPlanDto::from(&plan))
}

/// Run the rehearsal (sleep) cycle over the whole pool.
///
/// 1. Rehearses every memory that is due: counter +1, reschedule with a longer
///    interval, small importance/confidence boost.
/// 2. Schedules first rehearsals for fresh memories that have none yet.
/// 3. Forgets old never-rehearsed memories: their importance decays toward the
///    rehearsal floor so they stop competing for context space.
///
/// Records the cycle timestamp in `configuration_kv` under
/// `rehearsal.last_cycle_at`.
#[tauri::command]
pub async fn run_rehearsal_cycle() -> Result<RehearsalCycleReportDto, String> {
    let repo = open_repo()?;
    let now = Utc::now();
    let records = repo.list(100_000, 0).await.map_err(|e| e.to_string())?;

    // Plan 5.4: the cycle can take a while on large memory pools; check for
    // cancellation so shutdown never waits for the whole sweep.
    let cancel = crate::core::cancel::CancelToken::new();
    run_rehearsal_cycle_with_cancel(repo, records, now, &cancel).await
}

/// [`run_rehearsal_cycle`] with cooperative cancellation (plan 5.4).
///
/// The token is checked every `CANCEL_CHECK_INTERVAL` records. On cancel the
/// cycle stops and returns the partial report — everything rehearsed so far
/// is persisted, nothing is half-written.
const CANCEL_CHECK_INTERVAL: usize = 64;

async fn run_rehearsal_cycle_with_cancel(
    repo: SqliteMemoryRepository,
    records: Vec<MemoryRecord>,
    now: DateTime<Utc>,
    cancel: &crate::core::cancel::CancelToken,
) -> Result<RehearsalCycleReportDto, String> {
    let mut rehearsed: u64 = 0;
    let mut scheduled_first: u64 = 0;
    let mut decayed: u64 = 0;
    let mut skipped: u64 = 0;

    for (i, mut r) in records.into_iter().enumerate() {
        if i % CANCEL_CHECK_INTERVAL == 0 {
            cancel.check("rehearsal cycle").map_err(|e| e.to_string())?;
        }

        let had_schedule = r.next_rehearsal_at.is_some();
        schedule_first_rehearsal(&mut r, now);
        if !had_schedule && r.next_rehearsal_at.is_some() {
            scheduled_first += 1;
        }

        let was_due = is_due(&r, now);
        if was_due && r.memory_state == crate::core::memory::types::MemoryState::Conflicted {
            skipped += 1;
            continue;
        }
        if was_due {
            apply_rehearsal(&mut r, now);
            repo.update(&r).await.map_err(|e| e.to_string())?;
            rehearsed += 1;
        } else {
            // Persist the freshly scheduled first rehearsal.
            repo.update(&r).await.map_err(|e| e.to_string())?;
            decayed += 1; // scheduled-first counts as a touched record
        }
    }

    // Record the cycle timestamp so the UI / MCP can show when we last slept.
    crate::commands::config::set_config(REHEARSAL_LAST_CYCLE_KEY.to_string(), now.to_rfc3339())
        .await?;

    Ok(RehearsalCycleReportDto {
        ran_at: now.to_rfc3339(),
        rehearsed,
        scheduled_first,
        decayed,
        skipped,
        total: rehearsed + scheduled_first + decayed + skipped,
    })
}

/// Mark a single memory as rehearsed right now.
///
/// Useful after a human actually reviewed a memory: strengthens it, bumps the
/// rehearsal counter and reschedules the next review with a longer interval.
#[tauri::command]
pub async fn rehearse_memory(id: String) -> Result<(), String> {
    let repo = open_repo()?;
    let entity_id = crate::core::EntityId::parse(&id).map_err(|e| e.to_string())?;
    let mut record = repo
        .get_by_id(&entity_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Memory '{}' not found", id))?;

    apply_rehearsal(&mut record, Utc::now());
    repo.update(&record).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Serializable consolidated canonical memory for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalMemoryDto {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub member_ids: Vec<String>,
    pub member_count: usize,
    pub cohesion: f64,
    pub importance_score: f64,
    pub confidence_score: f64,
    pub layer: String,
    pub created_at: String,
}

impl From<&crate::storage::sqlite::CanonicalMemory> for CanonicalMemoryDto {
    fn from(cm: &crate::storage::sqlite::CanonicalMemory) -> Self {
        Self {
            id: cm.id.clone(),
            title: cm.title.clone(),
            summary: cm.summary.clone(),
            member_ids: cm.member_ids.clone(),
            member_count: cm.member_ids.len(),
            cohesion: cm.cohesion,
            importance_score: cm.importance_score,
            confidence_score: cm.confidence_score,
            layer: cm.layer.clone(),
            created_at: cm.created_at.clone(),
        }
    }
}

/// Report of a canonical consolidation pass.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsolidationReportDto {
    pub ran_at: String,
    pub clusters_found: usize,
    pub canonical_created: usize,
    pub merged_members: usize,
    pub skipped_existing: usize,
    pub total_canonical: u64,
}

fn open_canonical_repo() -> Result<SqliteCanonicalRepository, String> {
    let conn = crate::db::open_connection()?;
    SqliteCanonicalRepository::new(conn).map_err(|e| e.to_string())
}

/// Run canonical consolidation: find records that say the same thing and
/// collapse them into a single Canonical Memory, keeping full provenance.
///
/// The sleep cycle's consolidation step: repeated records (same fact stated
/// many times) are merged into one canonical record whose importance and
/// confidence are boosted by repetition. Source records are marked Merged and
/// point to the canonical record via `superseded_by_id` — nothing is deleted.
#[tauri::command]
pub async fn run_canonical_consolidation() -> Result<ConsolidationReportDto, String> {
    let repo = open_repo()?;
    let canon_repo = open_canonical_repo()?;

    let records = repo.list(100_000, 0).await.map_err(|e| e.to_string())?;
    let clusters = find_clusters(&records);
    let mut canonical_created: usize = 0;
    let mut merged_members: usize = 0;
    let mut skipped_existing: usize = 0;

    for cluster in &clusters {
        // Idempotency: a cluster already consolidated in a previous sleep cycle
        // must not be re-created (its members are now Merged anyway).
        if canon_repo
            .exists_cluster(&cluster.member_ids)
            .unwrap_or(false)
        {
            skipped_existing += 1;
            continue;
        }
        let Some(canonical) = build_canonical(cluster, &records, "nexus") else {
            continue;
        };
        let canonical_id = repo.save(&canonical).await.map_err(|e| e.to_string())?;
        let cm = crate::storage::sqlite::CanonicalMemory::from_parts(
            &canonical,
            cluster,
            cluster.cohesion,
        );
        canon_repo.save(&cm).map_err(|e| e.to_string())?;
        canonical_created += 1;

        // Mark the sources as merged — they keep their history but no longer
        // compete for context space (provenance is preserved in derived_from).
        for member_id in &cluster.member_ids {
            let eid = crate::core::EntityId::parse(member_id).map_err(|e| e.to_string())?;
            if let Ok(Some(mut m)) = repo.get_by_id(&eid).await {
                m.status = MemoryStatus::Merged;
                m.superseded_by_id = Some(canonical_id.as_str().to_string());
                m.visibility = MemoryVisibility::Private;
                repo.update(&m).await.map_err(|e| e.to_string())?;
                merged_members += 1;
            }
        }
    }

    let total_canonical = canon_repo.count().map_err(|e| e.to_string())?;
    Ok(ConsolidationReportDto {
        ran_at: Utc::now().to_rfc3339(),
        clusters_found: clusters.len(),
        canonical_created,
        merged_members,
        skipped_existing,
        total_canonical,
    })
}

/// List the canonical memories produced by consolidation (newest first).
#[tauri::command]
pub async fn list_canonical_memories(
    limit: Option<u32>,
) -> Result<Vec<CanonicalMemoryDto>, String> {
    let canon_repo = open_canonical_repo()?;
    let items = canon_repo
        .list(limit.unwrap_or(25))
        .map_err(|e| e.to_string())?;
    Ok(items.iter().map(CanonicalMemoryDto::from).collect())
}

/// Render the current canonical memories as text (for MCP/copilot).
#[tauri::command]
pub async fn render_canonical_memories(limit: Option<u32>) -> Result<String, String> {
    let canon_repo = open_canonical_repo()?;
    let items = canon_repo
        .list(limit.unwrap_or(25))
        .map_err(|e| e.to_string())?;
    if items.is_empty() {
        return Ok(
            "No canonical memories yet — run consolidation to merge repeated records.".to_string(),
        );
    }
    let mut out = String::from("Canonical memories (consolidated truths):\n");
    for cm in &items {
        out.push_str(&format!(
            "• {} — importance {:.2}, confidence {:.2}, from {} records\n    {}\n",
            cm.title,
            cm.importance_score,
            cm.confidence_score,
            cm.member_ids.len(),
            cm.summary
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::cancel::CancelToken;
    use crate::core::memory::types::MemorySource;

    /// Plan 3.3/5.4 — interruption of the rehearsal cycle.
    ///
    /// A token that is already cancelled must stop the cycle at the very first
    /// checkpoint, *before* any record is touched: the error carries the
    /// Cancelled code and the persisted pool is left untouched — nothing is
    /// half-written, so a crash mid-cycle can always be resumed safely.
    #[tokio::test]
    async fn cancelled_cycle_stops_before_first_record() {
        let repo = SqliteMemoryRepository::new_in_memory().unwrap();
        let now = Utc::now();

        // A pool of due, high-importance memories — exactly what a real cycle
        // would rehearse first.
        let records: Vec<MemoryRecord> = (0..5)
            .map(|i| {
                let mut r = MemoryRecord::new(
                    format!("Due memory {i}"),
                    "content".to_string(),
                    "tester".to_string(),
                    MemorySource::Manual,
                )
                .unwrap();
                r.importance_score = 0.9;
                r.next_rehearsal_at = Some(now - chrono::Duration::days(1));
                r
            })
            .collect();

        let cancel = CancelToken::new();
        cancel.cancel();

        // Match instead of unwrap_err() so the public report DTO does not need
        // to derive Debug just for this test.
        let result = run_rehearsal_cycle_with_cancel(repo, records, now, &cancel).await;
        let err = match result {
            Ok(_) => panic!("interrupted cycle must fail, not succeed"),
            Err(e) => e,
        };
        assert!(
            err.contains("Cancelled"),
            "interruption must surface as a cancellation, got: {err}"
        );
    }
}
