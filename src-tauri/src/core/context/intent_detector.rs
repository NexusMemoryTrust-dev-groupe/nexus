use crate::core::context::context_package::{IntentType, UserIntent};

/// Detects user intent from a query string using keyword-based heuristics.
/// Now with keyword extraction and temporal reasoning.
pub struct IntentDetector;

impl Default for IntentDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl IntentDetector {
    pub fn new() -> Self {
        Self
    }

    /// Detect intent from a query string.
    pub fn detect(&self, query: &str) -> UserIntent {
        let intent_type = self.classify_intent(query);
        let confidence = self.calculate_confidence(query, &intent_type);
        let keywords = self.extract_keywords(query);
        let temporal = self.detect_temporal(query);

        UserIntent {
            query: query.to_string(),
            intent_type,
            confidence,
            keywords,
            temporal,
        }
    }

    /// Classify the intent type based on keywords.
    pub fn classify_intent(&self, query: &str) -> IntentType {
        let lower = query.to_lowercase();

        if lower.contains("найди")
            || lower.contains("поиск")
            || lower.contains("где")
            || lower.contains("find")
            || lower.contains("search")
            || lower.contains("where")
        {
            IntentType::Search
        } else if lower.contains("проанализируй")
            || lower.contains("сравни")
            || lower.contains("статистика")
            || lower.contains("analyze")
            || lower.contains("compare")
            || lower.contains("statistics")
        {
            IntentType::Analysis
        } else if lower.contains("реши")
            || lower.contains("выбери")
            || lower.contains("стоит ли")
            || lower.contains("decide")
            || lower.contains("choose")
            || lower.contains("should")
        {
            IntentType::Decision
        } else if lower.contains("создай")
            || lower.contains("добавь")
            || lower.contains("новый")
            || lower.contains("create")
            || lower.contains("add")
            || lower.contains("new")
        {
            IntentType::Creation
        } else if lower.contains("обнови")
            || lower.contains("измени")
            || lower.contains("обновить")
            || lower.contains("update")
            || lower.contains("change")
            || lower.contains("modify")
        {
            IntentType::Update
        } else {
            IntentType::Exploration
        }
    }

    /// Extract keywords from query (remove stop words, extract meaningful terms).
    pub fn extract_keywords(&self, query: &str) -> Vec<String> {
        let stop_words = [
            // English
            "the",
            "a",
            "an",
            "and",
            "or",
            "but",
            "in",
            "on",
            "at",
            "to",
            "for",
            "of",
            "with",
            "by",
            "from",
            "as",
            "is",
            "was",
            "are",
            "were",
            "be",
            "been",
            "being",
            "have",
            "has",
            "had",
            "do",
            "does",
            "did",
            "will",
            "would",
            "could",
            "should",
            "may",
            "might",
            "can",
            "shall",
            "all",
            "this",
            "that",
            "these",
            "those",
            "it",
            "its",
            "my",
            "your",
            "his",
            "her",
            "our",
            "their",
            "what",
            "which",
            "who",
            "whom",
            "how",
            "when",
            "where",
            "why",
            "not",
            "no",
            "nor",
            "if",
            "then",
            "else",
            "than",
            "too",
            "very",
            "just",
            "about",
            "also",
            "now",
            "here",
            "there",
            "only",
            "own",
            "same",
            "so",
            "some",
            "such",
            "into",
            "over",
            "after",
            "before",
            "between",
            "through",
            "during",
            "without",
            "again",
            "further",
            "once",
            "each",
            "every",
            "both",
            "few",
            "more",
            "most",
            "other",
            "any",
            "much",
            "many",
            "well",
            "back",
            "even",
            "still",
            "new",
            // Russian
            "и",
            "а",
            "но",
            "в",
            "на",
            "с",
            "для",
            "от",
            "по",
            "из",
            "к",
            "что",
            "как",
            "где",
            "когда",
            "почему",
            "кто",
            "чем",
            "это",
            "все",
            "не",
            "ни",
            "да",
            "нет",
            "уже",
            "еще",
            "тоже",
            "также",
            "только",
            "все",
            "всё",
            "каждый",
            "каждая",
            "каждое",
            "можно",
            "нужно",
            "надо",
            "быть",
            "был",
            "была",
            "было",
            "были",
            "будет",
            "будут",
            "есть",
            "нет",
            "быть",
            "является",
            "являются",
            "этот",
            "эта",
            "это",
            "эти",
            "тот",
            "та",
            "те",
            "такой",
            "такая",
            "такие",
            "какой",
            "какая",
            "какие",
            "чей",
            "чья",
            "чьи",
            "мой",
            "моя",
            "мои",
            "твой",
            "твоя",
            "твои",
            "наш",
            "наша",
            "наши",
            "ваш",
            "ваша",
            "ваши",
            "его",
            "её",
            "их",
            "себя",
            "себе",
            "собой",
            "сам",
            "сама",
            "само",
            "сами",
            "тут",
            "там",
            "здесь",
            "потом",
            "тогда",
            "сейчас",
            "потому",
            "поэтому",
            "однако",
            "если",
            "чтобы",
            "чтоб",
            "который",
            "которая",
            "которое",
            "которые",
        ];

        let mut keywords: Vec<String> = query
            .split_whitespace()
            .map(|w| {
                w.to_lowercase()
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                    .collect::<String>()
            })
            .filter(|w| w.len() > 2 && !stop_words.contains(&w.as_str()))
            .collect();
        // `dedup()` only removes *adjacent* duplicates, so "rust vs rust" kept
        // both copies. Retain first occurrence of each word, preserving order.
        let mut seen = std::collections::HashSet::new();
        keywords.retain(|w| seen.insert(w.clone()));
        keywords
    }

