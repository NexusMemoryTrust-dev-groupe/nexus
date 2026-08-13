//! Layer classifier — deterministic signature scoring.
//!
//! Pure function: `classify(title, content, source, state, importance)`
//! returns a `LayerClassification { layer, confidence, reason }`.
//!
//! No LLM, no I/O, fully unit-testable. Each signal set votes for a layer;
//! the layer with the highest score wins. Confidence is the normalised
//! margin between the winner and the runner-up.

use crate::core::memory::layer::signals;
use crate::core::memory::types::{MemoryLayer, MemorySource, MemoryState};

/// Result of a classification pass.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerClassification {
    pub layer: MemoryLayer,
    /// 0.5 (weak / no signals) ..= 1.0 (landslide margin).
    pub confidence: f64,
    /// Human-readable reason: which signals fired (RU or EN).
    pub reason: String,
}

impl Default for LayerClassification {
    fn default() -> Self {
        Self {
            layer: MemoryLayer::Episodic,
            confidence: 0.5,
            reason: "no signals detected — defaulted to Episodic".to_string(),
        }
    }
}

/// Deterministic classifier over the six cognitive layers.
pub struct LayerClassifier;

impl LayerClassifier {
    /// Classify a memory from its title, content and metadata.
    ///
    /// Deterministic: identical inputs always produce identical output.
    pub fn classify(
        title: &str,
        content: &str,
        source: MemorySource,
        state: MemoryState,
        importance: f64,
    ) -> LayerClassification {
        let haystack = format!("{} {}\n{}", title, content, title).to_lowercase();

        let mut scores = [0i32; 6]; // Working, Episodic, Semantic, Procedural, Decision, Strategic

        // 1) Signature voting.
        scores[0] = signals::count_matches(&haystack, signals::WORKING_KEYWORDS);
        scores[1] = signals::count_matches(&haystack, signals::EPISODIC_KEYWORDS);
        scores[2] = signals::count_matches(&haystack, signals::SEMANTIC_KEYWORDS);
        scores[3] = signals::count_matches(&haystack, signals::PROCEDURAL_KEYWORDS);
        scores[4] = signals::count_matches(&haystack, signals::DECISION_KEYWORDS);
        scores[5] = signals::count_matches(&haystack, signals::STRATEGIC_KEYWORDS);

        // 2) Metadata boosts.
        let mut boost_reasons = Vec::new();
        if state == MemoryState::UserConfirmed {
            scores[2] += signals::CONFIRMED_BOOST_SEMANTIC; // Semantic
            scores[4] += signals::CONFIRMED_BOOST_DECISION; // Decision
            boost_reasons.push("user-confirmed");
        }
        match source {
            MemorySource::Meeting => {
                scores[4] += signals::MEETING_BOOST_DECISION;
                boost_reasons.push("source=meeting");
            }
            MemorySource::Git => {
                scores[1] += signals::GIT_BOOST_EPISODIC;
                boost_reasons.push("source=git");
            }
            _ => {}
        }
        if importance >= 0.8 {
            scores[5] += signals::IMPORTANCE_BOOST_STRATEGIC; // Strategic
            scores[4] += signals::IMPORTANCE_BOOST_DECISION; // Decision
            boost_reasons.push("importance>=0.8");
        }

        // 3) No signals at all → deterministic default (Episodic, 0.5).
        let total: i32 = scores.iter().sum();
        if total == 0 {
            return LayerClassification::default();
        }

        // 4) Pick the winner and the runner-up.
        let layers = [
            MemoryLayer::Working,
            MemoryLayer::Episodic,
            MemoryLayer::Semantic,
            MemoryLayer::Procedural,
            MemoryLayer::Decision,
            MemoryLayer::Strategic,
        ];
        let mut order: Vec<usize> = (0..6).collect();
        order.sort_by(|&a, &b| scores[b].cmp(&scores[a]).then(a.cmp(&b)));

        let winner = order[0];
        let runner_up = order[1];

        // 5) Confidence: normalised margin between winner and runner-up.
        //    Base 0.5; +0.5 * (margin / total) capped at 1.0.
        let margin = scores[winner] - scores[runner_up];
        let raw = 0.5 + 0.5 * (margin as f64 / total as f64);
        let confidence = raw.min(1.0);

        // 6) Reason: which signals fired for the winning layer.
        let fired = fired_signals(&haystack, winner);
        let mut reason = format!("signals: {}", fired.join(", "));
        if !boost_reasons.is_empty() {
            reason.push_str(&format!("; boosts: {}", boost_reasons.join(", ")));
        }

        LayerClassification {
            layer: layers[winner].clone(),
            confidence,
            reason,
        }
    }
}

