//! Signal sets for the layer classifier — deterministic, language-aware
//! heuristics (RU + EN). Each signal votes for one or more cognitive layers.
//!
//! A signal is a phrase or word pattern that indicates the *kind* of cognitive
//! processing a memory went through:
//!
//! - Temporal markers ("yesterday", "вчера", "tried", "пробовали") → Episodic
//! - Present-tense doing ("fixing", "исправляем") → Working
//! - Fact presentation ("uses", "реализован") → Semantic
//! - Imperative/sequence ("first", "сначала", "steps") → Procedural
//! - Decisions ("decided", "решили") → Decision
//! - Principles ("must", "должно", "strategy") → Strategic

/// Number of points a matched keyword contributes to a layer.
pub const MATCH_WEIGHT: i32 = 2;
/// Points for a *strong* match (multi-word phrase or capitalised principle).
pub const STRONG_WEIGHT: i32 = 3;
/// Points when a "directional" word like "always"/"никогда" appears.
pub const DIRECTION_WEIGHT: i32 = 1;

/// Working — active task, the hot zone. Present-tense doing verbs.
pub const WORKING_KEYWORDS: &[&str] = &[
    // RU
    "исправляем",
    "чиним",
    "делаем сейчас",
    "работаю над",
    "работаем над",
    "сейчас делаю",
    "сейчас исправля",
    "в процессе",
    "текущая задача",
    // EN
    "fixing",
    "working on",
    "currently doing",
    "in progress",
    "right now",
    "working on",
    "debugging now",
    "implementing now",
];

/// Episodic — events, experiments, what was tried. Temporal markers.
pub const EPISODIC_KEYWORDS: &[&str] = &[
    // RU
    "вчера",
    "сегодня",
    "позавчера",
    "на прошлой неделе",
    "недавно",
    "случилось",
    "произошло",
    "пробовали",
    "попробовали",
    "эксперимент",
    "тестировали",
    "было так",
    "в тот день",
    "утром",
    "вечером",
    // EN
    "yesterday",
    "today",
    "last week",
    "recently",
    "happened",
    "occurred",
    "tried",
    "experiment",
    "tested",
    "was doing",
    "at that time",
];

/// Semantic — stable facts about the system or world. Fact presentation.
pub const SEMANTIC_KEYWORDS: &[&str] = &[
    // RU
    "реализован",
    "реализована",
    "реализовано",
    "использует",
    "является",
    "настроен",
    "настроена",
    "состоит из",
    "поддерживает",
    "работает через",
    "базируется на",
    "представляет собой",
    // EN
    "is implemented",
    "uses",
    "consists of",
    "supports",
    "is built on",
    "is based on",
    "configured with",
    "provides",
    "implements",
];

/// Procedural — how things are done. Imperatives, sequences, steps.
pub const PROCEDURAL_KEYWORDS: &[&str] = &[
    // RU
    "сначала",
    "затем",
    "потом",
    "нужно сделать",
    "шаги",
    "шаг",
    "порядок",
    "инструкция",
    "как сделать",
    "делается так",
    "процесс",
    "алгоритм",
    "рецепт",
    "последовательность",
    "следует делать",
    "повторяем",
    // EN
    "first",
    "then",
    "steps",
    "step by step",
    "how to",
    "procedure",
    "workflow",
    "process",
    "algorithm",
    "recipe",
    "sequence",
    "must be done",
];

/// Decision — a decision with its rationale.
pub const DECISION_KEYWORDS: &[&str] = &[
    // RU
    "решили",
    "решил",
    "отказались от",
    "отказался от",
    "выбрали",
    "выбрал",
    "приняли решение",
    "решение принято",
    "вместо",
    "предпочли",
    // EN
    "decided",
    "chose",
    "rejected",
    "opted for",
    "made a decision",
    "instead of",
    "selected",
    "we decided",
    "concluded",
];

/// Strategic — principles and long-term direction.
pub const STRATEGIC_KEYWORDS: &[&str] = &[
    // RU
    "принцип",
    "стратегия",
    "должно",
    "должна",
    "нельзя",
    "всегда",
    "никогда",
    "архитектура должна",
    "долгосрочн",
    "направление",
    "видение",
    "миссия",
    "фундаментальн",
    "стандарт",
    "политика",
    // EN
    "principle",
    "strategy",
    "must",
    "must not",
    "never",
    "always",
    "long-term",
    "vision",
    "mission",
    "fundamental",
    "standard",
    "policy",
    "architecture should",
];

/// MemoryState boost: user-confirmed memories lean Semantic/Decision.
pub const CONFIRMED_BOOST_SEMANTIC: i32 = 2;
pub const CONFIRMED_BOOST_DECISION: i32 = 2;

/// Source boosts: Meeting notes → Decision candidate, Git → Episodic.
pub const MEETING_BOOST_DECISION: i32 = 2;
pub const GIT_BOOST_EPISODIC: i32 = 2;

/// Importance boost: highly important memories lean Strategic/Decision.
pub const IMPORTANCE_BOOST_STRATEGIC: i32 = 2;
pub const IMPORTANCE_BOOST_DECISION: i32 = 1;

/// Count how many keywords from a set occur in a lowercased text.
pub fn count_matches(text: &str, keywords: &[&str]) -> i32 {
    keywords.iter().filter(|kw| text.contains(*kw)).count() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_matches_finds_multiword_phrases() {
        let text = "вчера пробовали менять middleware, сегодня чиним";
        assert_eq!(count_matches(text, EPISODIC_KEYWORDS), 3); // вчера, пробовали, сегодня
        assert_eq!(count_matches(text, WORKING_KEYWORDS), 1); // чиним
        assert_eq!(count_matches(text, SEMANTIC_KEYWORDS), 0);
    }

    #[test]
    fn count_matches_case_insensitive_en() {
        let text = "Yesterday we TRIED a new router";
        assert_eq!(count_matches(&text.to_lowercase(), EPISODIC_KEYWORDS), 2);
    }

    #[test]
    fn count_matches_zero_on_unrelated_text() {
        let text = "the quick brown fox jumps over the lazy dog";
        assert_eq!(count_matches(text, DECISION_KEYWORDS), 0);
        assert_eq!(count_matches(text, STRATEGIC_KEYWORDS), 0);
    }

    #[test]
    fn every_layer_has_signals() {
        assert!(!WORKING_KEYWORDS.is_empty());
        assert!(!EPISODIC_KEYWORDS.is_empty());
        assert!(!SEMANTIC_KEYWORDS.is_empty());
        assert!(!PROCEDURAL_KEYWORDS.is_empty());
        assert!(!DECISION_KEYWORDS.is_empty());
        assert!(!STRATEGIC_KEYWORDS.is_empty());
    }

    #[test]
    fn keywords_are_lowercased() {
        let all: Vec<&str> = WORKING_KEYWORDS
            .iter()
            .chain(EPISODIC_KEYWORDS)
            .chain(SEMANTIC_KEYWORDS)
            .chain(PROCEDURAL_KEYWORDS)
            .chain(DECISION_KEYWORDS)
            .chain(STRATEGIC_KEYWORDS)
            .copied()
            .collect();
        for kw in all {
            assert_eq!(kw.to_lowercase(), kw, "keyword must be lowercase: {kw}");
        }
    }
}
