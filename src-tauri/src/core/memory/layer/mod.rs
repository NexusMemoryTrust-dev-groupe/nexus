//! Cognitive layer classification — deterministic signature scoring.
//!
//! `LayerClassifier::classify(title, content, source, state, importance)`
//! assigns one of the six cognitive layers (Working / Episodic / Semantic /
//! Procedural / Decision / Strategic) with a confidence score and a
//! human-readable reason. Pure, deterministic, no LLM, no I/O.

pub mod classifier;
pub mod signals;

pub use classifier::{LayerClassification, LayerClassifier};

/// Static catalogue of the six cognitive layers: name, meaning, and what
/// promotes a memory into them. Mirrors the backend `MemoryLayer` taxonomy and
/// is used by the `nexus_layers_list` MCP tool (and can be reused by the UI).
pub struct LayerInfo {
    pub name: &'static str,
    pub meaning: &'static str,
    pub promotes: &'static str,
}

/// The six cognitive layers in ladder order (working → strategic).
pub const LAYER_CATALOG: &[LayerInfo] = &[
    LayerInfo {
        name: "Working",
        meaning: "Active task — the hot zone of what is being done right now.",
        promotes: "When the task is finished, verify it and it becomes Episodic or Semantic.",
    },
    LayerInfo {
        name: "Episodic",
        meaning: "Events, experiments, what was tried — raw experience.",
        promotes: "Verify it and it becomes Semantic.",
    },
    LayerInfo {
        name: "Semantic",
        meaning: "Stable facts about the system or the world.",
        promotes: "When you learn how to do it, it becomes Procedural.",
    },
    LayerInfo {
        name: "Procedural",
        meaning: "How things are done here — order of actions, steps.",
        promotes: "When you choose between options, it becomes Decision.",
    },
    LayerInfo {
        name: "Decision",
        meaning: "A decision with its rationale.",
        promotes: "Hold across projects and it hardens into Strategic.",
    },
    LayerInfo {
        name: "Strategic",
        meaning: "Principles and long-term direction.",
        promotes: "— (top of the ladder)",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_six_layers_in_ladder_order() {
        assert_eq!(LAYER_CATALOG.len(), 6);
        assert_eq!(LAYER_CATALOG[0].name, "Working");
        assert_eq!(LAYER_CATALOG[1].name, "Episodic");
        assert_eq!(LAYER_CATALOG[2].name, "Semantic");
        assert_eq!(LAYER_CATALOG[3].name, "Procedural");
        assert_eq!(LAYER_CATALOG[4].name, "Decision");
        assert_eq!(LAYER_CATALOG[5].name, "Strategic");
    }

    #[test]
    fn catalog_names_match_memory_layer() {
        use crate::core::memory::types::MemoryLayer;
        for info in LAYER_CATALOG {
            assert_eq!(
                MemoryLayer::parse(info.name).as_str(),
                info.name,
                "catalog name {} must parse back to itself",
                info.name
            );
        }
    }

    #[test]
    fn catalog_entries_have_meaning() {
        for info in LAYER_CATALOG {
            assert!(!info.meaning.is_empty());
            assert!(!info.promotes.is_empty());
        }
    }
}
