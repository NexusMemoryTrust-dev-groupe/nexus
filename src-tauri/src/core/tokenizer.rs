//! Real token counting.
//!
//! Why this exists
//! ---------------
//! Token counts used to be estimated as `text.len() / 4`, a rule of thumb that
//! only holds for English ASCII. It is badly wrong for the two things this app
//! handles constantly:
//!
//! * **Cyrillic** — every character is 2 UTF-8 bytes, so `len() / 4`
//!   *under*-counts Russian text by roughly 2x while the real tokenizer emits
//!   about one token per 2-3 characters.
//! * **Code and identifiers** — punctuation-dense text splits into many more
//!   tokens than the byte heuristic suggests.
//!
//! Since the savings figures shown to users are derived from these counts, a
//! wrong estimate means the headline number in the UI is fiction. We therefore
//! count with the same BPE tokenizer that ships with the embedding model
//! (`tokenizer.json`, WordPiece/BPE vocabulary), and only fall back to a
//! heuristic when that file is genuinely unavailable.
//!
//! The fallback is deliberately *character*-based rather than byte-based, so a
//! missing model degrades accuracy instead of inverting it.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tokenizers::Tokenizer;

/// How a count was produced. Surfaced to the UI so the savings panel can state
/// its own accuracy instead of implying false precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Counted with the real BPE vocabulary — exact for the embedding model.
    Exact,
    /// Counted with the character-class heuristic (model file unavailable).
    Estimated,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Exact => "exact",
            Method::Estimated => "estimated",
        }
    }

    pub fn is_exact(self) -> bool {
        matches!(self, Method::Exact)
    }
}

/// A token count together with how it was obtained.
#[derive(Debug, Clone, Copy)]
pub struct Count {
    pub tokens: u32,
    pub method: Method,
}

// ── Locating the vocabulary ─────────────────────────────────────────────────

/// Root of the fastembed model cache, honouring the same env var fastembed uses
/// so a custom cache location keeps working.
fn cache_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(dir) = std::env::var("FASTEMBED_CACHE_DIR")
        && !dir.trim().is_empty()
    {
        roots.push(PathBuf::from(dir));
    }

    // fastembed's own default, relative to the process working directory.
    roots.push(PathBuf::from(".fastembed_cache"));

    // Alongside the database, which is where an installed build keeps its data.
    if let Some(parent) = crate::db::db_path().parent() {
        roots.push(parent.join(".fastembed_cache"));
        roots.push(parent.join("models"));
    }

    // Next to the executable, for a portable install.
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        roots.push(dir.join(".fastembed_cache"));
    }

    roots
}

/// Depth-limited search for `tokenizer.json`.
///
/// The cache layout is `models--<org>--<name>/snapshots/<hash>/tokenizer.json`,
/// so a bounded walk finds it without risking an unbounded filesystem scan.
fn find_tokenizer_json(root: &Path, depth: usize) -> Option<PathBuf> {
    if depth == 0 || !root.is_dir() {
        return None;
    }

    let direct = root.join("tokenizer.json");
    if direct.is_file() {
        return Some(direct);
    }

    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir()
            && let Some(found) = find_tokenizer_json(&path, depth - 1)
        {
            return Some(found);
        }
    }
    None
}

fn locate_vocabulary() -> Option<PathBuf> {
    cache_roots()
        .iter()
        .find_map(|root| find_tokenizer_json(root, 5))
}

/// Loaded once per process. `None` means we could not find or parse the vocab
/// and must use the heuristic.
fn tokenizer() -> Option<&'static Tokenizer> {
    static INSTANCE: OnceLock<Option<Tokenizer>> = OnceLock::new();
    INSTANCE
        .get_or_init(|| {
            let path = locate_vocabulary()?;
            match Tokenizer::from_file(&path) {
                Ok(mut t) => {
                    // The embedding model ships a tokenizer configured for
                    // fixed-size inference: padding to 128 and truncation at
                    // the model's context limit. Left alone, every `encode`
                    // reports exactly 128 tokens regardless of input, and long
                    // text is silently clipped. Both must go for counting.
                    t.with_padding(None);
                    if t.with_truncation(None).is_err() {
                        tracing::warn!("Token counting: could not clear truncation");
                    }
                    tracing::info!("Token counting: exact, vocabulary at {}", path.display());
                    Some(t)
                }
                Err(e) => {
                    tracing::warn!(
                        "Token counting: failed to load {} ({e}), using heuristic",
                        path.display()
                    );
                    None
                }
            }
        })
        .as_ref()
}

/// True when exact counting is available.
pub fn is_exact() -> bool {
    tokenizer().is_some()
}

/// The method that [`count`] will currently use.
pub fn method() -> Method {
    if is_exact() {
        Method::Exact
    } else {
        Method::Estimated
    }
}

// ── Counting ────────────────────────────────────────────────────────────────

/// Longest text handed to the tokenizer in one go. Beyond this we count in
/// chunks: BPE is effectively additive across chunk boundaries (±1 token per
/// seam), and this keeps a pathological input from allocating without bound.
const CHUNK_BYTES: usize = 64 * 1024;

