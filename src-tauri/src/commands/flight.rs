//! Flight Recorder commands (System 5) — бортовой самописец операций.
//!
//! Команды поверх `SqliteFlightRepository`: обзор журнала, последние записи,
//! воспроизведение цепочки по сущности и сводная статистика. Записи в журнал
//! попадают автоматически через `flight_listener` (мост event_bus) и через
//! MCP/copilot при ручном логировании операций.

use serde::Serialize;

use crate::core::flight::flight_recorder::{
    FlightCategory, FlightOutcome, FlightRecord, FlightRepository, FlightSession, FlightStats,
};
use crate::storage::sqlite::SqliteFlightRepository;

/// Serializable session for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlightSessionDto {
    pub id: String,
    pub title: String,
    pub purpose: String,
    pub actor: String,
    pub source: String,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
}

impl From<&FlightSession> for FlightSessionDto {
    fn from(s: &FlightSession) -> Self {
        Self {
            id: s.id.clone(),
            title: s.title.clone(),
            purpose: s.purpose.clone(),
            actor: s.actor.clone(),
            source: s.source.clone(),
            status: s.status.as_str().to_string(),
            started_at: s.started_at.to_rfc3339(),
            ended_at: s.ended_at.map(|dt| dt.to_rfc3339()),
        }
    }
}

/// Serializable record for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlightRecordDto {
    pub id: String,
    pub session_id: Option<String>,
    pub recorded_at: String,
    pub actor: String,
    pub category: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: String,
    pub summary: String,
    pub details: serde_json::Value,
    pub duration_ms: i64,
    pub outcome: String,
}

impl From<&FlightRecord> for FlightRecordDto {
    fn from(r: &FlightRecord) -> Self {
        Self {
            id: r.id.clone(),
            session_id: r.session_id.clone(),
            recorded_at: r.recorded_at.to_rfc3339(),
            actor: r.actor.clone(),
            category: r.category.as_str().to_string(),
            action: r.action.clone(),
            entity_type: r.entity_type.clone(),
            entity_id: r.entity_id.clone(),
            summary: r.summary.clone(),
            details: r.details.clone(),
            duration_ms: r.duration_ms,
            outcome: r.outcome.as_str().to_string(),
        }
    }
}

/// Serializable stats for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlightStatsDto {
    pub total_records: u64,
    pub total_sessions: u64,
    pub active_sessions: u64,
    pub context_chains: u64,
    pub by_category: std::collections::BTreeMap<String, u64>,
    pub by_outcome: std::collections::BTreeMap<String, u64>,
}

impl From<&FlightStats> for FlightStatsDto {
    fn from(s: &FlightStats) -> Self {
        Self {
            total_records: s.total_records,
            total_sessions: s.total_sessions,
            active_sessions: s.active_sessions,
            context_chains: 0,
            by_category: s.by_category.clone(),
            by_outcome: s.by_outcome.clone(),
        }
    }
}

fn open_flight_repo() -> Result<SqliteFlightRepository, String> {
    let conn = crate::db::open_connection()?;
    SqliteFlightRepository::new(conn).map_err(|e| e.to_string())
}

/// Логирует операцию вручную (MCP/copilot/UI): произвольный шаг экосистемы.
/// Ничего не создаёт в памяти — только пишет в журнал полёта.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn flight_log(
    category: String,
    action: String,
    summary: String,
    entity_type: Option<String>,
    entity_id: Option<String>,
    outcome: Option<String>,
    details: Option<serde_json::Value>,
    duration_ms: Option<i64>,
    actor: Option<String>,
    session_id: Option<String>,
) -> Result<FlightRecordDto, String> {
    let repo = open_flight_repo()?;
    let record = FlightRecord::new(
        session_id,
        &actor.unwrap_or_else(|| "system".to_string()),
        FlightCategory::parse(&category),
        &action,
        &entity_type.unwrap_or_default(),
        &entity_id.unwrap_or_default(),
        &summary,
        details.unwrap_or_else(|| serde_json::json!({})),
        duration_ms.unwrap_or(0),
        FlightOutcome::parse(&outcome.unwrap_or_else(|| "success".to_string())),
    );
    repo.add_record(&record).await.map_err(|e| e.to_string())?;
    Ok(FlightRecordDto::from(&record))
}

/// Открывает сессию полёта и возвращает её (для MCP/copilot/UI).
#[tauri::command]
pub async fn flight_session_start(
    title: String,
    purpose: String,
    actor: Option<String>,
    source: Option<String>,
) -> Result<FlightSessionDto, String> {
    let repo = open_flight_repo()?;
    let session = FlightSession::new(
        &title,
        &purpose,
        &actor.unwrap_or_else(|| "system".to_string()),
        &source.unwrap_or_else(|| "manual".to_string()),
    );
    repo.create_session(&session)
        .await
        .map_err(|e| e.to_string())?;
    Ok(FlightSessionDto::from(&session))
}

/// Закрывает сессию полёта.
#[tauri::command]
pub async fn flight_session_end(session_id: String) -> Result<(), String> {
    let repo = open_flight_repo()?;
    repo.close_session(&session_id)
        .await
        .map_err(|e| e.to_string())
}

