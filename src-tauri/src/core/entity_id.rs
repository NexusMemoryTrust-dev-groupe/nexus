use serde::{Deserialize, Serialize};
use std::fmt;

use crate::core::result::{AppError, Result};

/// Unique identifier for any domain entity.
/// Wraps a UUID v4 string with validation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(String);

impl EntityId {
    /// Generate a new random EntityId.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Parse and validate an EntityId from a string.
    pub fn parse(s: &str) -> Result<Self> {
        uuid::Uuid::parse_str(s)
            .map_err(|e| AppError::Validation(e.to_string()))?;
        Ok(Self(s.to_string()))
    }

    /// Get the inner string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for EntityId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_id_new_is_valid_uuid() {
        let id = EntityId::new();
        assert!(uuid::Uuid::parse_str(&id.0).is_ok());
    }

    #[test]
    fn entity_id_parse_valid() {
        let uuid = uuid::Uuid::new_v4().to_string();
        let id = EntityId::parse(&uuid);
        assert!(id.is_ok());
        assert_eq!(id.unwrap().0, uuid);
    }

    #[test]
    fn entity_id_parse_invalid() {
        let id = EntityId::parse("not-a-uuid");
        assert!(id.is_err());
        match id.unwrap_err() {
            AppError::Validation(_) => {}
            _ => panic!("Expected Validation error"),
        }
    }

    #[test]
    fn entity_id_display() {
        let id = EntityId::new();
        assert_eq!(id.to_string(), id.0);
    }

    #[test]
    fn entity_id_as_str() {
        let id = EntityId::new();
        assert_eq!(id.as_str(), id.0);
    }

    #[test]
    fn entity_id_default() {
        let id = EntityId::default();
        assert!(uuid::Uuid::parse_str(&id.0).is_ok());
    }

    #[test]
    fn entity_id_serialization() {
        let id = EntityId::new();
        let json = serde_json::to_string(&id).unwrap();
        let decoded: EntityId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, decoded);
    }

    #[test]
    fn entity_id_equality() {
        let uuid = uuid::Uuid::new_v4().to_string();
        let id1 = EntityId::parse(&uuid).unwrap();
        let id2 = EntityId::parse(&uuid).unwrap();
        assert_eq!(id1, id2);
    }

    #[test]
    fn entity_id_hash() {
        let uuid = uuid::Uuid::new_v4().to_string();
        let id1 = EntityId::parse(&uuid).unwrap();
        let id2 = EntityId::parse(&uuid).unwrap();
        let mut set = std::collections::HashSet::new();
        set.insert(id1);
        assert!(set.contains(&id2));
    }
}
