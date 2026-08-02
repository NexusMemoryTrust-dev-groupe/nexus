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
            assert!(std::str::from_utf8(cut.as_bytes()).is_ok(), "limit={}", limit);
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
}