/// List the signal keywords that fired for a layer (max 3 for a readable reason).
fn fired_signals(haystack: &str, layer_index: usize) -> Vec<String> {
    let keywords = match layer_index {
        0 => signals::WORKING_KEYWORDS,
        1 => signals::EPISODIC_KEYWORDS,
        2 => signals::SEMANTIC_KEYWORDS,
        3 => signals::PROCEDURAL_KEYWORDS,
        4 => signals::DECISION_KEYWORDS,
        _ => signals::STRATEGIC_KEYWORDS,
    };
    keywords
        .iter()
        .filter(|kw| haystack.contains(*kw))
        .take(3)
        .map(|kw| kw.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(t: &str, c: &str) -> LayerClassification {
        LayerClassifier::classify(t, c, MemorySource::Manual, MemoryState::Current, 0.5)
    }

    #[test]
    fn episodic_temporal_marker() {
        let r = classify(
            "Yesterday's experiment",
            "Вчера пробовали менять middleware",
        );
        assert_eq!(r.layer, MemoryLayer::Episodic);
        assert!(r.confidence >= 0.5);
        assert!(!r.reason.is_empty());
    }

    #[test]
    fn working_present_tense() {
        let r = classify("Auth bug", "Сейчас исправляем authentication bug");
        assert_eq!(r.layer, MemoryLayer::Working);
    }

    #[test]
    fn semantic_fact_presentation() {
        let r = classify("Auth design", "Auth реализован через JWT");
        assert_eq!(r.layer, MemoryLayer::Semantic);
    }

    #[test]
    fn procedural_sequence() {
        let r = classify(
            "Token refresh",
            "Сначала проверить, затем обновить: шаги 1-3",
        );
        assert_eq!(r.layer, MemoryLayer::Procedural);
    }

    #[test]
    fn decision_marker() {
        let r = classify("Redis", "3 августа отказались от Redis");
        assert_eq!(r.layer, MemoryLayer::Decision);
    }

    #[test]
    fn strategic_principle() {
        let r = classify(
            "Architecture",
            "Архитектура должна быть локальной, принцип №1",
        );
        assert_eq!(r.layer, MemoryLayer::Strategic);
    }

    #[test]
    fn user_confirmed_boosts_semantic() {
        let r = LayerClassifier::classify(
            "Auth uses JWT",
            "Authentication uses JWT tokens",
            MemorySource::Manual,
            MemoryState::UserConfirmed,
            0.5,
        );
        assert_eq!(r.layer, MemoryLayer::Semantic);
    }

    #[test]
    fn meeting_source_boosts_decision() {
        let r = LayerClassifier::classify(
            "Договорились по плану",
            "обсудили варианты",
            MemorySource::Meeting,
            MemoryState::Current,
            0.5,
        );
        assert_eq!(r.layer, MemoryLayer::Decision);
    }

    #[test]
    fn git_source_boosts_episodic() {
        let r = LayerClassifier::classify(
            "Commit log",
            "changes in middleware",
            MemorySource::Git,
            MemoryState::Current,
            0.5,
        );
        assert_eq!(r.layer, MemoryLayer::Episodic);
    }

    #[test]
    fn high_importance_boosts_strategic() {
        let r = LayerClassifier::classify(
            "Security model",
            "security model overview",
            MemorySource::Manual,
            MemoryState::Current,
            0.95,
        );
        assert_eq!(r.layer, MemoryLayer::Strategic);
    }

    #[test]
    fn no_signals_defaults_to_episodic() {
        let r = classify("lorem ipsum", "qwerty zxcvbn asdfgh jkl");
        assert_eq!(r.layer, MemoryLayer::Episodic);
        assert_eq!(r.confidence, 0.5);
        assert!(r.reason.contains("defaulted"));
    }

    #[test]
    fn deterministic_same_input_same_output() {
        let a = classify("Deploy notes", "Сегодня релизили версию 1.1.0");
        let b = classify("Deploy notes", "Сегодня релизили версию 1.1.0");
        assert_eq!(a, b);
    }

    #[test]
    fn confidence_is_bounded() {
        let r = classify("Deploy", "Вчера, сегодня и позавчера пробовали деплоить");
        assert!((0.5..=1.0).contains(&r.confidence), "{}", r.confidence);
    }

    #[test]
    fn episodic_beats_working_on_pure_event() {
        // Strong temporal stack should win over a single present-tense verb.
        let r = classify(
            "Incident",
            "Вчера случился инцидент, пробовали восстановить",
        );
        assert_eq!(r.layer, MemoryLayer::Episodic);
    }

    #[test]
    fn english_facts_classify_semantic() {
        let r = classify("API", "the API is built on Rust and supports WebSockets");
        assert_eq!(r.layer, MemoryLayer::Semantic);
    }

    #[test]
    fn english_decisions_classify_decision() {
        let r = classify(
            "Storage",
            "we decided to reject Postgres and chose SQLite instead",
        );
        assert_eq!(r.layer, MemoryLayer::Decision);
    }

    #[test]
    fn english_procedures_classify_procedural() {
        let r = classify(
            "Setup",
            "first install deps, then run build, steps are simple",
        );
        assert_eq!(r.layer, MemoryLayer::Procedural);
    }

    #[test]
    fn english_principles_classify_strategic() {
        let r = classify(
            "Policy",
            "privacy is a fundamental principle, we must never log secrets",
        );
        assert_eq!(r.layer, MemoryLayer::Strategic);
    }

    #[test]
    fn title_alone_can_classify() {
        // Title carries the signal even with empty content.
        let r = LayerClassifier::classify(
            "Решили отказаться от Redis",
            "",
            MemorySource::Manual,
            MemoryState::Current,
            0.5,
        );
        assert_eq!(r.layer, MemoryLayer::Decision);
    }

    #[test]
    fn reason_lists_fired_signals() {
        let r = classify("X", "вчера произошло ЧП");
        assert!(r.reason.contains("вчера") || r.reason.contains("произошло"));
    }

    #[test]
    fn mixed_ru_en_signals_combine() {
        // RU "вчера" + EN "tried" both vote Episodic.
        let r = classify("Retro", "Вчера we tried a new approach");
        assert_eq!(r.layer, MemoryLayer::Episodic);
    }
}
