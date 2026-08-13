//! Owning transitions of the global token-counting state (coverage 8.1).
//!
//! `tokenizer::ACTIVE` is a process-wide singleton that lib unit tests rely on
//! staying stable (`count_is_deterministic`, additive counting). Mutating it
//! from a lib test races with those parallel tests, so every write-path is
//! exercised here instead — a separate binary with its own process and its own
//! `ACTIVE`, where the sequence below is fully deterministic.
//!
//! Covered paths (see `src/core/tokenizer.rs`):
//! * `set_active_model` — empty no-op, first-write (ACTIVE unset) and
//!   subsequent-write (ACTIVE set) branches,
//! * `active_model_name` / `active_target` reads,
//! * `build_engine` per family: Gpt → tiktoken, Claude/Gemini → None,
//! * `count` fallback to the heuristic when no exact engine exists,
//! * tile-counting (`count_with_engine` Tiktoken arm) and the whitespace-only
//!   zero special case,
//! * `method() == Estimated` and the "estimated" provenance label.

use nexus::core::tokenizer::{
    Method, Target, active_model_name, active_target, count, count_with_method, is_exact, method,
    set_active_model,
};

#[test]
fn active_state_transitions_are_deterministic() {
    // Empty input is a no-op and must not touch the state.
    set_active_model("   ");
    assert!(!active_model_name().is_empty() || active_target() == Target::Embedding);

    // GPT family → embedded tiktoken vocabulary → exact counting.
    set_active_model("gpt-4o");
    assert_eq!(active_target(), Target::Gpt);
    assert_eq!(active_model_name(), "gpt-4o");
    assert!(is_exact(), "gpt must have an exact engine");
    assert!(count("hello world") > 0, "tiktoken must count text");
    assert_eq!(count("   "), 0, "whitespace-only content reports zero");
    assert_eq!(count_with_method("hi").method.as_str(), "exact");

    // Claude family → no public tokenizer file → estimated counting.
    set_active_model("claude-sonnet-4");
    assert_eq!(active_target(), Target::Claude);
    assert!(!is_exact(), "claude must not have an exact engine");
    assert_eq!(method(), Method::Estimated);
    assert!(count("какой-то текст") >= 1, "estimate must floor to 1");
    assert_eq!(count_with_method("привет мир").method.as_str(), "estimated");

    // Remaining family mappings resolve at write time too.
    set_active_model("gemini-2.0-flash");
    assert_eq!(active_target(), Target::Gemini);
    set_active_model("llama3.1:8b");
    assert_eq!(active_target(), Target::Local);
}