/// Последние записи журнала полёта (лимит; опционально фильтр по категории).
#[tauri::command]
pub async fn flight_recent(
    limit: Option<u32>,
    category: Option<String>,
) -> Result<Vec<FlightRecordDto>, String> {
    let repo = open_flight_repo()?;
    let records = repo
        .recent_records(limit.unwrap_or(50), category.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    Ok(records.iter().map(FlightRecordDto::from).collect())
}

/// Полная цепочка записей по сущности — «воспроизведение полёта» сущности.
#[tauri::command]
pub async fn flight_replay(
    entity_type: String,
    entity_id: String,
) -> Result<Vec<FlightRecordDto>, String> {
    let repo = open_flight_repo()?;
    let records = repo
        .entity_replay(&entity_type, &entity_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(records.iter().map(FlightRecordDto::from).collect())
}

/// Активные сессии полёта — «что сейчас происходит».
#[tauri::command]
pub async fn flight_active_sessions(limit: Option<u32>) -> Result<Vec<FlightSessionDto>, String> {
    let repo = open_flight_repo()?;
    let sessions = repo
        .list_active_sessions(limit.unwrap_or(20))
        .await
        .map_err(|e| e.to_string())?;
    Ok(sessions.iter().map(FlightSessionDto::from).collect())
}

/// Сводная статистика по всему журналу полёта.
#[tauri::command]
pub async fn flight_stats() -> Result<FlightStatsDto, String> {
    let repo = open_flight_repo()?;
    let stats = repo.stats().await.map_err(|e| e.to_string())?;
    let context_chains = repo.count_context_chains().map_err(|e| e.to_string())?;
    let mut dto = FlightStatsDto::from(&stats);
    dto.context_chains = context_chains;
    Ok(dto)
}

// ── Context chain recording (System 5: «почему ИИ так сказал») ──────

/// Serializable context chain for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextChainDto {
    pub id: String,
    pub session_id: Option<String>,
    pub actor: String,
    pub query: String,
    pub intent: String,
    pub answer_confidence: f64,
    pub answer: String,
    pub total_tokens: u32,
    pub created_at: String,
    /// «Why did AI say this?» — текст с ASCII-барами разбивки контекста.
    pub why: String,
    /// Хронология этапов конвейера.
    pub pipeline: String,
}

impl From<&crate::core::flight::context_chain::ContextChain> for ContextChainDto {
    fn from(c: &crate::core::flight::context_chain::ContextChain) -> Self {
        Self {
            id: c.id.clone(),
            session_id: c.session_id.clone(),
            actor: c.actor.clone(),
            query: c.query.clone(),
            intent: c.intent.clone(),
            answer_confidence: c.answer_confidence,
            answer: c.answer.clone(),
            total_tokens: c.total_tokens,
            created_at: c.created_at.to_rfc3339(),
            why: crate::core::flight::context_chain::render_why(c),
            pipeline: crate::core::flight::context_chain::render_stages(c),
        }
    }
}

/// Записать полную цепочку построения контекста ответа.
///
/// `seeds` — JSON-массив объектов `{kind, memoryId, title, weight, tokens}`;
/// `stages` — JSON-массив `{stage, durationMs, note}`. Вызывается в конце
/// конвейера (после ответа модели): отныне любой ответ объясним — можно
/// открыть «Why did AI say this?».
#[tauri::command]
pub async fn context_chain_record(
    query: String,
    intent: Option<String>,
    answer: Option<String>,
    confidence: Option<f64>,
    seeds_json: Option<String>,
    stages_json: Option<String>,
    actor: Option<String>,
) -> Result<ContextChainDto, String> {
    use crate::core::flight::context_chain::{ChainStage, ContextChain, ContextKind};

    let mut chain = ContextChain::begin(
        &query,
        &intent.unwrap_or_else(|| "general".to_string()),
        &actor.unwrap_or_else(|| "user".to_string()),
    );

    if let Some(seeds) = seeds_json {
        let parsed: Vec<serde_json::Value> =
            serde_json::from_str(&seeds).map_err(|e| e.to_string())?;
        for s in parsed {
            let kind = ContextKind::parse(
                s.get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("architecture"),
            );
            let memory_id = s.get("memoryId").and_then(|v| v.as_str()).unwrap_or("");
            let title = s.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let weight = s.get("weight").and_then(|v| v.as_f64()).unwrap_or(0.5);
            let tokens = s.get("tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            if !memory_id.is_empty() {
                chain.add_seed(kind, memory_id, title, weight, tokens);
            }
        }
    }

    if let Some(stages) = stages_json {
        let parsed: Vec<serde_json::Value> =
            serde_json::from_str(&stages).map_err(|e| e.to_string())?;
        for s in parsed {
            let stage = ChainStage::parse(s.get("stage").and_then(|v| v.as_str()).unwrap_or(""));
            let duration = s.get("durationMs").and_then(|v| v.as_i64()).unwrap_or(0);
            let note = s.get("note").and_then(|v| v.as_str()).unwrap_or("");
            chain.pass_stage(stage, duration, note);
        }
    }

    let answer_text = answer.unwrap_or_default();
    if !answer_text.is_empty() {
        chain.finish(&answer_text, confidence.unwrap_or(0.5));
    }

    let repo = open_flight_repo()?;
    repo.save_context_chain(&chain).map_err(|e| e.to_string())?;
    Ok(ContextChainDto::from(&chain))
}

/// Получить цепочку построения контекста по id — «Why did AI say this?».
#[tauri::command]
pub async fn context_chain_get(id: String) -> Result<ContextChainDto, String> {
    let repo = open_flight_repo()?;
    let chain = repo
        .get_context_chain(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Context chain '{}' not found", id))?;
    Ok(ContextChainDto::from(&chain))
}

/// Последние записанные цепочки контекста (новые первыми).
#[tauri::command]
pub async fn context_chain_recent(limit: Option<u32>) -> Result<Vec<ContextChainDto>, String> {
    let repo = open_flight_repo()?;
    let chains = repo
        .recent_context_chains(limit.unwrap_or(10))
        .map_err(|e| e.to_string())?;
    Ok(chains.iter().map(ContextChainDto::from).collect())
}
