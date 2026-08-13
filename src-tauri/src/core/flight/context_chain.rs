//! Context Chain Recording — цепочка построения контекста (Система 5).
//!
//! Спецификация: когда агент отвечает, конвейер проходит
//!
//!   USER REQUEST → INTENT → MEMORY SEEDS → GRAPH EXPANSION → RANKING →
//!   TRUST FILTER → TOKEN COMPRESSION → FINAL CONTEXT → MODEL → ANSWER
//!
//! и пользователь может открыть **«Why did AI say this?»**:
//!
//!   Answer confidence: 87%
//!   Context:
//!     ████████████ 42% Architecture
//!     ████████     28% Recent decisions
//!     ████         19% Code evidence
//!     ██            7% Semantic memory
//!     █             4% Historical context
//!
//! Модуль хранит каждый шаг конвейера и каждую «затравку» (memory seed),
//! попавшую в контекст, с её весом. Отсюда — объяснимость: любой ответ можно
//! разобрать до конкретных записей памяти. Чистые функции, без БД.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Этап конвейера построения контекста (порядок из спецификации).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChainStage {
    Request,
    Intent,
    MemorySeeds,
    GraphExpansion,
    Ranking,
    TrustFilter,
    TokenCompression,
    FinalContext,
    Model,
    Answer,
}

impl ChainStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Intent => "intent",
            Self::MemorySeeds => "memory_seeds",
            Self::GraphExpansion => "graph_expansion",
            Self::Ranking => "ranking",
            Self::TrustFilter => "trust_filter",
            Self::TokenCompression => "token_compression",
            Self::FinalContext => "final_context",
            Self::Model => "model",
            Self::Answer => "answer",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "request" => Self::Request,
            "intent" => Self::Intent,
            "memory_seeds" => Self::MemorySeeds,
            "graph_expansion" => Self::GraphExpansion,
            "ranking" => Self::Ranking,
            "trust_filter" => Self::TrustFilter,
            "token_compression" => Self::TokenCompression,
            "final_context" => Self::FinalContext,
            "model" => Self::Model,
            "answer" => Self::Answer,
            _ => Self::Request,
        }
    }
}

/// Категория контекста для разбивки «почему ИИ так сказал».
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ContextKind {
    Architecture,
    Decisions,
    Code,
    Semantic,
    Historical,
    Working,
}

impl ContextKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Architecture => "architecture",
            Self::Decisions => "decisions",
            Self::Code => "code",
            Self::Semantic => "semantic",
            Self::Historical => "historical",
            Self::Working => "working",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "decisions" => Self::Decisions,
            "code" => Self::Code,
            "semantic" => Self::Semantic,
            "historical" => Self::Historical,
            "working" => Self::Working,
            _ => Self::Architecture,
        }
    }
}

/// Одна затравка памяти, попавшая в контекст (seed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSeed {
    /// Категория контекста (для разбивки).
    pub kind: ContextKind,
    /// Id записи памяти, ответственной за этот кусок контекста.
    pub memory_id: String,
    pub title: String,
    /// Относительный вес в контексте (0.0–1.0).
    pub weight: f64,
    /// Сколько токенов заняла запись в финальном контексте.
    pub tokens: u32,
}

/// Запись прохождения одного этапа конвейера.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageRecord {
    pub stage: ChainStage,
    pub started_at: DateTime<Utc>,
    pub duration_ms: i64,
    /// Что этап сделал (человекочитаемо).
    pub note: String,
}

/// Полная цепочка построения контекста одного ответа.
#[derive(Debug, Clone, Serialize)]
pub struct ContextChain {
    pub id: String,
    pub session_id: Option<String>,
    pub actor: String,
    pub query: String,
    pub intent: String,
    /// Уверенность ответа 0.0–1.0.
    pub answer_confidence: f64,
    pub answer: String,
    pub seeds: Vec<ContextSeed>,
    pub stages: Vec<StageRecord>,
    pub created_at: DateTime<Utc>,
    /// Итоговая стоимость контекста в токенах.
    pub total_tokens: u32,
}

