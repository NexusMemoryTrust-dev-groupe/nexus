//! Render a context package for a model that is not driven through OpenCode.
//!
//! Why this exists
//! ---------------
//! Nexus assembles a ranked, compressed context package, but that package was
//! only ever reachable through our own MCP server. Anyone pasting into a web
//! chat, calling a vendor API directly, or using a different agent framework had
//! no way to benefit from it — which quietly tied the product's main advantage
//! to a single integration.
//!
//! Two formats, for two different consumers:
//!
//! * **Markdown** — what a human pastes into a chat window. Ordered by
//!   relevance, with the reasoning trail attached so the model (and the reader)
//!   can see why each item is present.
//! * **JSON** — what another program consumes. Stable field names, the full
//!   provenance record, and the measured token figures.
//!
//! Both are produced from the same package, so the two can never disagree.

use serde::{Deserialize, Serialize};

use crate::core::context::context_package::ContextPackage;
use crate::core::result::{AppError, Result};

/// Output format for an exported package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// Human-pasteable prompt.
    Markdown,
    /// Machine-readable payload.
    Json,
    /// Prompt only: no headings, no reasoning — just the facts, for models with
    /// a small context window where every token of scaffolding is a cost.
    Plain,
}

impl ExportFormat {
    /// Parse a format name coming from the UI or an MCP argument.
    pub fn parse(name: &str) -> Result<Self> {
        match name.trim().to_lowercase().as_str() {
            "markdown" | "md" => Ok(Self::Markdown),
            "json" => Ok(Self::Json),
            "plain" | "text" | "txt" => Ok(Self::Plain),
            other => Err(AppError::Validation(format!(
                "Unknown export format '{}'. Use markdown, json or plain.",
                other
            ))),
        }
    }

    /// Conventional file extension.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Json => "json",
            Self::Plain => "txt",
        }
    }
}

/// An exported package plus the numbers a caller needs to show.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Export {
    /// The rendered payload.
    pub content: String,
    /// Format actually used.
    pub format: ExportFormat,
    /// Tokens in the rendered payload, measured with the real vocabulary.
    ///
    /// Deliberately measured on the *output*, not copied from the package: the
    /// Markdown scaffolding costs tokens too, and a figure that ignored it would
    /// understate what the user is about to send.
    pub tokens: u32,
    /// How the count was produced: `exact` or `estimated`.
    pub token_method: String,
    /// Suggested filename, safe to write on Windows.
    pub filename: String,
}

/// Characters Windows forbids in a filename, plus the ones that make a path
/// ambiguous. Replaced rather than stripped so two different queries cannot
/// collapse onto the same name.
const UNSAFE_FILENAME_CHARS: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Build a filesystem-safe filename from the query.
///
/// Truncation is character-aware: a byte-indexed cut would panic on Cyrillic,
/// which is exactly the input this product expects.
fn safe_filename(query: &str, extension: &str) -> String {
    let cleaned: String = query
        .chars()
        .map(|c| {
            if UNSAFE_FILENAME_CHARS.contains(&c) || c.is_control() {
                '-'
            } else if c.is_whitespace() {
                '_'
            } else {
                c
            }
        })
        .collect();

    let trimmed = crate::core::text::truncate_chars(cleaned.trim_matches(['_', '-', '.']), 60);
    let stem = if trimmed.is_empty() { "context" } else { trimmed };
    format!("nexus-context-{}.{}", stem, extension)
}

/// Render the reasoning trail for one item as a compact inline note.
fn reasons_line(pkg: &ContextPackage, id: &str) -> Option<String> {
    let trace = pkg.provenance.get(id)?;
    if trace.reasons.is_empty() {
        return None;
    }
    let parts: Vec<String> = trace
        .reasons
        .iter()
        .map(|r| match r {
            crate::core::context::provenance::Reason::QueryMatch { query } => {
                format!("matches query \"{}\"", query)
            }
            crate::core::context::provenance::Reason::KeywordMatch { keyword } => {
                format!("keyword \"{}\"", keyword)
            }
            crate::core::context::provenance::Reason::GraphExpansion {
                from_title, hops, ..
            } => format!("linked from \"{}\" ({} hop(s))", from_title, hops),
            crate::core::context::provenance::Reason::MemorySearch { query } => {
                format!("memory search \"{}\"", query)
            }
            crate::core::context::provenance::Reason::RecentActivity { age_days } => {
                format!("updated {} day(s) ago", age_days)
            }
            crate::core::context::provenance::Reason::HighImportance { importance } => {
                format!("marked important ({:.2})", importance)
            }
        })
        .collect();
    Some(parts.join("; "))
}

