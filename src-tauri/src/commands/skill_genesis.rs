//! Skill Genesis commands (System 7) — Nexus обнаруживает повторяющиеся
//! операции в журнале полёта и предлагает превратить их в скиллы.
//!
//! Цикл: `scan` (анализ flight_records → кандидаты) → `candidates` (обзор) →
//! `approve` (создаёт настоящий скилл в `skills`) / `reject` (отклонить).

use serde::Serialize;

use crate::core::flight::flight_recorder::FlightRepository;
use crate::core::knowledge::skill_genesis::{
    ProposalStatus, SkillProposal, detect_patterns, filter_existing, propose, render_proposals,
};
use crate::storage::sqlite::SqliteSkillProposalRepository;

/// Serializable proposal for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillProposalDto {
    pub id: String,
    pub category: String,
    pub action: String,
    pub occurrences: usize,
    pub name: String,
    pub description: String,
    pub status: String,
    pub created_at: String,
}

impl From<&SkillProposal> for SkillProposalDto {
    fn from(p: &SkillProposal) -> Self {
        Self {
            id: p.id.clone(),
            category: p.signature.category.clone(),
            action: p.signature.action.clone(),
            occurrences: p.occurrences,
            name: p.name.clone(),
            description: p.description.clone(),
            status: p.status.as_str().to_string(),
            created_at: p.created_at.clone(),
        }
    }
}

fn open_proposal_repo() -> Result<SqliteSkillProposalRepository, String> {
    let conn = crate::db::open_connection().map_err(|e| e.to_string())?;
    SqliteSkillProposalRepository::new(conn).map_err(|e| e.to_string())
}

/// Отсканировать журнал полёта и предложить новые скиллы.
///
/// `limit` — сколько последних записей анализировать (по умолчанию 2000).
/// `min_occurrences` — порог повторений (по умолчанию 3).
/// Уже известные паттерны (одобренные или отклонённые) не предлагаются снова.
#[tauri::command]
pub async fn skill_genesis_scan(
    limit: Option<u32>,
    min_occurrences: Option<usize>,
) -> Result<serde_json::Value, String> {
    let limit = limit.unwrap_or(2000);
    let min = min_occurrences.unwrap_or(crate::core::knowledge::skill_genesis::MIN_OCCURRENCES);

    // 1. Читаем журнал полёта.
    let conn = crate::db::open_connection().map_err(|e| e.to_string())?;
    let flight_repo =
        crate::storage::sqlite::SqliteFlightRepository::new(conn).map_err(|e| e.to_string())?;
    let records = flight_repo
        .recent_records(limit, None)
        .await
        .map_err(|e| e.to_string())?;

    // 2. Детектируем паттерны.
    let patterns = detect_patterns(&records, min);

    // 3. Исключаем уже известные сигнатуры.
    let proposal_repo = open_proposal_repo()?;
    let known = proposal_repo
        .known_signatures()
        .map_err(|e| e.to_string())?;
    let fresh = filter_existing(patterns, &known);

    // 4. Сохраняем кандидатов.
    let proposals = propose(fresh);
    let mut saved = Vec::new();
    for p in &proposals {
        proposal_repo
            .upsert_proposal(p)
            .map_err(|e| e.to_string())?;
        saved.push(SkillProposalDto::from(p));
    }

    Ok(serde_json::json!({
        "scanned_records": records.len(),
        "threshold": min,
        "new_proposals": saved.len(),
        "proposals": saved,
    }))
}

/// Список кандидатов (status: proposed | approved | rejected | all).
#[tauri::command]
pub async fn skill_genesis_candidates(
    status: Option<String>,
) -> Result<Vec<SkillProposalDto>, String> {
    let repo = open_proposal_repo()?;
    let filter = match status.as_deref() {
        None | Some("") | Some("all") => None,
        Some(s) => Some(ProposalStatus::parse(s)),
    };
    let list = repo.list(filter).map_err(|e| e.to_string())?;
    Ok(list.iter().map(SkillProposalDto::from).collect())
}

/// Одобрить кандидата: создаёт настоящий скилл в `skills` и помечает approved.
#[tauri::command]
pub async fn skill_genesis_approve(id: String) -> Result<SkillProposalDto, String> {
    let repo = open_proposal_repo()?;
    let prop = repo
        .set_status(&id, ProposalStatus::Approved)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Proposal not found: {id}"))?;

    // Создаём реальный скилл. Команда по умолчанию — `true` (no-op), чтобы
    // не запускать ничего опасного: человек/агент потом пропишет команду.
    let skill_repo =
        crate::core::knowledge::skills::SkillRepository::open().map_err(|e| e.to_string())?;
    skill_repo
        .upsert(&prop.name, &prop.description, "true", "")
        .map_err(|e| e.to_string())?;

    Ok(SkillProposalDto::from(&prop))
}

/// Отклонить кандидата: помечает rejected, скилл не создаётся.
#[tauri::command]
pub async fn skill_genesis_reject(id: String) -> Result<SkillProposalDto, String> {
    let repo = open_proposal_repo()?;
    let prop = repo
        .set_status(&id, ProposalStatus::Rejected)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Proposal not found: {id}"))?;
    Ok(SkillProposalDto::from(&prop))
}

/// Текстовый обзор кандидатов для copilot.
pub async fn skill_genesis_render(status: Option<String>) -> Result<String, String> {
    let dto = skill_genesis_candidates(status).await?;
    let proposals: Vec<SkillProposal> = dto
        .iter()
        .map(|d| SkillProposal {
            id: d.id.clone(),
            signature: crate::core::knowledge::skill_genesis::PatternSignature::new(
                &d.category,
                &d.action,
            ),
            occurrences: d.occurrences,
            name: d.name.clone(),
            description: d.description.clone(),
            status: ProposalStatus::parse(&d.status),
            created_at: d.created_at.clone(),
        })
        .collect();
    Ok(render_proposals(&proposals))
}
