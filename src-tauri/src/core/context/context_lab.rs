//! Nexus Context Lab — «научная лаборатория» качества контекста (System 6).
//!
//! Один вопрос → несколько стратегий построения контекста → метрики и
//! предсказание точности. Nexus начинает измерять не просто память, а
//! *качество* контекста:
//!
//! ```text
//! Query: "How does authentication work?"
//! Context A — 12 memories,  8,400 tokens, accuracy prediction: 81%
//! Context B —  7 memories,  3,200 tokens, accuracy prediction: 89%
//! Context C —  4 memories,  1,400 tokens, accuracy prediction: 91%
//! ```
//!
//! Чистые функции живут здесь и тестируются без БД: стратегии, метрики и
//! предиктор точности. Командный слой вызывает существующий ContextBuilder
//! с разными параметрами и сохраняет эксперименты в `context_lab_runs`.

use serde::{Deserialize, Serialize};

/// Параметры одной стратегии построения контекста.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextStrategy {
    pub name: String,
    pub max_tokens: u32,
    pub max_entities: u32,
    pub max_depth: u32,
    pub min_relevance: f64,
}

impl ContextStrategy {
    /// Компактная стратегия: только самое релевантное, мало шума.
    pub fn compact() -> Self {
        Self {
            name: "compact".to_string(),
            max_tokens: 2000,
            max_entities: 20,
            max_depth: 1,
            min_relevance: 0.5,
        }
    }

    /// Сбалансированная стратегия: стандартный набор по умолчанию.
    pub fn balanced() -> Self {
        Self {
            name: "balanced".to_string(),
            max_tokens: 4000,
            max_entities: 100,
            max_depth: 2,
            min_relevance: 0.3,
        }
    }

    /// Богатая стратегия: максимум контекста, глубокая раскладка графа.
    pub fn rich() -> Self {
        Self {
            name: "rich".to_string(),
            max_tokens: 8000,
            max_entities: 300,
            max_depth: 3,
            min_relevance: 0.15,
        }
    }

    /// Стратегии по умолчанию для лабораторного эксперимента.
    pub fn default_lab() -> Vec<Self> {
        vec![Self::compact(), Self::balanced(), Self::rich()]
    }
}

/// Метрики результата одной стратегии.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LabMetrics {
    /// Число memory-записей в контексте.
    pub memories: u32,
    /// Число сущностей графа в контексте.
    pub entities: u32,
    /// Токены итогового контекста.
    pub tokens: u32,
    /// Токены до сжатия (базовая линия экономии).
    pub baseline_tokens: u32,
    /// Средняя релевантность включённых элементов (0.0–1.0), если известна.
    pub avg_relevance: f64,
    /// Доля «зрелых» слоёв (Semantic/Procedural/Decision/Strategic) 0.0–1.0.
    pub maturity: f64,
    /// Предсказанная точность ответа с этим контекстом (0.0–1.0).
    pub accuracy: f64,
    /// Сколько секунд заняло построение.
    pub build_ms: u64,
}

/// Результат одного прогона стратегии по вопросу.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LabResult {
    pub query: String,
    pub strategy: ContextStrategy,
    pub metrics: LabMetrics,
    pub package_id: String,
}

/// Итог лабораторного эксперимента: все стратегии по одному вопросу.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LabExperiment {
    pub query: String,
    pub created_at: String,
    pub results: Vec<LabResult>,
}