/// Render as Markdown suitable for pasting into any chat model.
fn render_markdown(pkg: &ContextPackage) -> String {
    let mut out = String::with_capacity(4096);

    out.push_str("# Context\n\n");
    out.push_str(&format!("**Question:** {}\n\n", pkg.user_intent.query));

    if !pkg.memory_records.is_empty() {
        out.push_str("## What is already known\n\n");
        for m in &pkg.memory_records {
            out.push_str(&format!("### {}\n\n", m.title));
            if !m.summary.trim().is_empty() {
                out.push_str(&format!("_{}_\n\n", m.summary.trim()));
            }
            out.push_str(m.content.trim());
            out.push_str("\n\n");
            if let Some(why) = reasons_line(pkg, m.id.as_str()) {
                out.push_str(&format!("> Included because: {}\n\n", why));
            }
        }
    }

    if !pkg.entities.is_empty() {
        out.push_str("## Related things\n\n");
        for e in &pkg.entities {
            out.push_str(&format!(
                "- **{}** ({})",
                e.title,
                e.entity_type.as_str()
            ));
            if !e.description.trim().is_empty() {
                out.push_str(&format!(" — {}", e.description.trim()));
            }
            out.push('\n');
            if let Some(why) = reasons_line(pkg, e.id.as_str()) {
                out.push_str(&format!("  - _why:_ {}\n", why));
            }
        }
        out.push('\n');
    }

    if !pkg.relationships.is_empty() {
        // A title map turns opaque ids into something a model can reason about;
        // dumping raw UUIDs here would waste tokens and mean nothing.
        let title_of = |id: &str| -> String {
            pkg.entities
                .iter()
                .find(|e| e.id.as_str() == id)
                .map(|e| e.title.clone())
                .unwrap_or_else(|| id.to_string())
        };

        out.push_str("## How they connect\n\n");
        for r in &pkg.relationships {
            out.push_str(&format!(
                "- {} → {} ({})\n",
                title_of(r.source_entity_id.as_str()),
                title_of(r.target_entity_id.as_str()),
                r.relationship_type.as_str()
            ));
        }
        out.push('\n');
    }

    out.push_str("---\n\n");
    out.push_str(&format!(
        "_Assembled by Nexus: {} memor{}, {} entit{}, {} relationship{}._\n",
        pkg.memory_records.len(),
        if pkg.memory_records.len() == 1 { "y" } else { "ies" },
        pkg.entities.len(),
        if pkg.entities.len() == 1 { "y" } else { "ies" },
        pkg.relationships.len(),
        if pkg.relationships.len() == 1 { "" } else { "s" },
    ));

    out
}

/// Render without any scaffolding, for tight context windows.
fn render_plain(pkg: &ContextPackage) -> String {
    let mut out = String::with_capacity(2048);
    out.push_str(&pkg.user_intent.query);
    out.push_str("\n\n");

    for m in &pkg.memory_records {
        out.push_str(&m.title);
        out.push('\n');
        out.push_str(m.content.trim());
        out.push_str("\n\n");
    }
    for e in &pkg.entities {
        out.push_str(&e.title);
        if !e.description.trim().is_empty() {
            out.push_str(": ");
            out.push_str(e.description.trim());
        }
        out.push('\n');
    }
    out
}