/// Heuristic used when the vocabulary is unavailable.
///
/// Calibrated per script rather than on raw byte length:
/// * Latin words average ~4 characters per token.
/// * Cyrillic averages ~1.8 characters per token (subword splits are finer
///   because the vocabulary is English-dominant). Kept below 2.0 chars per
///   token so the fallback never under-reports Cyrillic relative to the legacy
///   byte heuristic (`bytes/4` ≈ 0.5 tokens/character for 2-byte characters).
/// * CJK is close to one token per character.
/// * Runs of punctuation and symbols tokenize almost one-to-one.
fn estimate(text: &str) -> u32 {
    let mut latin = 0usize;
    let mut cyrillic = 0usize;
    let mut cjk = 0usize;
    let mut symbols = 0usize;
    let mut whitespace_runs = 0usize;
    let mut prev_ws = true;

    for ch in text.chars() {
        let is_ws = ch.is_whitespace();
        if is_ws {
            if !prev_ws {
                whitespace_runs += 1;
            }
            prev_ws = true;
            continue;
        }
        prev_ws = false;

        match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => latin += 1,
            '\u{0400}'..='\u{04FF}' | '\u{0500}'..='\u{052F}' => cyrillic += 1,
            '\u{3000}'..='\u{9FFF}' | '\u{AC00}'..='\u{D7AF}' => cjk += 1,
            c if c.is_alphanumeric() => latin += 1,
            _ => symbols += 1,
        }
    }

    // Integer arithmetic scaled by 10 to avoid float rounding drift.
    let tokens = (latin * 10 / 40) // 4.0 chars/token
        + (cyrillic * 10 / 18)     // 1.8 chars/token (see module docs)
        + cjk                      // ~1 token/char
        + symbols                  // punctuation is ~1:1
        + whitespace_runs / 4; // word boundaries add a little overhead

    tokens.max(if text.trim().is_empty() { 0 } else { 1 }) as u32
}

/// Count tokens in `text`.
///
/// Uses the real BPE vocabulary when available and the script-aware heuristic
/// otherwise. Never panics: a tokenizer error falls back to the estimate.
pub fn count(text: &str) -> u32 {
    if text.is_empty() {
        return 0;
    }

    let Some(tk) = tokenizer() else {
        return estimate(text);
    };

    if text.len() <= CHUNK_BYTES {
        return match tk.encode(text, false) {
            Ok(enc) => enc.len() as u32,
            Err(_) => estimate(text),
        };
    }

    // Chunk on character boundaries so multi-byte text is never split mid-char.
    let mut total: u32 = 0;
    let mut rest = text;
    while !rest.is_empty() {
        let take = crate::core::text::floor_char_boundary(rest, CHUNK_BYTES).max(1);
        let (head, tail) = rest.split_at(take);
        total = total.saturating_add(match tk.encode(head, false) {
            Ok(enc) => enc.len() as u32,
            Err(_) => estimate(head),
        });
        rest = tail;
    }
    total
}

/// Count with provenance attached.
pub fn count_with_method(text: &str) -> Count {
    Count {
        tokens: count(text),
        method: method(),
    }
}

/// Sum the token counts of several fragments.
pub fn count_all<'a, I: IntoIterator<Item = &'a str>>(parts: I) -> u32 {
    parts
        .into_iter()
        .fold(0u32, |acc, p| acc.saturating_add(count(p)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_is_zero_tokens() {
        assert_eq!(count(""), 0);
        assert_eq!(estimate(""), 0);
    }

    #[test]
    fn whitespace_only_is_zero() {
        assert_eq!(estimate("   \n\t "), 0);
    }

    #[test]
    fn any_real_content_is_at_least_one_token() {
        assert!(count("a") >= 1);
        assert!(estimate("a") >= 1);
        assert!(count("я") >= 1);
    }

    #[test]
    fn cyrillic_is_not_undercounted_like_the_old_heuristic() {
        // The bug being fixed: `len()/4` on Cyrillic divides a 2-byte-per-char
        // string by 4, producing roughly half a token per character.
        let text = "Пользователь создал проект и добавил документацию";
        let old_heuristic = (text.len() / 4) as u32;
        let counted = count(text);
        assert!(
            counted > old_heuristic,
            "Cyrillic must count higher than len/4: counted={counted}, old={old_heuristic}"
        );
    }

    #[test]
    fn longer_text_counts_more_tokens() {
        let short = count("short sentence");
        let long = count("short sentence with considerably more words appended to it");
        assert!(long > short, "short={short}, long={long}");
    }

    #[test]
    fn counting_is_additive_across_fragments() {
        let a = "first fragment";
        let b = "second fragment";
        let sum = count_all([a, b]);
        assert_eq!(sum, count(a) + count(b));
    }

    #[test]
    fn method_is_reported_consistently() {
        let c = count_with_method("hello world");
        assert_eq!(c.method.is_exact(), is_exact());
        assert_eq!(c.tokens, count("hello world"));
        assert!(matches!(c.method.as_str(), "exact" | "estimated"));
    }

    #[test]
    fn handles_text_larger_than_one_chunk() {
        let big = "предложение со словами ".repeat(4000);
        assert!(big.len() > CHUNK_BYTES);
        let counted = count(&big);
        assert!(counted > 0);
        // Must not panic and must scale with the input.
        assert!(counted > count("предложение со словами "));
    }

    #[test]
    fn multibyte_chunk_boundary_does_not_panic() {
        // Emoji are 4 bytes; a naive byte split would land mid-character.
        let text = "🚀".repeat(CHUNK_BYTES / 2);
        assert!(text.len() > CHUNK_BYTES);
        let _ = count(&text);
    }

    #[test]
    fn estimate_distinguishes_scripts() {
        // Same character count, different scripts: Cyrillic should not be
        // treated as cheaper than Latin.
        let latin = "abcdefghij";
        let cyr = "абвгдежзий";
        assert!(
            estimate(cyr) >= estimate(latin),
            "cyr={}, latin={}",
            estimate(cyr),
            estimate(latin)
        );
    }

    #[test]
    fn punctuation_heavy_code_is_not_undercounted() {
        let code = "let x = foo(bar[0], baz{qux});";
        assert!(estimate(code) >= 10, "got {}", estimate(code));
    }

    #[test]
    fn count_is_deterministic() {
        let text = "детерминированный подсчёт токенов";
        assert_eq!(count(text), count(text));
    }
}
