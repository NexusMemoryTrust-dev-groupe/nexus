//! Real token counting.
//!
//! Why this exists
//! ---------------
//! Token counts used to be estimated as `text.len() / 4`, a rule of thumb that
//! only holds for English ASCII. It is badly wrong for the two things this app
//! handles constantly:
//!
//! * **Cyrillic and mixed code** — character heuristics are especially
//!   inaccurate here. A real tokenizer splits subword units, not raw
//!   characters, so the length of a string in bytes tells you little about how
//!   many tokens a model will see.
//! * **Code and identifiers** — punctuation-dense text splits into many more
//!   tokens than a byte heuristic suggests.
//!
//! Since the savings figures shown to users are derived from these counts, a
//! wrong estimate means the headline number in the UI is fiction. We therefore
//! count with the tokenizer of the *selected target model* whenever one is
//! available:
//!
//! * **GPT-family models** — tiktoken vocabularies (`o200k_base`,
//!   `cl100k_base`) embedded in the binary: exact, offline, no download.
//! * **The embedding model** — the BPE tokenizer shipped with it
//!   (`tokenizer.json` from the fastembed cache).
//! * **A local model** — a `tokenizer.json` found next to the model files.
//! * **Claude / Gemini** — no public tokenizer file is distributed, so these
//!   are reported as *estimated* unless the user provides one on disk.
//!
//! When no exact tokenizer is available, the result is labelled
//! [`Method::Estimated`] and counted with a script-aware character heuristic
//! rather than a raw byte length. The UI surfaces this label instead of
//! implying false precision.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use tokenizers::Tokenizer;

/// The model family whose tokenizer we count with. Determined by the model
/// name selected in the setup wizard (`ai.model` config).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    Claude,
    Gpt,
    Gemini,
    Local,
    Embedding,
}

impl Target {
    /// Resolve a target family from a model name (case-insensitive).
    pub fn from_model_name(model: &str) -> Target {
        let m = model.to_lowercase();
        if m.contains("claude") {
            Target::Claude
        } else if m.contains("gpt")
            || m.contains("o1")
            || m.contains("o3")
            || m.contains("o4")
            || m.contains("davinci")
            || m.contains("chatgpt")
        {
            Target::Gpt
        } else if m.contains("gemini") {
            Target::Gemini
        } else if m.contains("ollama")
            || m.contains("llama")
            || m.contains("qwen")
            || m.contains("mistral")
            || m.contains("local")
            || m.contains("deepseek")
        {
            Target::Local
        } else {
            Target::Embedding
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Target::Claude => "claude",
            Target::Gpt => "gpt",
            Target::Gemini => "gemini",
            Target::Local => "local",
            Target::Embedding => "embedding",
        }
    }
}

/// How a count was produced. Surfaced to the UI so the savings panel can state
/// its own accuracy instead of implying false precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Counted with the exact tokenizer of the selected target model.
    Exact,
    /// Counted with the character-class heuristic (no exact tokenizer).
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

/// A loaded counting engine. A tokenizer is only "exact" for the model family
/// it belongs to — the embedding vocabulary is not a stand-in for the target
/// LLM's vocabulary.
enum Engine {
    /// A `tokenizer.json` loaded from disk (embedding model or local model).
    HuggingFace(Box<Tokenizer>),
    /// An embedded tiktoken vocabulary for GPT-family models.
    Tiktoken(&'static tiktoken_rs::CoreBPE),
}

// ── Active target selection ─────────────────────────────────────────────────

struct ActiveState {
    target: Target,
    model: String,
}

static ACTIVE: OnceLock<RwLock<ActiveState>> = OnceLock::new();

/// Read the selected model from the config table (created by the setup wizard).
fn configured_model() -> Option<String> {
    let conn = crate::db::open_connection().ok()?;
    conn.query_row(
        "SELECT value FROM configuration_kv WHERE key = 'ai.model'",
        [],
        |row| row.get(0),
    )
    .ok()
}

fn active_state() -> &'static RwLock<ActiveState> {
    ACTIVE.get_or_init(|| {
        let model = configured_model().unwrap_or_default();
        RwLock::new(ActiveState {
            target: Target::from_model_name(&model),
            model,
        })
    })
}

/// Update the active target after the user changes the selected model.
/// Called by the `select_model` command once the config is persisted.
pub fn set_active_model(model: &str) {
    let model = model.trim().to_string();
    if model.is_empty() {
        return;
    }
    if let Some(lock) = ACTIVE.get() {
        let mut state = lock.write().unwrap();
        state.model = model.clone();
        state.target = Target::from_model_name(&model);
    } else {
        let _ = ACTIVE.set(RwLock::new(ActiveState {
            target: Target::from_model_name(&model),
            model,
        }));
    }
}

/// The model family currently being counted for.
pub fn active_target() -> Target {
    active_state().read().unwrap().target
}