impl ContextChain {
    /// Начать новую цепочку (без семян и этапов — их добавляют по ходу).
    pub fn begin(query: &str, intent: &str, actor: &str) -> Self {
        Self {
            id: crate::core::entity_id::EntityId::new().as_str().to_string(),
            session_id: None,
            actor: actor.to_string(),
            query: query.to_string(),
            intent: intent.to_string(),
            answer_confidence: 0.0,
            answer: String::new(),
            seeds: Vec::new(),
            stages: vec![StageRecord {
                stage: ChainStage::Request,
                started_at: Utc::now(),
                duration_ms: 0,
                note: "user request received".to_string(),
            }],
            created_at: Utc::now(),
            total_tokens: 0,
        }
    }

    /// Добавить семя памяти, попавшее в контекст.
    pub fn add_seed(
        &mut self,
        kind: ContextKind,
        memory_id: &str,
        title: &str,
        weight: f64,
        tokens: u32,
    ) {
        self.seeds.push(ContextSeed {
            kind,
            memory_id: memory_id.to_string(),
            title: title.to_string(),
            weight: weight.clamp(0.0, 1.0),
            tokens,
        });
        self.total_tokens += tokens;
    }

    /// Отметить прохождение этапа конвейера.
    pub fn pass_stage(&mut self, stage: ChainStage, duration_ms: i64, note: &str) {
        self.stages.push(StageRecord {
            stage,
            started_at: Utc::now(),
            duration_ms,
            note: note.to_string(),
        });
    }

    /// Зафиксировать ответ.
    pub fn finish(&mut self, answer: &str, confidence: f64) {
        self.answer = answer.to_string();
        self.answer_confidence = confidence.clamp(0.0, 1.0);
        self.pass_stage(ChainStage::Answer, 0, "answer produced");
    }
}

/// Доля категории в контексте (0.0–1.0, сумма ~1.0).
#[derive(Debug, Clone, Serialize)]
pub struct KindShare {
    pub kind: ContextKind,
    pub share: f64,
}

