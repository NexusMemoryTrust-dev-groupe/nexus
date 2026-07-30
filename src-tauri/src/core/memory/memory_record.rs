use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity_id::EntityId;
use crate::core::memory::types::{MemoryCaptureMode, MemoryLayer, MemorySource, MemoryStatus, MemoryVisibility};
use crate::core::result::{AppError, Result};

/// A single memory record — the atomic unit of the Nexus memory system.
/// Immutable history: any modification creates a version (V1→V2→V3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: EntityId,
    pub title: String,
    pub summary: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub author: String,
    pub source: MemorySource,
    pub confidence_score: f64,   // 0.0 (rumor) — 1.0 (official decision)
    pub importance_score: f64,   // 0.0 (meeting note) — 1.0 (architecture decision)
    pub visibility: MemoryVisibility,
    pub capture_mode: MemoryCaptureMode,
    pub project_space_id: Option<EntityId>,
    pub linked_entity_ids: Vec<EntityId>,
    pub latest_version_id: Option<String>,
    pub status: MemoryStatus,
    pub layer: MemoryLayer,
    pub attached_files: Vec<AttachedFile>,
    // Memory Trust fields
    pub derived_from: Vec<String>,  // Sources this memory was derived from
    pub reason: Option<String>,     // Why this memory exists
    pub version: u32,               // Version number (starts at 1)
    pub updated_by: Option<String>, // Who last updated this memory
}

/// A file attached to a memory record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AttachedFile {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub mime_type: String,
}

impl MemoryRecord {
    /// Create a new MemoryRecord with sensible defaults.
    /// Validates title and content are non-empty.
    pub fn new(
        title: String,
        content: String,
        author: String,
        source: MemorySource,
    ) -> Result<Self> {
        if title.trim().is_empty() {
            return Err(AppError::Validation(
                "Title cannot be empty".to_string(),
            ));
        }
        if content.trim().is_empty() {
            return Err(AppError::Validation(
                "Content cannot be empty".to_string(),
            ));
        }

        let now = Utc::now();
        Ok(Self {
            id: EntityId::new(),
            title,
            summary: String::new(),
            content,
            created_at: now,
            updated_at: now,
            author,
            source,
            confidence_score: 0.5,
            importance_score: 0.5,
            visibility: MemoryVisibility::Private,
            capture_mode: MemoryCaptureMode::Passive,
            project_space_id: None,
            linked_entity_ids: Vec::new(),
            latest_version_id: None,
            status: MemoryStatus::Active,
            layer: MemoryLayer::Raw,
            attached_files: Vec::new(),
            // Memory Trust defaults
            derived_from: Vec::new(),
            reason: None,
            version: 1,
            updated_by: None,
        })
    }

    /// Validate all invariants: scores in range, title/content non-empty.
    pub fn validate(&self) -> Result<()> {
        if self.title.trim().is_empty() {
            return Err(AppError::Validation(
                "Title cannot be empty".to_string(),
            ));
        }
        if self.content.trim().is_empty() {
            return Err(AppError::Validation(
                "Content cannot be empty".to_string(),
            ));
        }
        if !(0.0..=1.0).contains(&self.confidence_score) {
            return Err(AppError::Validation(format!(
                "Confidence score must be 0.0–1.0, got {}",
                self.confidence_score
            )));
        }
        if !(0.0..=1.0).contains(&self.importance_score) {
            return Err(AppError::Validation(format!(
                "Importance score must be 0.0–1.0, got {}",
                self.importance_score
            )));
        }
        Ok(())
    }

    /// Touch updated_at timestamp and increment version.
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
        self.version += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record() -> MemoryRecord {
        MemoryRecord::new(
            "Test Title".to_string(),
            "Test content body".to_string(),
            "author-1".to_string(),
            MemorySource::Manual,
        )
        .unwrap()
    }

    #[test]
    fn new_record_has_valid_uuid() {
        let r = sample_record();
        assert!(!r.id.as_str().is_empty());
    }

    #[test]
    fn new_record_defaults() {
        let r = sample_record();
        assert_eq!(r.summary, "");
        assert_eq!(r.confidence_score, 0.5);
        assert_eq!(r.importance_score, 0.5);
        assert_eq!(r.visibility, MemoryVisibility::Private);
        assert_eq!(r.capture_mode, MemoryCaptureMode::Passive);
        assert!(r.project_space_id.is_none());
        assert!(r.linked_entity_ids.is_empty());
        assert!(r.latest_version_id.is_none());
        assert_eq!(r.status, MemoryStatus::Active);
        assert_eq!(r.layer, MemoryLayer::Raw);
        assert!(r.attached_files.is_empty());
        // Memory Trust defaults
        assert!(r.derived_from.is_empty());
        assert!(r.reason.is_none());
        assert_eq!(r.version, 1);
        assert!(r.updated_by.is_none());
    }

    #[test]
    fn new_record_timestamps_are_close() {
        let before = Utc::now();
        let r = sample_record();
        let after = Utc::now();
        assert!(r.created_at >= before && r.created_at <= after);
        assert!(r.updated_at >= before && r.updated_at <= after);
    }

    #[test]
    fn new_record_empty_title_fails() {
        let result = MemoryRecord::new(
            "  ".to_string(),
            "content".to_string(),
            "author".to_string(),
            MemorySource::Manual,
        );
        assert!(result.is_err());
    }

    #[test]
    fn new_record_empty_content_fails() {
        let result = MemoryRecord::new(
            "title".to_string(),
            "   ".to_string(),
            "author".to_string(),
            MemorySource::Manual,
        );
        assert!(result.is_err());
    }

    #[test]
    fn validate_scores_in_range() {
        let mut r = sample_record();
        r.confidence_score = 0.0;
        r.importance_score = 1.0;
        assert!(r.validate().is_ok());
    }

    #[test]
    fn validate_confidence_too_high() {
        let mut r = sample_record();
        r.confidence_score = 1.1;
        assert!(r.validate().is_err());
    }

    #[test]
    fn validate_importance_negative() {
        let mut r = sample_record();
        r.importance_score = -0.1;
        assert!(r.validate().is_err());
    }

    #[test]
    fn touch_updates_timestamp() {
        let mut r = sample_record();
        let original = r.updated_at;
        let original_version = r.version;
        // Small delay to ensure timestamp changes
        std::thread::sleep(std::time::Duration::from_millis(10));
        r.touch();
        assert!(r.updated_at >= original);
        assert_eq!(r.version, original_version + 1);
    }

    #[test]
    fn record_clone() {
        let r = sample_record();
        let cloned = r.clone();
        assert_eq!(r.id, cloned.id);
        assert_eq!(r.title, cloned.title);
        assert_eq!(r.content, cloned.content);
    }

    #[test]
    fn record_serialization() {
        let r = sample_record();
        let json = serde_json::to_string(&r).unwrap();
        let decoded: MemoryRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(r.id, decoded.id);
        assert_eq!(r.title, decoded.title);
        assert_eq!(r.source, decoded.source);
    }
}
