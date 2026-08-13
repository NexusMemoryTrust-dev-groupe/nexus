//! Predictive Context commands (System 8) — Nexus предсказывает следующий шаг.
//!
//! Каждый запрос через `build_context` автоматически оседает в query_history.
//! `predictive_predict` строит марковские переходы и отвечает: «с вероятностью
//! 67% следующим будет X, и вот какие сущности прогреть заранее».

use serde::Serialize;

use crate::core::context::predictive::{Prediction, predict_next, prewarm_entities};
use crate::storage::sqlite::SqliteQueryHistoryRepository;

/// Serializable prediction for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PredictionDto {
    pub suggested_query: String,
    pub confidence: f64,
    pub intent_type: String,
    pub entities: Vec<String>,
    pub matches: usize,
}

impl From<&Prediction> for PredictionDto {
    fn from(p: &Prediction) -> Self {
        Self {
            suggested_query: p.suggested_query.clone(),
            confidence: p.confidence,
            intent_type: p.intent_type.clone(),
            entities: p.entities.clone(),
            matches: p.matches,
        }
    }
}

fn open_repo() -> Result<SqliteQueryHistoryRepository, String> {
    let conn = crate::db::open_connection().map_err(|e| e.to_string())?;
    SqliteQueryHistoryRepository::new(conn).map_err(|e| e.to_string())
}

/// Записать запрос в историю (автоматически вызывается из build_context).
pub async fn predictive_log(
    query: &str,
    intent_type: &str,
    entities: Vec<String>,
) -> Result<(), String> {
    if query.trim().is_empty() {
        return Ok(());
    }
    let repo = open_repo()?;
    repo.log_query(query, intent_type, &entities)
        .map_err(|e| e.to_string())
}

/// Предсказать следующий запрос и сущности для прогрева.
#[tauri::command]
pub async fn predictive_predict(
    query: String,
    top_k: Option<usize>,
) -> Result<serde_json::Value, String> {
    let k = top_k.unwrap_or(3);
    let repo = open_repo()?;
    let history = repo.recent(5000).map_err(|e| e.to_string())?;
    let predictions = predict_next(&history, &query, k);
    let prewarm = prewarm_entities(&predictions);
    let dtos: Vec<PredictionDto> = predictions.iter().map(PredictionDto::from).collect();
    Ok(serde_json::json!({
        "query": query,
        "predictions": dtos,
        "prewarm_entities": prewarm,
        "history_size": repo.count().unwrap_or(0),
    }))
}

/// Статистика предсказаний: сколько запросов в истории.
#[tauri::command]
pub async fn predictive_stats() -> Result<serde_json::Value, String> {
    let repo = open_repo()?;
    Ok(serde_json::json!({
        "history_size": repo.count().unwrap_or(0),
    }))
}

/// Текстовый рендер предсказаний для copilot.
pub async fn predictive_render(query: String, top_k: Option<usize>) -> Result<String, String> {
    let k = top_k.unwrap_or(3);
    let repo = open_repo()?;
    let history = repo.recent(5000).map_err(|e| e.to_string())?;
    let predictions = predict_next(&history, &query, k);
    let prewarm = prewarm_entities(&predictions);
    let text = crate::core::context::predictive::render_predictions(&predictions);
    if prewarm.is_empty() {
        Ok(text)
    } else {
        Ok(format!("{text}Prewarm: {}", prewarm.join(", ")))
    }
}
