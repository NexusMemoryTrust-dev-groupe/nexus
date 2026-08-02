use rusqlite::params;
use serde::Serialize;

use crate::db;

/// Pricing per 1M tokens (USD) for popular LLMs — August 2026 actual data.
/// Used to calculate realistic cost savings.
mod pricing {
    /// Average input cost per 1M tokens across popular models.
    /// Weighted by usage: GPT-5.6 Terra, Claude Sonnet 5, Gemini 3.1 Pro, DeepSeek V4 Flash.
    /// (2 + 2 + 2 + 0.14) / 4 ≈ $1.54 per 1M input tokens
    pub const AVG_INPUT_PER_M: f64 = 1.54;

    /// Cost per input token.
    pub const COST_PER_INPUT_TOKEN: f64 = AVG_INPUT_PER_M / 1_000_000.0;
}

/// Model pricing reference (per 1M tokens, USD) — August 2026.
/// Mirrors the frontend pricing table so the numbers are always truthful.
#[derive(Clone, Copy, Serialize)]
pub struct ModelPricing {
    pub company: &'static str,
    pub name: &'static str,
    pub input_per_m: f64,
    pub output_per_m: f64,
    pub context: &'static str,
    pub purpose: &'static str,
}

/// Full model catalog used for per-model savings calculations.
pub const ALL_MODELS: &[ModelPricing] = &[
    // OpenAI
    ModelPricing { company: "OpenAI",     name: "GPT-5.6 Sol",       input_per_m: 5.00,  output_per_m: 30.00, context: "1M",    purpose: "Flagship" },
    ModelPricing { company: "OpenAI",     name: "GPT-5.6 Terra",     input_per_m: 2.00,  output_per_m: 12.00, context: "1M",    purpose: "Main" },
    ModelPricing { company: "OpenAI",     name: "GPT-5.6 Luna",      input_per_m: 0.20,  output_per_m: 1.20,  context: "1M",    purpose: "Budget" },
    // Anthropic
    ModelPricing { company: "Anthropic",  name: "Claude Fable 5",    input_per_m: 10.00, output_per_m: 50.00, context: "500k+", purpose: "Max quality" },
    ModelPricing { company: "Anthropic",  name: "Claude Opus 5",     input_per_m: 5.00,  output_per_m: 25.00, context: "500k+", purpose: "Top reasoning" },
    ModelPricing { company: "Anthropic",  name: "Claude Sonnet 5",   input_per_m: 2.00,  output_per_m: 10.00, context: "500k+", purpose: "Best value" },
    ModelPricing { company: "Anthropic",  name: "Claude Sonnet 4.6", input_per_m: 3.00,  output_per_m: 15.00, context: "500k",  purpose: "Prev gen" },
    ModelPricing { company: "Anthropic",  name: "Claude Haiku 4.5",  input_per_m: 1.00,  output_per_m: 5.00,  context: "500k",  purpose: "Fast" },
    // Google
    ModelPricing { company: "Google",     name: "Gemini 3.1 Pro",    input_per_m: 2.00,  output_per_m: 12.00, context: "1M",    purpose: "Main Gemini" },
    ModelPricing { company: "Google",     name: "Gemini 2.5 Pro",    input_per_m: 1.25,  output_per_m: 10.00, context: "1M",    purpose: "Prev gen" },
    ModelPricing { company: "Google",     name: "Gemini Flash",      input_per_m: 0.35,  output_per_m: 1.50,  context: "1M",    purpose: "Budget" },
    // xAI
    ModelPricing { company: "xAI",        name: "Grok 4.x",          input_per_m: 2.50,  output_per_m: 12.50, context: "2M",    purpose: "Reasoning" },
    ModelPricing { company: "xAI",        name: "Grok Fast",         input_per_m: 0.80,  output_per_m: 5.00,  context: "2M",    purpose: "Fast" },
    // DeepSeek
    ModelPricing { company: "DeepSeek",   name: "DeepSeek V4",       input_per_m: 0.30,  output_per_m: 1.20,  context: "256k", purpose: "Universal" },
    ModelPricing { company: "DeepSeek",   name: "DeepSeek V4 Flash", input_per_m: 0.14,  output_per_m: 0.90,  context: "256k", purpose: "Cheapest" },
    // Moonshot
    ModelPricing { company: "Moonshot",   name: "Kimi K3",           input_per_m: 0.50,  output_per_m: 2.00,  context: "1M",    purpose: "Code" },
    // Alibaba
    ModelPricing { company: "Alibaba",    name: "Qwen 3",            input_per_m: 0.40,  output_per_m: 2.00,  context: "1M",    purpose: "Universal" },
    // Mistral
    ModelPricing { company: "Mistral",    name: "Magistral Medium",  input_per_m: 2.00,  output_per_m: 8.00,  context: "256k", purpose: "Reasoning" },
    ModelPricing { company: "Mistral",    name: "Mistral Small",     input_per_m: 0.80,  output_per_m: 2.50,  context: "128k", purpose: "Fast" },
    // Cohere
    ModelPricing { company: "Cohere",     name: "Command R+",        input_per_m: 3.00,  output_per_m: 15.00, context: "128k", purpose: "RAG" },
    ModelPricing { company: "Cohere",     name: "Command R",         input_per_m: 1.00,  output_per_m: 5.00,  context: "128k", purpose: "Enterprise" },
];