impl LabExperiment {
    /// Стратегия с наивысшим предсказанным качеством.
    pub fn best(&self) -> Option<&LabResult> {
        self.results.iter().max_by(|a, b| {
            a.metrics
                .accuracy
                .partial_cmp(&b.metrics.accuracy)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Отсортировать результаты по точности (лучшие первыми).
    pub fn sorted_by_accuracy(&self) -> Vec<&LabResult> {
        let mut v: Vec<&LabResult> = self.results.iter().collect();
        v.sort_by(|a, b| {
            b.metrics
                .accuracy
                .partial_cmp(&a.metrics.accuracy)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        v
    }
}

// ── Предиктор точности (чистая функция) ────────────────────────────────────

/// Целевой бюджет токенов для «идеального» контекста: достаточно полно, но без
/// шума. Используется для штрафа за избыточность/недобор.
const TARGET_TOKENS: f64 = 3000.0;
/// Отклонение токенов, при котором фокус падает вдвое.
const TOKEN_SLACK: f64 = 2500.0;

/// Предсказывает точность ответа по метрикам контекста (0.0–1.0).
///
/// Составляющие:
/// * **Полнота** — чем больше релевантных элементов, тем выше (с насыщением).
/// * **Релевантность** — средний score включённых элементов.
/// * **Зрелость** — доля фактов/решений/процедур среди включённых записей.
/// * **Фокус** — штраф за перерасход или недобор токенов относительно цели.
pub fn predict_accuracy(memories: u32, tokens: u32, avg_relevance: f64, maturity: f64) -> f64 {
    // Полнота: 50 элементов дают ~0.95 насыщения, дальше рост мал.
    let coverage = 1.0 - (-(memories as f64) / 18.0).exp();

    // Фокус: гауссоподобный штраф за отклонение от целевого бюджета.
    let delta = (tokens as f64 - TARGET_TOKENS).abs() / TOKEN_SLACK;
    let focus = (-delta * delta).exp();

    let accuracy = 0.45 * coverage
        + 0.25 * avg_relevance.clamp(0.0, 1.0)
        + 0.15 * maturity.clamp(0.0, 1.0)
        + 0.15 * focus;

    accuracy.clamp(0.0, 1.0)
}

/// Космиметричный «качество-на-токен»: сколько точности даёт один токен.
pub fn efficiency_per_token(metrics: &LabMetrics) -> f64 {
    if metrics.tokens == 0 {
        return 0.0;
    }
    metrics.accuracy * 1000.0 / metrics.tokens as f64
}

/// Человекочитаемое сравнение стратегий (для /context-lab и контекстных пакетов).
pub fn render_comparison(exp: &LabExperiment) -> String {
    let mut out = String::with_capacity(512);
    out.push_str(&format!("Context Lab: \"{}\"\n", exp.query));
    for r in exp.sorted_by_accuracy() {
        out.push_str(&format!(
            "  {:>9} — {:>2} mem, {:>6} tok, accuracy {:.0}%, eff {:.2} pt/ktok\n",
            r.strategy.name,
            r.metrics.memories,
            r.metrics.tokens,
            r.metrics.accuracy * 100.0,
            r.metrics.accuracy * 1000.0 / r.metrics.tokens.max(1) as f64,
        ));
    }
    if let Some(best) = exp.best() {
        out.push_str(&format!(
            "  BEST: {} (accuracy {:.0}%)",
            best.strategy.name,
            best.metrics.accuracy * 100.0
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strategies_have_distinct_profiles() {
        let lab = ContextStrategy::default_lab();
        assert_eq!(lab.len(), 3);
        assert!(lab[0].max_tokens < lab[1].max_tokens);
        assert!(lab[1].max_tokens < lab[2].max_tokens);
        assert_eq!(lab[0].name, "compact");
    }

    #[test]
    fn accuracy_rises_with_relevance_and_maturity() {
        let low = predict_accuracy(5, 3000, 0.3, 0.2);
        let high = predict_accuracy(10, 3000, 0.9, 0.8);
        assert!(high > low, "low={low}, high={high}");
    }

    #[test]
    fn accuracy_saturates_with_memories() {
        let small = predict_accuracy(10, 3000, 0.6, 0.5);
        let large = predict_accuracy(100, 3000, 0.6, 0.5);
        assert!(large > small);
        // Насыщение: рост полноты от 50 к 100 записям почти нулевой.
        let medium = predict_accuracy(50, 3000, 0.6, 0.5);
        assert!(large - medium < 0.1, "medium={medium}, large={large}");
    }

    #[test]
    fn focus_penalizes_extremes() {
        let tiny = predict_accuracy(10, 100, 0.6, 0.5);
        let ideal = predict_accuracy(10, 3000, 0.6, 0.5);
        let huge = predict_accuracy(10, 50_000, 0.6, 0.5);
        assert!(ideal > tiny, "ideal={ideal}, tiny={tiny}");
        assert!(ideal > huge, "ideal={ideal}, huge={huge}");
    }

    #[test]
    fn accuracy_stays_in_range() {
        for m in [0, 1, 5, 50] {
            for t in [0, 500, 3000, 20_000] {
                let a = predict_accuracy(m, t, 0.5, 0.5);
                assert!((0.0..=1.0).contains(&a), "m={m} t={t} a={a}");
            }
        }
    }

    #[test]
    fn empty_context_predicts_low_accuracy() {
        let a = predict_accuracy(0, 0, 0.0, 0.0);
        assert!(a < 0.3, "empty context must not look accurate: {a}");
    }

    #[test]
    fn best_returns_highest_accuracy() {
        let mk = |name: &str, acc: f64| LabResult {
            query: "q".to_string(),
            strategy: ContextStrategy {
                name: name.to_string(),
                max_tokens: 100,
                max_entities: 1,
                max_depth: 1,
                min_relevance: 0.3,
            },
            metrics: LabMetrics {
                memories: 1,
                entities: 1,
                tokens: 100,
                baseline_tokens: 100,
                avg_relevance: 0.5,
                maturity: 0.5,
                accuracy: acc,
                build_ms: 1,
            },
            package_id: "p".to_string(),
        };
        let exp = LabExperiment {
            query: "q".to_string(),
            created_at: "now".to_string(),
            results: vec![mk("a", 0.7), mk("b", 0.9), mk("c", 0.5)],
        };
        assert_eq!(exp.best().unwrap().strategy.name, "b");
        let sorted = exp.sorted_by_accuracy();
        assert_eq!(sorted[0].strategy.name, "b");
        assert_eq!(sorted[2].strategy.name, "c");
    }

    #[test]
    fn efficiency_is_per_token() {
        let m1 = LabMetrics {
            memories: 5,
            entities: 5,
            tokens: 1000,
            baseline_tokens: 2000,
            avg_relevance: 0.6,
            maturity: 0.5,
            accuracy: 0.8,
            build_ms: 1,
        };
        let m2 = LabMetrics {
            tokens: 2000,
            ..m1.clone()
        };
        assert!(efficiency_per_token(&m1) > efficiency_per_token(&m2));
    }

    #[test]
    fn render_shows_all_strategies_and_best() {
        let exp = LabExperiment {
            query: "auth".to_string(),
            created_at: "now".to_string(),
            results: vec![
                LabResult {
                    query: "auth".to_string(),
                    strategy: ContextStrategy::compact(),
                    metrics: LabMetrics {
                        memories: 4,
                        entities: 3,
                        tokens: 1400,
                        baseline_tokens: 3000,
                        avg_relevance: 0.8,
                        maturity: 0.6,
                        accuracy: 0.91,
                        build_ms: 5,
                    },
                    package_id: "p1".to_string(),
                },
                LabResult {
                    query: "auth".to_string(),
                    strategy: ContextStrategy::rich(),
                    metrics: LabMetrics {
                        memories: 12,
                        entities: 20,
                        tokens: 8400,
                        baseline_tokens: 16_000,
                        avg_relevance: 0.6,
                        maturity: 0.5,
                        accuracy: 0.81,
                        build_ms: 12,
                    },
                    package_id: "p2".to_string(),
                },
            ],
        };
        let text = render_comparison(&exp);
        assert!(text.contains("compact"));
        assert!(text.contains("rich"));
        assert!(text.contains("BEST"));
    }
}
