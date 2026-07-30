use serde::{Deserialize, Serialize};

/// Source of a memory record — where the information came from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemorySource {
    Manual,
    Git,
    Telegram,
    Email,
    Meeting,
    Document,
    AiGenerated,
    Compressed,
}

/// Visibility level — controls access to a memory record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryVisibility {
    Public,
    Private,
    Restricted,
}

/// How the memory was captured.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryCaptureMode {
    Passive,
    Assisted,
    Automatic,
}

/// Memory layer — represents the level of processing/abstraction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryLayer {
    Raw,
    Knowledge,
    Decision,
    Wisdom,
}

/// Current status of a memory record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryStatus {
    Active,
    Archived,
    Merged,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_source_serialization() {
        let sources = vec![
            MemorySource::Manual,
            MemorySource::Git,
            MemorySource::Telegram,
            MemorySource::Email,
            MemorySource::Meeting,
            MemorySource::Document,
            MemorySource::AiGenerated,
            MemorySource::Compressed,
        ];
        for source in sources {
            let json = serde_json::to_string(&source).unwrap();
            let decoded: MemorySource = serde_json::from_str(&json).unwrap();
            assert_eq!(source, decoded);
        }
    }

    #[test]
    fn memory_visibility_serialization() {
        let vis = MemoryVisibility::Private;
        let json = serde_json::to_string(&vis).unwrap();
        let decoded: MemoryVisibility = serde_json::from_str(&json).unwrap();
        assert_eq!(vis, decoded);
    }

    #[test]
    fn memory_layer_serialization() {
        let layer = MemoryLayer::Knowledge;
        let json = serde_json::to_string(&layer).unwrap();
        let decoded: MemoryLayer = serde_json::from_str(&json).unwrap();
        assert_eq!(layer, decoded);
    }

    #[test]
    fn memory_status_serialization() {
        let status = MemoryStatus::Active;
        let json = serde_json::to_string(&status).unwrap();
        let decoded: MemoryStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, decoded);
    }

    #[test]
    fn memory_capture_mode_serialization() {
        let mode = MemoryCaptureMode::Assisted;
        let json = serde_json::to_string(&mode).unwrap();
        let decoded: MemoryCaptureMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, decoded);
    }
}
