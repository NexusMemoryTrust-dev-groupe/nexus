use chrono::Utc;
use serde::Serialize;

use crate::core::entity_id::EntityId;
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::memory::types::{MemoryFeedback, MemorySource, MemoryState};

use super::memory::MemoryDto;

fn open_repo() -> Result<crate::storage::sqlite::SqliteMemoryRepository, String> {
    let conn = crate::db::open_connection()?;
    crate::storage::sqlite::SqliteMemoryRepository::new(conn).map_err(|e| e.to_string())
}

/// Summary of the memory trust lifecycle: how many memories are in each state.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleOverview {
    pub current: u64,
    pub user_confirmed: u64,
    pub inferred: u64,
    pub superseded: u64,
    pub conflicted: u64,
    pub total: u64,
}

/// Aggregate feedback counters across all memories.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackSummary {
    pub useful: u64,
    pub irrelevant: u64,
    pub wrong: u64,
    pub total_feedback: u64,
}

/// Set the trust state of a memory explicitly (Current / Inferred / Superseded / Conflicted).
#[tauri::command]
pub async fn memory_set_state(id: String, state: String) -> Result<MemoryDto, String> {
    let repo = open_repo()?;
    let entity_id = EntityId::parse(&id).map_err(|e| e.to_string())?;
    let mut record = repo
        .get_by_id(&entity_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Memory {} not found", id))?;

    let new_state = MemoryState::parse(&state);
    record.memory_state = new_state;
    record.touch();
    repo.update(&record).await.map_err(|e| e.to_string())?;
    Ok(MemoryDto::from(record))
}

/// Mark a memory as explicitly confirmed by a human.
#[tauri::command]
pub async fn memory_confirm(id: String, by: Option<String>) -> Result<MemoryDto, String> {
    let repo = open_repo()?;
    let entity_id = EntityId::parse(&id).map_err(|e| e.to_string())?;
    let mut record = repo
        .get_by_id(&entity_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Memory {} not found", id))?;

    record.memory_state = MemoryState::UserConfirmed;
    record.confirmed_at = Some(Utc::now());
    record.confirmed_by = by.or(record.confirmed_by);
    record.touch();
    repo.update(&record).await.map_err(|e| e.to_string())?;
    Ok(MemoryDto::from(record))
}

/// Record user feedback: "useful" | "irrelevant" | "wrong".
///
/// One-vote-per-memory logic: the first click registers the vote and records
/// `voted = kind`; clicking the same kind again removes the vote (counter
/// decremented, `voted = None`); clicking a different kind switches the vote
/// (old counter decremented, new incremented). Counters can therefore never
/// grow from repeated clicks — each kind counts at most one vote from the user.
///
/// `note` is optional free text explaining *why*. When present it is stored in
/// `feedback.note` (the counter is left untouched) and the memory is re-indexed
/// so the copilot / semantic search can use the explanation.
#[tauri::command]
pub async fn memory_feedback(
    id: String,
    kind: String,
    note: Option<String>,
) -> Result<MemoryDto, String> {
    let repo = open_repo()?;
    let entity_id = EntityId::parse(&id).map_err(|e| e.to_string())?;
    let mut record = repo
        .get_by_id(&entity_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Memory {} not found", id))?;

    if !matches!(kind.as_str(), "useful" | "irrelevant" | "wrong") {
        return Err(format!(
            "Unknown feedback kind: {} (expected useful|irrelevant|wrong)",
            kind
        ));
    }

    // A note alone updates the explanation without touching the vote counters,
    // so the UI can vote first and then send the "why" in a second call.
    if let Some(text) = note.as_deref().map(str::trim)
        && !text.is_empty()
    {
        record.feedback.note = Some(text.to_string());
        record.touch();
        repo.update(&record).await.map_err(|e| e.to_string())?;

        // Re-index so the explanation becomes part of semantic context.
        spawn_index_with_note(&record);
        return Ok(MemoryDto::from(record));
    }

    // One-vote toggle: switch / remove / register. Compute the full new
    // counter state first (no overlapping borrows), then assign.
    let mut useful = record.feedback.useful;
    let mut irrelevant = record.feedback.irrelevant;
    let mut wrong = record.feedback.wrong;
    let voted = record.feedback.voted.clone();

    let new_voted = match voted.as_deref() {
        // Same kind clicked again → remove the vote.
        Some(active) if active == kind => {
            match kind.as_str() {
                "useful" => useful = useful.saturating_sub(1),
                "irrelevant" => irrelevant = irrelevant.saturating_sub(1),
                "wrong" => wrong = wrong.saturating_sub(1),
                _ => unreachable!("validated above"),
            }
            None
        }
        // A different kind is active → switch the vote (old --, new ++).
        Some(old_kind) => {
            match old_kind {
                "useful" => useful = useful.saturating_sub(1),
                "irrelevant" => irrelevant = irrelevant.saturating_sub(1),
                "wrong" => wrong = wrong.saturating_sub(1),
                _ => {}
            }
            match kind.as_str() {
                "useful" => useful = useful.saturating_add(1),
                "irrelevant" => irrelevant = irrelevant.saturating_add(1),
                "wrong" => wrong = wrong.saturating_add(1),
                _ => unreachable!("validated above"),
            }
            Some(kind.clone())
        }
        // No active vote → register the first one.
        None => {
            match kind.as_str() {
                "useful" => useful = useful.saturating_add(1),
                "irrelevant" => irrelevant = irrelevant.saturating_add(1),
                "wrong" => wrong = wrong.saturating_add(1),
                _ => unreachable!("validated above"),
            }
            Some(kind.clone())
        }
    };

    record.feedback.useful = useful;
    record.feedback.irrelevant = irrelevant;
    record.feedback.wrong = wrong;
    record.feedback.voted = new_voted;

    // A "wrong" verdict means this memory is no longer trustworthy as-is.
    if kind == "wrong" && record.memory_state == MemoryState::Current {
        record.memory_state = MemoryState::Conflicted;
    }
    record.touch();
    repo.update(&record).await.map_err(|e| e.to_string())?;
    Ok(MemoryDto::from(record))
}

/// Re-index a memory whose feedback note changed, so the explanation is
/// discoverable by the copilot and semantic search. Fire-and-forget.
fn spawn_index_with_note(record: &MemoryRecord) {
    let mut content = record.content.clone();
    if let Some(note) = &record.feedback.note {
        content.push_str("\n\n[user feedback] ");
        content.push_str(note);
    }
    crate::core::context::indexer::spawn_index_memory(
        &record.id,
        &record.title,
        &record.summary,
        &content,
    );
}

/// Supersede an existing memory with a newer one.
///
/// The old memory is marked `Superseded` (superseded_by_id points at the new
/// one); a new `Current` record is created. This is the explicit "decision
/// changed" flow — the old decision is never deleted, only demoted.
#[tauri::command]
pub async fn memory_supersede(
    old_id: String,
    new_title: String,
    new_content: String,
    author: Option<String>,
) -> Result<MemoryDto, String> {
    let repo = open_repo()?;
    let old_entity_id = EntityId::parse(&old_id).map_err(|e| e.to_string())?;
    let mut old = repo
        .get_by_id(&old_entity_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Memory {} not found", old_id))?;

    if new_title.trim().is_empty() {
        return Err("new_title cannot be empty".to_string());
    }
    if new_content.trim().is_empty() {
        return Err("new_content cannot be empty".to_string());
    }

    // 1. Create the new record (Current by default) that supersedes the old one.
    let mut replacement = MemoryRecord::new(
        new_title,
        new_content,
        author.unwrap_or_else(|| "user".to_string()),
        MemorySource::Manual,
    )
    .map_err(|e| e.to_string())?;
    replacement.summary = old.summary.clone();
    replacement.supersedes_id = Some(old.id.as_str().to_string());
    replacement.derived_from.push(old.id.as_str().to_string());
    let new_id = repo.save(&replacement).await.map_err(|e| e.to_string())?;

    // 2. Demote the old record.
    old.memory_state = MemoryState::Superseded;
    old.superseded_by_id = Some(new_id.as_str().to_string());
    old.touch();
    repo.update(&old).await.map_err(|e| e.to_string())?;

    // 3. Re-index both so semantic search reflects the new state.
    crate::core::context::indexer::spawn_index_memory(
        &replacement.id,
        &replacement.title,
        &replacement.summary,
        &replacement.content,
    );

    Ok(MemoryDto::from(replacement))
}

/// Count memories per trust state — the "health of the memory" dashboard.
#[tauri::command]
pub async fn get_lifecycle_overview() -> Result<LifecycleOverview, String> {
    let repo = open_repo()?;
    let records = repo.list(100_000, 0).await.map_err(|e| e.to_string())?;
    let mut overview = LifecycleOverview {
        current: 0,
        user_confirmed: 0,
        inferred: 0,
        superseded: 0,
        conflicted: 0,
        total: records.len() as u64,
    };
    for r in &records {
        match r.memory_state {
            MemoryState::Current => overview.current += 1,
            MemoryState::UserConfirmed => overview.user_confirmed += 1,
            MemoryState::Inferred => overview.inferred += 1,
            MemoryState::Superseded => overview.superseded += 1,
            MemoryState::Conflicted => overview.conflicted += 1,
        }
    }
    Ok(overview)
}

/// Aggregate user feedback counters across all memories.
#[tauri::command]
pub async fn get_feedback_summary() -> Result<FeedbackSummary, String> {
    let repo = open_repo()?;
    let records = repo.list(100_000, 0).await.map_err(|e| e.to_string())?;
    let mut summary = FeedbackSummary {
        useful: 0,
        irrelevant: 0,
        wrong: 0,
        total_feedback: 0,
    };
    for r in &records {
        summary.useful += r.feedback.useful as u64;
        summary.irrelevant += r.feedback.irrelevant as u64;
        summary.wrong += r.feedback.wrong as u64;
    }
    summary.total_feedback = summary.useful + summary.irrelevant + summary.wrong;
    Ok(summary)
}

/// Count memory fixes by the user: `irrelevant` + `wrong` feedback marks.
///
/// Used by product metrics as "количество исправлений памяти пользователем".
pub async fn feedback_fix_count() -> Result<u64, String> {
    let repo = open_repo()?;
    let records = repo.list(100_000, 0).await.map_err(|e| e.to_string())?;
    let mut fixes = 0u64;
    for r in &records {
        fixes += r.feedback.irrelevant as u64 + r.feedback.wrong as u64;
    }
    Ok(fixes)
}

/// Empty helper to keep `MemoryFeedback` import meaningful in tests.
#[allow(dead_code)]
fn _feedback_default() -> MemoryFeedback {
    MemoryFeedback::default()
}