/// Считает разбивку контекста по категориям (для «почему ИИ так сказал»).
///
/// Доля = сумма весов семян категории / сумма всех весов. Категории без
/// семян не включаются. Результат отсортирован по убыванию доли.
pub fn context_breakdown(chain: &ContextChain) -> Vec<KindShare> {
    let mut totals: std::collections::BTreeMap<ContextKind, f64> = Default::default();
    let mut sum: f64 = 0.0;
    for seed in &chain.seeds {
        *totals.entry(seed.kind).or_insert(0.0) += seed.weight;
        sum += seed.weight;
    }
    if sum <= 0.0 {
        return Vec::new();
    }
    let mut shares: Vec<KindShare> = totals
        .into_iter()
        .map(|(kind, w)| KindShare {
            kind,
            share: w / sum,
        })
        .collect();
    shares.sort_by(|a, b| {
        b.share
            .partial_cmp(&a.share)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    shares
}

/// Количество «символов-полосок» для процента (бар в ASCII).
fn bar_width(share: f64) -> usize {
    (share * 24.0).round() as usize
}

/// Рендерит отчёт «Why did AI say this?» — с ASCII-барами.
pub fn render_why(chain: &ContextChain) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Answer confidence: {:.0}%\n",
        chain.answer_confidence * 100.0
    ));
    if chain.seeds.is_empty() {
        out.push_str("Context: no memory seeds recorded.\n");
        return out;
    }
    out.push_str("Context:\n");
    for share in context_breakdown(chain) {
        let bar = "█".repeat(bar_width(share.share));
        out.push_str(&format!(
            "  {:<13} {:>3}% {}\n",
            share.kind.as_str(),
            (share.share * 100.0).round() as u32,
            bar
        ));
    }
    out.push_str("Seeds responsible:\n");
    let mut seeds = chain.seeds.clone();
    seeds.sort_by(|a, b| {
        b.weight
            .partial_cmp(&a.weight)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for seed in seeds.iter().take(12) {
        out.push_str(&format!(
            "  • [{}] {} ({:.0}%, {} tok)\n",
            seed.memory_id,
            seed.title,
            seed.weight * 100.0,
            seed.tokens
        ));
    }
    out
}

/// Рендерит этапы конвейера (хронология построения контекста).
pub fn render_stages(chain: &ContextChain) -> String {
    let mut out = String::from("Context pipeline:\n");
    for s in &chain.stages {
        out.push_str(&format!(
            "  {:>16}  {:>5} ms  {}\n",
            s.stage.as_str(),
            s.duration_ms,
            s.note
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_chain() -> ContextChain {
        let mut chain = ContextChain::begin(
            "How does authentication work?",
            "explain_architecture",
            "user",
        );
        chain.add_seed(
            ContextKind::Architecture,
            "mem-1",
            "Auth service design",
            0.9,
            500,
        );
        chain.add_seed(ContextKind::Decisions, "mem-2", "JWT decision", 0.6, 300);
        chain.add_seed(ContextKind::Code, "mem-3", "auth.rs impl", 0.4, 250);
        chain.add_seed(
            ContextKind::Semantic,
            "mem-4",
            "token refresh notes",
            0.15,
            120,
        );
        chain.add_seed(
            ContextKind::Historical,
            "mem-5",
            "old auth approach",
            0.08,
            80,
        );
        chain.pass_stage(ChainStage::Intent, 2, "intent classified");
        chain.pass_stage(ChainStage::MemorySeeds, 8, "5 seeds selected");
        chain.pass_stage(ChainStage::Ranking, 3, "seeds ranked");
        chain.pass_stage(ChainStage::TrustFilter, 1, "0 seeds quarantined");
        chain.pass_stage(ChainStage::TokenCompression, 4, "1250 → 1080 tokens");
        chain.pass_stage(ChainStage::FinalContext, 1, "context assembled");
        chain.pass_stage(ChainStage::Model, 850, "model call");
        chain.finish("Authentication uses JWT access tokens.", 0.87);
        chain
    }

    #[test]
    fn chain_stage_str_roundtrip() {
        let stages = [
            ChainStage::Request,
            ChainStage::Intent,
            ChainStage::MemorySeeds,
            ChainStage::GraphExpansion,
            ChainStage::Ranking,
            ChainStage::TrustFilter,
            ChainStage::TokenCompression,
            ChainStage::FinalContext,
            ChainStage::Model,
            ChainStage::Answer,
        ];
        for s in stages {
            assert_eq!(ChainStage::parse(s.as_str()), s);
        }
    }

    #[test]
    fn kind_str_roundtrip() {
        let kinds = [
            ContextKind::Architecture,
            ContextKind::Decisions,
            ContextKind::Code,
            ContextKind::Semantic,
            ContextKind::Historical,
            ContextKind::Working,
        ];
        for k in kinds {
            assert_eq!(ContextKind::parse(k.as_str()), k);
        }
    }

    #[test]
    fn breakdown_sums_to_one_and_orders_desc() {
        let chain = sample_chain();
        let shares = context_breakdown(&chain);
        let total: f64 = shares.iter().map(|s| s.share).sum();
        assert!((total - 1.0).abs() < 1e-9);
        // Отсортировано по убыванию.
        for w in shares.windows(2) {
            assert!(w[0].share >= w[1].share);
        }
        // Архитектура — самая весомая категория.
        assert_eq!(shares[0].kind, ContextKind::Architecture);
    }

    #[test]
    fn add_seed_accumulates_tokens() {
        let mut chain = ContextChain::begin("q", "i", "a");
        chain.add_seed(ContextKind::Code, "m1", "t1", 0.5, 100);
        chain.add_seed(ContextKind::Code, "m2", "t2", 0.5, 50);
        assert_eq!(chain.total_tokens, 150);
    }

    #[test]
    fn finish_sets_confidence_clamped() {
        let mut chain = ContextChain::begin("q", "i", "a");
        chain.finish("answer", 1.7);
        assert_eq!(chain.answer_confidence, 1.0);
        let mut chain2 = ContextChain::begin("q", "i", "a");
        chain2.finish("answer", -0.2);
        assert_eq!(chain2.answer_confidence, 0.0);
    }

    #[test]
    fn render_why_contains_bars_and_seeds() {
        let chain = sample_chain();
        let text = render_why(&chain);
        assert!(text.contains("Answer confidence: 87%"));
        assert!(text.contains("█"));
        assert!(text.contains("architecture"));
        assert!(text.contains("mem-1"));
    }

    #[test]
    fn render_stages_lists_pipeline() {
        let chain = sample_chain();
        let text = render_stages(&chain);
        assert!(text.contains("request"));
        assert!(text.contains("intent"));
        assert!(text.contains("final_context"));
        assert!(text.contains("answer"));
    }

    #[test]
    fn empty_seeds_breakdown_is_empty() {
        let chain = ContextChain::begin("q", "i", "a");
        assert!(context_breakdown(&chain).is_empty());
        let text = render_why(&chain);
        assert!(text.contains("no memory seeds"));
    }
}