/// Look up a model by display name (case-insensitive, trims whitespace).
/// Also matches "Company Model" combined names.
pub fn find_model(name: &str) -> Option<&'static ModelPricing> {
    let needle = name.trim().to_lowercase();
    ALL_MODELS.iter().find(|m| {
        m.name.to_lowercase() == needle
            || format!("{} {}", m.company, m.name).to_lowercase() == needle
    })
}

/// Cost (USD) for `tokens` input tokens at the given per-1M price.
pub fn cost_for_tokens(tokens: u64, input_per_m: f64) -> f64 {
    tokens as f64 * (input_per_m / 1_000_000.0)
}

/// Cumulative savings stats returned to frontend.
#[derive(Serialize)]
pub struct SavingsStats {
    pub total_interactions: u64,
    pub total_tokens_saved: u64,
    pub total_cost_saved_usd: f64,
    pub avg_tokens_per_interaction: u64,
    pub tokens_saved_today: u64,
    pub cost_saved_today: f64,
    pub tokens_saved_week: u64,
    pub cost_saved_week: f64,
    pub tokens_saved_month: u64,
    pub cost_saved_month: f64,
    pub tokens_saved_year: u64,
    pub cost_saved_year: f64,
    pub obsidian_equivalent_tokens: u64,
    pub obsidian_equivalent_cost_usd: f64,
    pub recent_interactions: Vec<InteractionRecord>,

    // ── Provenance of the numbers above ──
    //
    // These let the UI state *how* a figure was produced instead of asking the
    // reader to trust it. `measured_interactions` counts rows recorded with a
    // measured baseline; `exact_interactions` counts the subset where the real
    // BPE vocabulary was available. When they differ, part of the total is
    // approximated and the UI should say so.
    /// Sum of measured baselines: tokens the model would have read in full.
    pub baseline_tokens: u64,
    /// Cost of that baseline at the blended input price.
    pub baseline_cost_usd: f64,
    /// Interactions recorded with a measured baseline (excludes legacy rows).
    pub measured_interactions: u64,
    /// Of those, how many were counted with the exact BPE vocabulary.
    pub exact_interactions: u64,
    /// `"exact"`, `"estimated"`, or `"mixed"` — the dominant counting method.
    pub token_method: String,
}

#[derive(Serialize)]
pub struct InteractionRecord {
    pub tokens_saved: u64,
    pub cost_saved_usd: f64,
    pub entities_count: u64,
    pub memories_count: u64,
    pub query_preview: String,
    pub created_at: String,
}

/// A measured savings event.
///
/// Both token figures are *measured*, not assumed:
///
/// * `baseline_tokens` — tokens the model would have consumed had it read every
///   candidate source in full. Counted with the same tokenizer as the payload.
/// * `context_tokens` — tokens in the package Nexus actually produced.
///
/// The saving is the difference. If the context engine ever produced more
/// tokens than reading the sources outright, the saving is zero rather than a
/// negative number dressed up as a win.
#[derive(Debug, Clone, Default)]
pub struct SavingsMeasurement {
    pub baseline_tokens: u32,
    pub context_tokens: u32,
    pub entities_count: u32,
    pub memories_count: u32,
    pub relationships_count: u32,
    pub candidate_entities: u32,
    pub candidate_memories: u32,
    /// `"exact"` when the real BPE vocabulary was used, `"estimated"` otherwise.
    pub token_method: String,
}

