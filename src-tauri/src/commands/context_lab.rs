//! Context Lab commands (System 6) — лаборатория качества контекста.
//!
//! Один вопрос собирается несколькими стратегиями (compact / balanced / rich),
//! каждая снимает метрики: сколько памяти и сущностей вошло, сколько токенов,
//! насколько зрелые слои, и предсказание точности ответа. Победитель и вся
//! история сохраняются в `context_lab_runs` — Nexus начинает учиться выбирать
//! стратегию по вопросу.

use serde::Serialize;

use crate::core::context::ContextRequest;
use crate::core::context::context_builder::ContextBuilder;
use crate::core::context::context_lab::{LabExperiment, LabMetrics, LabResult};
use crate::storage::sqlite::SqliteContextLabRepository;

/// Serializable experiment for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabExperimentDto {
    pub query: String,
    pub created_at: String,
    pub results: Vec<LabResultDto>,
    pub best_strategy: String,
    pub summary: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabResultDto {
    pub strategy: String,
    pub memories: u32,
    pub entities: u32,
    pub tokens: u32,
    pub baseline_tokens: u32,
    pub avg_relevance: f64,
    pub maturity: f64,
    pub accuracy: f64,
    pub efficiency_per_k_token: f64,
    pub build_ms: u64,
}

impl From<&LabExperiment> for LabExperimentDto {
    fn from(exp: &LabExperiment) -> Self {
        let results = exp
            .results
            .iter()
            .map(|r| LabResultDto {
                strategy: r.strategy.name.clone(),
                memories: r.metrics.memories,
                entities: r.metrics.entities,
                tokens: r.metrics.tokens,
                baseline_tokens: r.metrics.baseline_tokens,
                avg_relevance: r.metrics.avg_relevance,
                maturity: r.metrics.maturity,
                accuracy: r.metrics.accuracy,
                efficiency_per_k_token: crate::core::context::context_lab::efficiency_per_token(
                    &r.metrics,
                ),
                build_ms: r.metrics.build_ms,
            })
            .collect();
        let summary = crate::core::context::context_lab::render_comparison(exp);
        Self {
            query: exp.query.clone(),
            created_at: exp.created_at.clone(),
            best_strategy: exp
                .best()
                .map(|r| r.strategy.name.clone())
                .unwrap_or_default(),
            results,
            summary,
        }
    }
}

/// Собрать один пакет контекста по стратегии и снять метрики.
async fn run_strategy(
    query: &str,
    strategy: &crate::core::context::ContextStrategy,
) -> Result<LabResult, String> {
    let mem_conn = crate::db::open_connection()?;
    let graph_conn = crate::db::open_connection()?;

    let memory_repo =
        crate::storage::sqlite::SqliteMemoryRepository::new(mem_conn).map_err(|e| e.to_string())?;
    let graph_repo = crate::storage::sqlite::SqliteGraphRepository::new(graph_conn)
        .map_err(|e| e.to_string())?;

    let builder =
        crate::core::context::context_builder::ContextBuilderImpl::new(graph_repo, memory_repo);
    let request = ContextRequest {
        query: query.to_string(),
        max_tokens: strategy.max_tokens,
        max_entities: strategy.max_entities,
        max_depth: strategy.max_depth,
        min_relevance: strategy.min_relevance,
        ..Default::default()
    };

    let start = std::time::Instant::now();
    let pkg = builder.build(&request).await.map_err(|e| e.to_string())?;
    let build_ms = start.elapsed().as_millis() as u64;

    // Средняя релевантность по включённым элементам (из provenance).
    let mut score_sum = 0.0f64;
    let mut score_count = 0u32;
    for t in pkg.provenance.included() {
        if let Some(s) = t.score {
            score_sum += s;
            score_count += 1;
        }
    }
    let avg_relevance = if score_count > 0 {
        score_sum / score_count as f64
    } else {
        0.0
    };

    // Зрелость: доля записей в стабильных слоях (Semantic/Procedural/Decision/Strategic).
    let total = pkg.memory_records.len() as f64;
    let mature = pkg
        .memory_records
        .iter()
        .filter(|m| {
            matches!(
                m.layer,
                crate::core::memory::types::MemoryLayer::Semantic
                    | crate::core::memory::types::MemoryLayer::Procedural
                    | crate::core::memory::types::MemoryLayer::Decision
                    | crate::core::memory::types::MemoryLayer::Strategic
            )
        })
        .count() as f64;
    let maturity = if total > 0.0 { mature / total } else { 0.0 };

    let accuracy = crate::core::context::context_lab::predict_accuracy(
        pkg.memory_records.len() as u32,
        pkg.token_count,
        avg_relevance,
        maturity,
    );

    Ok(LabResult {
        query: query.to_string(),
        strategy: strategy.clone(),
        metrics: LabMetrics {
            memories: pkg.memory_records.len() as u32,
            entities: pkg.entities.len() as u32,
            tokens: pkg.token_count,
            baseline_tokens: pkg.baseline_tokens,
            avg_relevance,
            maturity,
            accuracy,
            build_ms,
        },
        package_id: pkg.id,
    })
}

/// Запустить лабораторный эксперимент: собрать контекст по всем стратегиям.
#[tauri::command]
pub async fn context_lab_run(query: String) -> Result<LabExperimentDto, String> {
    if query.trim().is_empty() {
        return Err("query must not be empty".to_string());
    }

    let strategies = crate::core::context::ContextStrategy::default_lab();
    let mut results = Vec::with_capacity(strategies.len());
    for s in &strategies {
        match run_strategy(&query, s).await {
            Ok(r) => results.push(r),
            Err(e) => {
                // Одна стратегия может не набрать контекст — не роняем весь
                // эксперимент, а помечаем прогон нулевыми метриками.
                results.push(LabResult {
                    query: query.clone(),
                    strategy: s.clone(),
                    metrics: LabMetrics {
                        memories: 0,
                        entities: 0,
                        tokens: 0,
                        baseline_tokens: 0,
                        avg_relevance: 0.0,
                        maturity: 0.0,
                        accuracy: 0.0,
                        build_ms: 0,
                    },
                    package_id: format!("failed:{}", e),
                });
            }
        }
    }

    let exp = LabExperiment {
        query: query.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        results,
    };

    // Сохраняем эксперимент для истории и обучения выбору стратегии.
    let conn = crate::db::open_connection()?;
    let repo = SqliteContextLabRepository::new(conn).map_err(|e| e.to_string())?;
    if let Err(e) = repo.save_experiment(&exp) {
        eprintln!("context_lab: failed to persist experiment: {e}");
    }

    Ok(LabExperimentDto::from(&exp))
}

/// История лабораторных экспериментов (свежие первыми).
#[tauri::command]
pub async fn context_lab_history(limit: Option<usize>) -> Result<Vec<LabExperimentDto>, String> {
    let conn = crate::db::open_connection()?;
    let repo = SqliteContextLabRepository::new(conn).map_err(|e| e.to_string())?;
    let experiments = repo
        .recent_experiments(limit.unwrap_or(10))
        .map_err(|e| e.to_string())?;
    Ok(experiments.iter().map(LabExperimentDto::from).collect())
}

/// Краткая статистика лаборатории: сколько экспериментов, кто победитель.
#[tauri::command]
pub async fn context_lab_stats() -> Result<serde_json::Value, String> {
    let conn = crate::db::open_connection()?;
    let repo = SqliteContextLabRepository::new(conn).map_err(|e| e.to_string())?;
    let total = repo.count().map_err(|e| e.to_string())?;
    let best = repo.best_strategy_overall().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "total_experiments": total,
        "winning_strategy": best,
    }))
}