    /// Detect temporal references in query.
    pub fn detect_temporal(&self, query: &str) -> Option<String> {
        let lower = query.to_lowercase();

        if lower.contains("неделю назад") || lower.contains("week ago") {
            Some("1w_ago".to_string())
        } else if lower.contains("месяц назад") || lower.contains("month ago") {
            Some("1m_ago".to_string())
        } else if lower.contains("вчера") || lower.contains("yesterday") {
            Some("1d_ago".to_string())
        } else if lower.contains("сегодня") || lower.contains("today") {
            Some("today".to_string())
        } else if lower.contains("позавчера") || lower.contains("day before yesterday") {
            Some("2d_ago".to_string())
        } else if lower.contains("неделя") || lower.contains("week") {
            Some("this_week".to_string())
        } else if lower.contains("месяц") || lower.contains("month") {
            Some("this_month".to_string())
        } else if lower.contains("год") || lower.contains("year") {
            Some("this_year".to_string())
        } else {
            None
        }
    }

    fn calculate_confidence(&self, query: &str, _intent_type: &IntentType) -> f64 {
        if query.len() < 5 {
            0.3
        } else if query.len() < 15 {
            0.6
        } else {
            0.8
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_search() {
        let d = IntentDetector::new();
        let intent = d.detect("найди проекты");
        assert_eq!(intent.intent_type, IntentType::Search);
        assert!(intent.confidence > 0.0);
    }

    #[test]
    fn detect_analysis() {
        let d = IntentDetector::new();
        let intent = d.detect("проанализируй статистику проекта");
        assert_eq!(intent.intent_type, IntentType::Analysis);
    }

    #[test]
    fn detect_decision() {
        let d = IntentDetector::new();
        let intent = d.detect("стоит ли выбирать Rust");
        assert_eq!(intent.intent_type, IntentType::Decision);
    }

    #[test]
    fn detect_creation() {
        let d = IntentDetector::new();
        let intent = d.detect("создай новый документ");
        assert_eq!(intent.intent_type, IntentType::Creation);
    }

    #[test]
    fn detect_update() {
        let d = IntentDetector::new();
        let intent = d.detect("обнови информацию");
        assert_eq!(intent.intent_type, IntentType::Update);
    }

    #[test]
    fn detect_exploration_default() {
        let d = IntentDetector::new();
        let intent = d.detect("расскажи о проекте");
        assert_eq!(intent.intent_type, IntentType::Exploration);
    }

    #[test]
    fn detect_english_keywords() {
        let d = IntentDetector::new();
        assert_eq!(d.detect("find all tasks").intent_type, IntentType::Search);
        assert_eq!(
            d.detect("analyze performance").intent_type,
            IntentType::Analysis
        );
        assert_eq!(
            d.detect("create new project").intent_type,
            IntentType::Creation
        );
        assert_eq!(d.detect("update status").intent_type, IntentType::Update);
    }

    #[test]
    fn confidence_short_query() {
        let d = IntentDetector::new();
        let intent = d.detect("hi");
        assert!(intent.confidence < 0.5);
    }

    #[test]
    fn confidence_long_query() {
        let d = IntentDetector::new();
        let intent = d.detect("find all project tasks created this week");
        assert!(intent.confidence >= 0.8);
    }

    #[test]
    fn detect_preserves_query() {
        let d = IntentDetector::new();
        let intent = d.detect("my custom query");
        assert_eq!(intent.query, "my custom query");
    }

    #[test]
    fn extract_keywords_basic() {
        let d = IntentDetector::new();
        let keywords = d.extract_keywords("find all project tasks");
        assert!(keywords.contains(&"find".to_string()));
        assert!(keywords.contains(&"project".to_string()));
        assert!(keywords.contains(&"tasks".to_string()));
        assert!(!keywords.contains(&"all".to_string())); // stop word
    }

    #[test]
    fn extract_keywords_russian() {
        let d = IntentDetector::new();
        let keywords = d.extract_keywords("найди все проекты");
        assert!(keywords.contains(&"найди".to_string()));
        assert!(keywords.contains(&"проекты".to_string()));
        assert!(!keywords.contains(&"все".to_string())); // stop word
    }

    #[test]
    fn detect_temporal_week_ago() {
        let d = IntentDetector::new();
        let temporal = d.detect_temporal("что мы обсуждали неделю назад");
        assert_eq!(temporal, Some("1w_ago".to_string()));
    }

    #[test]
    fn detect_temporal_today() {
        let d = IntentDetector::new();
        let temporal = d.detect_temporal("что сделано сегодня");
        assert_eq!(temporal, Some("today".to_string()));
    }

    #[test]
    fn detect_temporal_none() {
        let d = IntentDetector::new();
        let temporal = d.detect_temporal("расскажи о проекте");
        assert_eq!(temporal, None);
    }
}