/// Export a package in the requested format.
pub fn export(pkg: &ContextPackage, format: ExportFormat) -> Result<Export> {
    let content = match format {
        ExportFormat::Markdown => render_markdown(pkg),
        ExportFormat::Plain => render_plain(pkg),
        ExportFormat::Json => serde_json::to_string_pretty(pkg)
            .map_err(|e| AppError::Serialization(e.to_string()))?,
    };

    let tokens = crate::core::tokenizer::count(&content);

    Ok(Export {
        tokens,
        token_method: crate::core::tokenizer::method().as_str().to_string(),
        filename: safe_filename(&pkg.user_intent.query, format.extension()),
        format,
        content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::context_package::{IntentType, UserIntent};
    use crate::core::context::provenance::{ItemKind, Reason};
    use crate::core::graph::entity::Entity;
    use crate::core::graph::entity_types::EntityType;
    use crate::core::memory::memory_record::MemoryRecord;
    use crate::core::memory::types::MemorySource;

    fn pkg_with_content() -> ContextPackage {
        let mut pkg = ContextPackage::new(UserIntent {
            query: "release plan".into(),
            intent_type: IntentType::Search,
            confidence: 0.9,
            keywords: vec!["release".into()],
            temporal: None,
        });

        let e = Entity::new(
            EntityType::Project,
            "Nexus".into(),
            "Memory operating system".into(),
        );
        let m = MemoryRecord::new(
            "Ship in August".into(),
            "The installer must be signed before release.".into(),
            "user".into(),
            MemorySource::Manual,
        )
        .unwrap();

        pkg.provenance.record(
            e.id.as_str(),
            ItemKind::Entity,
            &e.title,
            Reason::QueryMatch { query: "release plan".into() },
        );
        pkg.provenance.record(
            m.id.as_str(),
            ItemKind::Memory,
            &m.title,
            Reason::MemorySearch { query: "release plan".into() },
        );

        pkg.entities = vec![e];
        pkg.memory_records = vec![m];
        pkg
    }

    // ── format parsing ──

    #[test]
    fn parses_every_supported_format_name() {
        assert_eq!(ExportFormat::parse("markdown").unwrap(), ExportFormat::Markdown);
        assert_eq!(ExportFormat::parse("md").unwrap(), ExportFormat::Markdown);
        assert_eq!(ExportFormat::parse("json").unwrap(), ExportFormat::Json);
        assert_eq!(ExportFormat::parse("plain").unwrap(), ExportFormat::Plain);
        assert_eq!(ExportFormat::parse("txt").unwrap(), ExportFormat::Plain);
    }

    #[test]
    fn format_parsing_is_case_and_space_insensitive() {
        assert_eq!(ExportFormat::parse("  MarkDown ").unwrap(), ExportFormat::Markdown);
    }

    #[test]
    fn unknown_format_is_rejected_with_guidance() {
        let err = ExportFormat::parse("pdf").unwrap_err().to_string();
        assert!(err.contains("markdown"), "error should list valid options: {err}");
    }

    // ── markdown ──

    #[test]
    fn markdown_contains_the_question_and_the_content() {
        let out = export(&pkg_with_content(), ExportFormat::Markdown).unwrap();
        assert!(out.content.contains("release plan"));
        assert!(out.content.contains("Ship in August"));
        assert!(out.content.contains("signed before release"));
        assert!(out.content.contains("Nexus"));
    }

    #[test]
    fn markdown_explains_why_each_item_is_present() {
        // The reasoning trail is the whole point of the feature: an export that
        // dropped it would be indistinguishable from a plain dump.
        let out = export(&pkg_with_content(), ExportFormat::Markdown).unwrap();
        assert!(out.content.contains("Included because"), "got: {}", out.content);
        assert!(out.content.contains("why:"), "got: {}", out.content);
    }

    #[test]
    fn markdown_resolves_relationship_ids_to_titles() {
        let mut pkg = pkg_with_content();
        let a = pkg.entities[0].clone();
        let b = Entity::new(EntityType::Person, "Alice".into(), String::new());
        let rel = crate::core::graph::relationship::Relationship::new(
            a.id.clone(),
            b.id.clone(),
            crate::core::graph::relationship_types::RelationshipType::RelatedTo,
            0.8,
        )
        .unwrap();
        pkg.entities.push(b);
        pkg.relationships = vec![rel];

        let out = export(&pkg, ExportFormat::Markdown).unwrap();
        assert!(out.content.contains("Nexus → Alice"), "got: {}", out.content);
        // The raw id must not leak into the prompt.
        assert!(!out.content.contains(a.id.as_str()));
    }

    #[test]
    fn markdown_pluralises_the_summary_correctly() {
        let out = export(&pkg_with_content(), ExportFormat::Markdown).unwrap();
        assert!(out.content.contains("1 memory"), "got: {}", out.content);
        assert!(out.content.contains("1 entity"), "got: {}", out.content);
    }

    #[test]
    fn empty_package_still_produces_valid_markdown() {
        let pkg = ContextPackage::new(UserIntent {
            query: "nothing here".into(),
            intent_type: IntentType::Search,
            confidence: 0.5,
            keywords: vec![],
            temporal: None,
        });
        let out = export(&pkg, ExportFormat::Markdown).unwrap();
        assert!(out.content.contains("nothing here"));
        assert!(out.content.contains("0 entities"));
    }

    // ── json ──

    #[test]
    fn json_round_trips_back_into_a_package() {
        let out = export(&pkg_with_content(), ExportFormat::Json).unwrap();
        let back: ContextPackage = serde_json::from_str(&out.content).unwrap();
        assert_eq!(back.user_intent.query, "release plan");
        assert_eq!(back.entities.len(), 1);
        assert_eq!(back.memory_records.len(), 1);
    }

    #[test]
    fn json_carries_the_provenance_for_other_programs() {
        let out = export(&pkg_with_content(), ExportFormat::Json).unwrap();
        assert!(out.content.contains("provenance"));
        assert!(out.content.contains("memorySearch"), "got: {}", out.content);
    }

    // ── plain ──

    #[test]
    fn plain_output_has_no_markdown_scaffolding() {
        let out = export(&pkg_with_content(), ExportFormat::Plain).unwrap();
        assert!(!out.content.contains('#'));
        assert!(!out.content.contains("**"));
        assert!(out.content.contains("Ship in August"));
    }

    #[test]
    fn plain_is_cheaper_than_markdown() {
        let md = export(&pkg_with_content(), ExportFormat::Markdown).unwrap();
        let plain = export(&pkg_with_content(), ExportFormat::Plain).unwrap();
        assert!(
            plain.tokens < md.tokens,
            "plain={} md={}",
            plain.tokens,
            md.tokens
        );
    }

    // ── token accounting ──

    #[test]
    fn tokens_are_measured_on_the_rendered_output() {
        let out = export(&pkg_with_content(), ExportFormat::Markdown).unwrap();
        assert_eq!(out.tokens, crate::core::tokenizer::count(&out.content));
        assert!(out.tokens > 0);
    }

    #[test]
    fn token_method_is_reported() {
        let out = export(&pkg_with_content(), ExportFormat::Markdown).unwrap();
        assert!(
            out.token_method == "exact" || out.token_method == "estimated",
            "unexpected method: {}",
            out.token_method
        );
    }

    // ── filenames ──

    #[test]
    fn filename_uses_the_right_extension() {
        assert!(export(&pkg_with_content(), ExportFormat::Markdown).unwrap().filename.ends_with(".md"));
        assert!(export(&pkg_with_content(), ExportFormat::Json).unwrap().filename.ends_with(".json"));
        assert!(export(&pkg_with_content(), ExportFormat::Plain).unwrap().filename.ends_with(".txt"));
    }

    #[test]
    fn filename_strips_characters_windows_forbids() {
        let name = safe_filename("a/b\\c:d*e?f\"g<h>i|j", "md");
        for c in UNSAFE_FILENAME_CHARS {
            assert!(!name.contains(c), "{c} survived in {name}");
        }
    }

    #[test]
    fn filename_handles_cyrillic_without_panicking() {
        // Byte-indexed truncation would split a 2-byte character and panic.
        let long = "Пользователь ".repeat(30);
        let name = safe_filename(&long, "md");
        assert!(std::str::from_utf8(name.as_bytes()).is_ok());
        assert!(name.starts_with("nexus-context-"));
        assert!(name.ends_with(".md"));
    }

    #[test]
    fn blank_query_falls_back_to_a_generic_filename() {
        assert_eq!(safe_filename("   ", "md"), "nexus-context-context.md");
    }

    #[test]
    fn filename_does_not_start_or_end_with_separators() {
        let name = safe_filename("///hello///", "md");
        assert!(!name.contains("--"), "got {name}");
    }
}