/// The configured model name (as selected in the wizard), if any.
pub fn active_model_name() -> String {
    active_state().read().unwrap().model.clone()
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

// ── Building engines per target ─────────────────────────────────────────────

/// tiktoken vocabulary for GPT-family models. The vocabularies are embedded in
/// the binary via `include_str!`, so this is exact and works fully offline.
fn build_tiktoken_engine(model: &str) -> Option<Arc<Engine>> {
    let bpe = tiktoken_rs::bpe_for_model(model)
        .or_else(|_| {
            // Unrecognised name — fall back to the current default vocabulary.
            tiktoken_rs::bpe_for_tokenizer(tiktoken_rs::tokenizer::Tokenizer::O200kBase)
        })
        .ok()?;
    tracing::info!(
        "Token counting: exact, tiktoken vocabulary for model '{}'",
        model
    );
    Some(Arc::new(Engine::Tiktoken(bpe)))
}

/// A `tokenizer.json` found on disk (embedding model or local model).
fn build_hf_engine() -> Option<Arc<Engine>> {
    let path = locate_vocabulary()?;
    match Tokenizer::from_file(&path) {
        Ok(mut t) => {
            // The embedding model ships a tokenizer configured for fixed-size
            // inference: padding to 128 and truncation at the model's context
            // limit. Left alone, every `encode` reports exactly 128 tokens
            // regardless of input, and long text is silently clipped. Both must
            // go for counting.
            t.with_padding(None);
            if t.with_truncation(None).is_err() {
                tracing::warn!("Token counting: could not clear truncation");
            }
            tracing::info!("Token counting: exact, vocabulary at {}", path.display());
            Some(Arc::new(Engine::HuggingFace(Box::new(t))))
        }
        Err(e) => {
            tracing::warn!(
                "Token counting: failed to load {} ({e}), using heuristic",
                path.display()
            );
            None
        }
    }
}

/// Build the exact engine for a target family, or `None` when no exact
/// tokenizer is available (→ the result must be labelled `estimated`).
fn build_engine(target: Target) -> Option<Arc<Engine>> {
    match target {
        Target::Gpt => build_tiktoken_engine(&active_state().read().unwrap().model),
        // The embedding model's own tokenizer is exact *for the embedding
        // model*. Local models ship their own tokenizer.json next to the files.
        Target::Embedding | Target::Local => build_hf_engine(),
        // Claude and Gemini do not distribute public tokenizer files; without
        // one on disk we report estimated rather than pretending the embedding
        // vocabulary is theirs.
        Target::Claude | Target::Gemini => None,
    }
}

/// Per-target engine cache: engines are expensive to build (vocab parse) but
/// immutable once loaded, and the target can change at runtime.
fn engine_for(target: Target) -> Option<Arc<Engine>> {
    static ENGINES: OnceLock<RwLock<HashMap<Target, Option<Arc<Engine>>>>> = OnceLock::new();
    let map = ENGINES.get_or_init(|| RwLock::new(HashMap::new()));

    if let Some(cached) = map.read().unwrap().get(&target) {
        return cached.clone();
    }

    let engine = build_engine(target);
    map.write().unwrap().insert(target, engine.clone());
    engine
}

/// The engine for the currently selected target, if an exact one exists.
fn active_engine() -> Option<Arc<Engine>> {
    engine_for(active_target())
}

/// True when exact counting for the selected target is available.
pub fn is_exact() -> bool {
    active_engine().is_some()
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

/// Heuristic used when no exact tokenizer is available.
///
/// Calibrated per script rather than on raw byte length:
/// * Latin words average ~4 characters per token.
/// * Cyrillic averages ~1.8 characters per token — subword splits are finer
///   because the vocabulary is English-dominant. Kept below 2.0 chars per
///   token so the fallback never under-reports Cyrillic relative to the legacy
///   byte heuristic (`bytes/4`).
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

/// Count tokens in `text` with a single engine.
fn count_with_engine(engine: &Engine, text: &str) -> u32 {
    match engine {
        Engine::HuggingFace(tk) => match tk.encode(text, false) {
            Ok(enc) => enc.len() as u32,
            Err(_) => estimate(text),
        },
        Engine::Tiktoken(bpe) => bpe.encode_ordinary(text).len() as u32,
    }
}

/// Count tokens in `text`.
///
/// Uses the exact tokenizer of the selected target model when available and
/// the script-aware heuristic otherwise. Never panics: a tokenizer error falls
/// back to the estimate.
pub fn count(text: &str) -> u32 {
    if text.is_empty() {
        return 0;
    }

    let Some(engine) = active_engine() else {
        return estimate(text);
    };

    if text.len() <= CHUNK_BYTES {
        return count_with_engine(&engine, text);
    }

    // Chunk on character boundaries so multi-byte text is never split mid-char.
    let mut total: u32 = 0;
    let mut rest = text;
    while !rest.is_empty() {
        let take = crate::core::text::floor_char_boundary(rest, CHUNK_BYTES).max(1);
        let (head, tail) = rest.split_at(take);
        total = total.saturating_add(count_with_engine(&engine, head));
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
        // The bug being fixed: `len()/4` divides by 4 regardless of how the
        // script actually tokenizes, producing roughly half a token per
        // character for Cyrillic.
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

    #[test]
    fn target_is_detected_from_model_name() {
        assert_eq!(Target::from_model_name("claude-sonnet-4"), Target::Claude);
        assert_eq!(Target::from_model_name("gpt-4o"), Target::Gpt);
        assert_eq!(Target::from_model_name("o3-mini"), Target::Gpt);
        assert_eq!(Target::from_model_name("gemini-2.0-flash"), Target::Gemini);
        assert_eq!(Target::from_model_name("llama3.1:8b"), Target::Local);
        assert_eq!(
            Target::from_model_name("all-MiniLM-L6-v2"),
            Target::Embedding
        );
        assert_eq!(Target::from_model_name(""), Target::Embedding);
    }

    #[test]
    fn tiktoken_counts_gpt_text_exactly() {
        // The embedded tiktoken vocabulary must load offline and produce a
        // sensible, deterministic count for GPT-family models.
        let engine = build_tiktoken_engine("gpt-4o").expect("tiktoken o200k must load");
        match &*engine {
            Engine::Tiktoken(bpe) => {
                let n = bpe.encode_ordinary("hello world").len();
                assert!(n > 0, "gpt token count must be positive");
                assert_eq!(n, bpe.encode_ordinary("hello world").len());
            }
            _ => panic!("expected tiktoken engine"),
        }
    }
}
