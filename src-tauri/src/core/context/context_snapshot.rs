use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::context::context_package::ContextPackage;
use crate::core::entity_id::EntityId;

/// A persisted context snapshot — saves a context package for later restoration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub id: String,
    pub entity_id: EntityId,
    pub package: ContextPackage,
    pub created_at: DateTime<Utc>,
    pub label: Option<String>,
}

impl ContextSnapshot {
    pub fn new(entity_id: EntityId, package: ContextPackage, label: Option<String>) -> Self {
        Self {
            id: crate::core::EntityId::new().as_str().to_string(),
            entity_id,
            package,
            created_at: Utc::now(),
            label,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::context_package::UserIntent;

    #[test]
    fn new_snapshot() {
        let pkg = ContextPackage::new(UserIntent {
            query: "test".to_string(),
            intent_type: crate::core::context::context_package::IntentType::Search,
            confidence: 0.8,
            keywords: vec!["test".to_string()],
            temporal: None,
        });
        let snap = ContextSnapshot::new(EntityId::new(), pkg.clone(), Some("test label".to_string()));
        assert!(!snap.id.is_empty());
        assert_eq!(snap.label, Some("test label".to_string()));
        assert_eq!(snap.package.id, pkg.id);
    }

    #[test]
    fn snapshot_serialization() {
        let pkg = ContextPackage::new(UserIntent {
            query: "q".to_string(),
            intent_type: crate::core::context::context_package::IntentType::Exploration,
            confidence: 0.5,
            keywords: vec![],
            temporal: None,
        });
        let snap = ContextSnapshot::new(EntityId::new(), pkg, None);
        let json = serde_json::to_string(&snap).unwrap();
        let decoded: ContextSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snap.id, decoded.id);
    }
}