impl SavingsMeasurement {
    /// Build a measurement from a finished context package.
    ///
    /// Both figures come from the package itself: `baseline_tokens` was counted
    /// by the builder before compression (the full candidate set), and
    /// `token_count` after it. Nothing is inferred here.
    pub fn from_package(pkg: &crate::core::context::ContextPackage) -> Self {
        Self {
            baseline_tokens: pkg.baseline_tokens,
            context_tokens: pkg.token_count,
            entities_count: pkg.entities.len() as u32,
            memories_count: pkg.memory_records.len() as u32,
            relationships_count: pkg.relationships.len() as u32,
            candidate_entities: pkg.candidate_entities,
            candidate_memories: pkg.candidate_memories,
            token_method: crate::core::tokenizer::method().as_str().to_string(),
        }
    }

    /// Tokens avoided. Saturating: never negative.
    pub fn tokens_saved(&self) -> u32 {
        self.baseline_tokens.saturating_sub(self.context_tokens)
    }

    /// Cost avoided in USD at the blended input price.
    pub fn cost_saved_usd(&self) -> f64 {
        self.tokens_saved() as f64 * pricing::COST_PER_INPUT_TOKEN
    }
}

/// Persist a measured savings event.
///
/// Replaces the previous implementation, which stored a hardcoded
/// `manual_context_tokens = 800` and then reported `context_tokens` itself as
/// the "saving" — so the headline number was both an estimate and the wrong
/// quantity. Nothing here is assumed: every value written was measured by the
/// caller.
fn record_savings_inner(m: &SavingsMeasurement, query: &str, intent_type: &str)
    -> std::result::Result<(), String>
{
    let conn = db::open_connection().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO savings_log (
            id, context_tokens, entities_count, memories_count, relationships_count,
            manual_context_tokens, tokens_saved, cost_saved_usd, query_text, intent_type,
            baseline_tokens, token_method, candidate_entities, candidate_memories
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            id,
            m.context_tokens,
            m.entities_count,
            m.memories_count,
            m.relationships_count,
            // Legacy column, kept so old reports keep parsing. It now holds the
            // measured baseline instead of the 800-token guess.
            m.baseline_tokens,
            m.tokens_saved(),
            m.cost_saved_usd(),
            query,
            intent_type,
            m.baseline_tokens,
            m.token_method,
            m.candidate_entities,
            m.candidate_memories,
        ],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// Tauri command: record a measured savings event.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn record_savings_event(
    baseline_tokens: u32,
    context_tokens: u32,
    entities_count: u32,
    memories_count: u32,
    relationships_count: u32,
    candidate_entities: u32,
    candidate_memories: u32,
    query: String,
    intent_type: String,
) -> std::result::Result<(), String> {
    let m = SavingsMeasurement {
        baseline_tokens,
        context_tokens,
        entities_count,
        memories_count,
        relationships_count,
        candidate_entities,
        candidate_memories,
        token_method: crate::core::tokenizer::method().as_str().to_string(),
    };
    record_savings_inner(&m, &query, &intent_type)
}

