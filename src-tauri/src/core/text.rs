//! UTF-8-safe text utilities.
//!
//! Rust string slicing (`&s[..n]`) panics if `n` lands inside a multi-byte
//! character. Cyrillic is 2 bytes per char, emoji up to 4, so any byte-indexed
//! truncation of user content is a latent panic. These helpers clamp the index
//! down to the nearest character boundary instead.

/// Largest index `<= max_bytes` that is a valid UTF-8 character boundary.
/// Returns `text.len()` when the text already fits.
pub fn floor_char_boundary(text: &str, max_bytes: usize) -> usize {
    if max_bytes >= text.len() {
        return text.len();
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    end
}

/// Truncate to at most `max_bytes` bytes without splitting a character.
/// Returns the whole string when it already fits.
pub fn truncate_chars(text: &str, max_bytes: usize) -> &str {
    &text[..floor_char_boundary(text, max_bytes)]
}

/// Truncate and append `suffix` when the text was actually shortened.
pub fn truncate_with_suffix(text: &str, max_bytes: usize, suffix: &str) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    format!("{}{}", truncate_chars(text, max_bytes), suffix)
}

// ═══════════════════════════════════════════════════════════════
//  Query normalization (shared by intent detection and search)
// ═══════════════════════════════════════════════════════════════

/// Words that carry no search signal. Shared between `IntentDetector` keyword
/// extraction and entity/memory search so every path treats a query the same
/// way. Kept as a flat lowercase list — membership tests dominate over size.
pub const STOP_WORDS: &[&str] = &[
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
    "ещё",
    "тоже",
    "также",
    "только",
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
    "является",
    "являются",
    "этот",
    "эта",
    "эти",
    "тот",
    "та",
    "те",
    "такой",
    "такая",
    "такие",
    "такое",
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
    "ее",
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
    "всё",
    "если",
    "чтобы",
    "чтоб",
    "который",
    "которая",
    "которое",
    "которые",
];

/// Split a free-text query into normalized search keywords.
///
/// Each word is lowercased, stripped of punctuation (alphanumerics, `-` and `_`
/// survive — `focus-tracker` must stay intact), dropped when it is a stop word
/// or shorter than 3 characters, and deduplicated while preserving order. An
/// empty result means the query carries no searchable signal.
pub fn normalize_search_words(query: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    query
        .split_whitespace()
        .map(|w| {
            w.to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect::<String>()
        })
        .filter(|w| w.len() > 2 && !STOP_WORDS.contains(&w.as_str()))
        .filter(|w| seen.insert(w.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_under_limit_is_untouched() {
        assert_eq!(truncate_chars("hello", 10), "hello");
        assert_eq!(truncate_with_suffix("hello", 10, "..."), "hello");
    }

    #[test]
    fn ascii_over_limit_is_cut_exactly() {
        assert_eq!(truncate_chars("hello world", 5), "hello");
        assert_eq!(truncate_with_suffix("hello world", 5, "..."), "hello...");
    }

    #[test]
    fn cyrillic_never_splits_a_char() {
        // Each Cyrillic char is 2 bytes; cutting at 5 would split one.
        let text = "Привет";
        let cut = truncate_chars(text, 5);
        assert!(std::str::from_utf8(cut.as_bytes()).is_ok());
        assert_eq!(cut, "Пр");
    }

    #[test]
    fn emoji_never_splits_a_char() {
        let text = "ab🚀cd";
        for limit in 0..text.len() + 2 {
            let cut = truncate_chars(text, limit);
            assert!(
                std::str::from_utf8(cut.as_bytes()).is_ok(),
                "limit={}",
                limit
            );
            assert!(cut.len() <= limit.min(text.len()), "limit={}", limit);
        }
    }

    #[test]
    fn exact_boundary_keeps_everything() {
        let text = "Привет"; // 12 bytes
        assert_eq!(truncate_chars(text, 12), text);
        assert_eq!(truncate_with_suffix(text, 12, "..."), text);
    }

    #[test]
    fn zero_limit_yields_empty() {
        assert_eq!(truncate_chars("Привет", 0), "");
    }

    #[test]
    fn floor_boundary_clamps_to_len() {
        assert_eq!(floor_char_boundary("abc", 99), 3);
    }

    #[test]
    fn normalize_removes_stopwords_and_punctuation() {
        // Стоп-слова и пунктуация уходят; «focus-tracker» с дефисом сохраняется.
        let words =
            normalize_search_words("Что такое focus-tracker, на чём он написан и как работает?");
        assert!(words.contains(&"focus-tracker".to_string()));
        assert!(
            !words
                .iter()
                .any(|w| w == "что" || w == "на" || w == "и" || w == "как")
        );
    }

    #[test]
    fn normalize_keeps_meaningful_english_words() {
        let words = normalize_search_words("find all project tasks");
        assert!(words.contains(&"find".to_string()));
        assert!(words.contains(&"project".to_string()));
        assert!(words.contains(&"tasks".to_string()));
        assert!(!words.contains(&"all".to_string()));
    }

    #[test]
    fn normalize_deduplicates_and_orders() {
        let words = normalize_search_words("rust vs rust project");
        assert_eq!(words.iter().filter(|w| *w == "rust").count(), 1);
        assert_eq!(words[0], "rust");
    }

    #[test]
    fn normalize_empty_query_yields_nothing() {
        assert!(normalize_search_words("и на по из").is_empty());
        assert!(normalize_search_words("   ").is_empty());
    }
}
