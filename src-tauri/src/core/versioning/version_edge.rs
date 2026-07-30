use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Relationship between two versions in the version graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VersionEdgeType {
    EvolvedTo,
    BranchedTo,
    MergedWith,
}

/// Directed edge between two version commits in the version graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionEdge {
    pub id: String,
    pub from_version_id: String,
    pub to_version_id: String,
    pub relationship_type: VersionEdgeType,
    pub created_at: DateTime<Utc>,
}

impl VersionEdge {
    pub fn new(
        from_version_id: String,
        to_version_id: String,
        relationship_type: VersionEdgeType,
    ) -> Self {
        Self {
            id: crate::core::EntityId::new().as_str().to_string(),
            from_version_id,
            to_version_id,
            relationship_type,
            created_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_version_edge() {
        let edge = VersionEdge::new(
            "v1".to_string(),
            "v2".to_string(),
            VersionEdgeType::EvolvedTo,
        );
        assert!(!edge.id.is_empty());
        assert_eq!(edge.from_version_id, "v1");
        assert_eq!(edge.to_version_id, "v2");
        assert_eq!(edge.relationship_type, VersionEdgeType::EvolvedTo);
    }

    #[test]
    fn edge_type_serialization() {
        for et in [
            VersionEdgeType::EvolvedTo,
            VersionEdgeType::BranchedTo,
            VersionEdgeType::MergedWith,
        ] {
            let json = serde_json::to_string(&et).unwrap();
            let decoded: VersionEdgeType = serde_json::from_str(&json).unwrap();
            assert_eq!(et, decoded);
        }
    }

    #[test]
    fn edge_serialization_roundtrip() {
        let edge = VersionEdge::new(
            "v1".to_string(),
            "v2".to_string(),
            VersionEdgeType::BranchedTo,
        );
        let json = serde_json::to_string(&edge).unwrap();
        let decoded: VersionEdge = serde_json::from_str(&json).unwrap();
        assert_eq!(edge.id, decoded.id);
        assert_eq!(edge.relationship_type, decoded.relationship_type);
    }
}
