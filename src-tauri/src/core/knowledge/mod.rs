//! Project knowledge base: RAG documents, AGENTS.md-style instructions, skills,
//! and the code graph.
//!
//! Implements the ideas from the team's feedback ("RAG для ИИ", "AGENTS.md",
//! "скиллы вместо раздутого MCP-контекста"):
//!
//! * [`documents`] — import and index project `.md`/`.txt` files so an agent
//!   can answer questions about the project by semantic search, not just about
//!   its own memories.
//! * [`agents`] — AGENTS.md-style instruction files. Content is attached to
//!   every context package so the AI follows the project's rules, and a
//!   generator builds one from the system's own data (the "documentation skill").
//! * [`skills`] — runnable commands (JS scripts etc.) that agents can invoke
//!   without carrying the full tool surface in context.
//! * [`code_graph`] — a structured map of source files: symbols extracted by
//!   the language parsers plus dependency edges (`import`/`use`/`#include`),
//!   so an agent can answer "what depends on X?" without dumping code into
//!   semantic memory.

pub mod agents;
pub mod code_graph;
pub mod documents;
pub mod skills;

pub use agents::{AgentsFile, AgentsRepository, generate_agents_file};
pub use code_graph::{CodeDependency, CodeFile, CodeGraphRepository, CodeSymbol};
pub use documents::{ImportReport, ProjectDocument, ProjectDocumentRepository};
pub use skills::{Skill, SkillOutput, SkillRepository, SkillRunner};

/// Small stable checksum for change detection on re-import.
/// SHA-256 hex of the given bytes (first 32 chars — plenty for dedup).
pub fn content_checksum(content: &str) -> String {
    use ring::digest::{SHA256, digest};
    let d = digest(&SHA256, content.as_bytes());
    d.as_ref()
        .iter()
        .take(16)
        .map(|b| format!("{:02x}", b))
        .collect()
}