/// Tauri command: get comprehensive savings statistics.
#[tauri::command]
pub fn get_savings_stats() -> std::result::Result<SavingsStats, String> {
    let conn = db::open_connection().map_err(|e| e.to_string())?;

    // Total stats
    let (total_interactions, total_tokens_saved, total_cost_saved): (i64, i64, f64) = conn
        .prepare("SELECT COUNT(*), COALESCE(SUM(tokens_saved), 0), COALESCE(SUM(cost_saved_usd), 0.0) FROM savings_log")
        .map_err(|e| e.to_string())?
        .query_row([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .unwrap_or((0, 0, 0.0));

    let total_interactions = total_interactions as u64;
    let total_tokens_saved = total_tokens_saved as u64;

    let avg_tokens = if total_interactions > 0 {
        total_tokens_saved / total_interactions
    } else {
        0
    };

    // Period stats
    let tokens_today = query_period_tokens(&conn, "datetime('now', 'start of day')").unwrap_or(0);
    let cost_today = query_period_cost(&conn, "datetime('now', 'start of day')").unwrap_or(0.0);
    let tokens_week = query_period_tokens(&conn, "datetime('now', '-7 days')").unwrap_or(0);
    let cost_week = query_period_cost(&conn, "datetime('now', '-7 days')").unwrap_or(0.0);
    let tokens_month = query_period_tokens(&conn, "datetime('now', 'start of month')").unwrap_or(0);
    let cost_month = query_period_cost(&conn, "datetime('now', 'start of month')").unwrap_or(0.0);
    let tokens_year = query_period_tokens(&conn, "datetime('now', 'start of year')").unwrap_or(0);
    let cost_year = query_period_cost(&conn, "datetime('now', 'start of year')").unwrap_or(0.0);

    // What the same work would have cost without Nexus: the measured baseline,
    // i.e. the tokens the model would have consumed reading every candidate
    // source in full. This used to be `interactions * 2000` — a number nobody
    // could reproduce. Now it is the sum of what we actually measured per
    // interaction, so the comparison is auditable.
    let baseline_tokens: u64 = conn
        .query_row(
            "SELECT COALESCE(SUM(baseline_tokens), 0) FROM savings_log",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|v| v.max(0) as u64)
        .unwrap_or(0);
    let baseline_cost = baseline_tokens as f64 * pricing::COST_PER_INPUT_TOKEN;

    // How many rows carry measured (rather than legacy estimated) numbers.
    let measured_interactions: u64 = conn
        .query_row(
            "SELECT COUNT(*) FROM savings_log WHERE token_method IN ('exact', 'estimated')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|v| v.max(0) as u64)
        .unwrap_or(0);

    let exact_interactions: u64 = conn
        .query_row(
            "SELECT COUNT(*) FROM savings_log WHERE token_method = 'exact'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|v| v.max(0) as u64)
        .unwrap_or(0);

    let recent = get_recent_interactions(&conn).unwrap_or_default();

    Ok(SavingsStats {
        total_interactions,
        total_tokens_saved,
        total_cost_saved_usd: total_cost_saved,
        avg_tokens_per_interaction: avg_tokens,
        tokens_saved_today: tokens_today,
        cost_saved_today: cost_today,
        tokens_saved_week: tokens_week,
        cost_saved_week: cost_week,
        tokens_saved_month: tokens_month,
        cost_saved_month: cost_month,
        tokens_saved_year: tokens_year,
        cost_saved_year: cost_year,
        obsidian_equivalent_tokens: baseline_tokens,
        obsidian_equivalent_cost_usd: baseline_cost,
        baseline_tokens,
        baseline_cost_usd: baseline_cost,
        measured_interactions,
        exact_interactions,
        token_method: crate::core::tokenizer::method().as_str().to_string(),
        recent_interactions: recent,
    })
}

/// Per-model savings row for reports.
#[derive(Serialize)]
pub struct ModelSavingsRow {
    pub company: &'static str,
    pub name: &'static str,
    pub input_per_m: f64,
    pub output_per_m: f64,
    pub context: &'static str,
    pub purpose: &'static str,
    /// Cost saved (USD) using this model's input price for the total tokens saved.
    pub cost_saved_usd: f64,
}

/// Full savings report: aggregate stats + per-model breakdown.
#[derive(Serialize)]
pub struct SavingsReport {
    pub stats: SavingsStats,
    pub models: Vec<ModelSavingsRow>,
}

/// Build a comprehensive savings report (stats + per-model cost breakdown).
/// Uses the real `total_tokens_saved` from the database — no estimates.
#[tauri::command]
pub fn get_savings_report() -> std::result::Result<SavingsReport, String> {
    let stats = get_savings_stats()?;
    let tokens = stats.total_tokens_saved;

    let models = ALL_MODELS
        .iter()
        .map(|m| ModelSavingsRow {
            company: m.company,
            name: m.name,
            input_per_m: m.input_per_m,
            output_per_m: m.output_per_m,
            context: m.context,
            purpose: m.purpose,
            cost_saved_usd: cost_for_tokens(tokens, m.input_per_m),
        })
        .collect();

    Ok(SavingsReport { stats, models })
}

/// Get savings for a single model (by display name).
/// Returns the model pricing + how much it saved with the total tokens.
#[tauri::command]
pub fn get_model_savings(model: &str) -> std::result::Result<serde_json::Value, String> {
    let stats = get_savings_stats()?;
    match find_model(model) {
        Some(m) => Ok(serde_json::json!({
            "model": {
                "company": m.company,
                "name": m.name,
                "input_per_m": m.input_per_m,
                "output_per_m": m.output_per_m,
                "context": m.context,
                "purpose": m.purpose,
            },
            "total_tokens_saved": stats.total_tokens_saved,
            "cost_saved_usd": cost_for_tokens(stats.total_tokens_saved, m.input_per_m),
            "total_interactions": stats.total_interactions,
        })),
        None => Err(format!(
            "Unknown model '{}'. Known models: {}",
            model,
            ALL_MODELS.iter().map(|m| m.name).collect::<Vec<_>>().join(", ")
        )),
    }
}

fn query_period_tokens(conn: &rusqlite::Connection, date_expr: &str) -> std::result::Result<u64, String> {
    let sql = format!(
        "SELECT COALESCE(SUM(tokens_saved), 0) FROM savings_log WHERE created_at >= {}",
        date_expr
    );
    conn.query_row(&sql, [], |row| row.get::<_, i64>(0))
        .map(|v| v as u64)
        .map_err(|e| e.to_string())
}

fn query_period_cost(conn: &rusqlite::Connection, date_expr: &str) -> std::result::Result<f64, String> {
    let sql = format!(
        "SELECT COALESCE(SUM(cost_saved_usd), 0.0) FROM savings_log WHERE created_at >= {}",
        date_expr
    );
    conn.query_row(&sql, [], |row| row.get::<_, f64>(0))
        .map_err(|e| e.to_string())
}

fn get_recent_interactions(conn: &rusqlite::Connection) -> std::result::Result<Vec<InteractionRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT tokens_saved, cost_saved_usd, entities_count, memories_count, query_text, created_at
             FROM savings_log ORDER BY created_at DESC LIMIT 30",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(InteractionRecord {
                tokens_saved: row.get::<_, i64>(0).unwrap_or(0) as u64,
                cost_saved_usd: row.get(1).unwrap_or(0.0),
                entities_count: row.get::<_, i64>(2).unwrap_or(0) as u64,
                memories_count: row.get::<_, i64>(3).unwrap_or(0) as u64,
                query_preview: row.get::<_, Option<String>>(4).ok().flatten().unwrap_or_default(),
                created_at: row.get(5).unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for row in rows {
        if let Ok(r) = row {
            result.push(r);
        }
    }
    Ok(result)
}

/// Record an already-measured event. Errors are swallowed by design.
pub fn record_savings(m: &SavingsMeasurement, query: &str, intent_type: &str) {
    let _ = record_savings_inner(m, query, intent_type);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pricing_constants_are_positive() {
        assert!(pricing::AVG_INPUT_PER_M > 0.0);
        assert!(pricing::COST_PER_INPUT_TOKEN > 0.0);
        assert!(pricing::COST_PER_INPUT_TOKEN < 0.001);
    }

    #[test]
    fn cost_calculation_is_correct() {
        // 1000 tokens at $1.54/1M = $0.00154
        let cost = 1000.0 * pricing::COST_PER_INPUT_TOKEN;
        assert!((cost - 0.00154).abs() < 0.0001);
    }

    #[test]
    fn model_catalog_has_all_expected_models() {
        // 21 models from the frontend reference table
        assert_eq!(ALL_MODELS.len(), 21);
        assert!(ALL_MODELS.iter().any(|m| m.name == "GPT-5.6 Sol"));
        assert!(ALL_MODELS.iter().any(|m| m.name == "DeepSeek V4 Flash"));
        assert!(ALL_MODELS.iter().any(|m| m.name == "Claude Sonnet 5"));
        assert!(ALL_MODELS.iter().any(|m| m.name == "Command R+"));
    }

    #[test]
    fn find_model_case_insensitive() {
        assert!(find_model("gpt-5.6 terra").is_some());
        assert!(find_model("GPT-5.6 SOL").is_some());
        assert!(find_model("deepseek v4 flash").is_some());
        assert!(find_model("Anthropic Claude Opus 5").is_some());
        assert!(find_model("nonexistent-model").is_none());
    }

    #[test]
    fn cost_for_tokens_is_correct() {
        // 1,000,000 tokens at $5/1M = $5.00
        let cost = cost_for_tokens(1_000_000, 5.0);
        assert!((cost - 5.0).abs() < 1e-9);
        // 500,000 tokens at $0.14/1M = $0.07
        let cost2 = cost_for_tokens(500_000, 0.14);
        assert!((cost2 - 0.07).abs() < 1e-9);
    }
}
