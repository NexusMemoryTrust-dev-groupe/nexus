use serde::{Deserialize, Serialize};

use crate::core::entity_id::EntityId;

/// Parameters for building a context package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRequest {
    pub query: String,
    pub project_id: Option<EntityId>,
    pub max_tokens: u32,
    pub max_entities: u32,
    pub max_depth: u32,
    pub min_relevance: f64,
}

impl Default for ContextRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            project_id: None,
            max_tokens: 4000,
            max_entities: 100,
            max_depth: 2,
            min_relevance: 0.3,
        }
    }
}

impl ContextRequest {
    /// Validate request parameters.
    pub fn validate(&self) -> crate::core::Result<()> {
        if self.max_tokens == 0 {
            return Err(crate::core::AppError::Validation(
                "max_tokens must be > 0".into(),
            ));
        }
        if self.max_entities == 0 {
            return Err(crate::core::AppError::Validation(
                "max_entities must be > 0".into(),
            ));
        }
        if self.max_depth == 0 {
            return Err(crate::core::AppError::Validation(
                "max_depth must be > 0".into(),
            ));
        }
        if self.min_relevance < 0.0 || self.min_relevance > 1.0 {
            return Err(crate::core::AppError::Validation(
                "min_relevance must be between 0.0 and 1.0".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_request() {
        let req = ContextRequest::default();
        assert!(req.query.is_empty());
        assert!(req.project_id.is_none());
        assert_eq!(req.max_tokens, 4000);
        assert_eq!(req.max_entities, 100);
        assert_eq!(req.max_depth, 2);
        assert_eq!(req.min_relevance, 0.3);
    }

    #[test]
    fn validate_valid() {
        assert!(ContextRequest::default().validate().is_ok());
    }

    #[test]
    fn validate_zero_tokens() {
        let req = ContextRequest {
            max_tokens: 0,
            ..Default::default()
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn validate_zero_entities() {
        let req = ContextRequest {
            max_entities: 0,
            ..Default::default()
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn validate_zero_depth() {
        let req = ContextRequest {
            max_depth: 0,
            ..Default::default()
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn validate_min_relevance_out_of_range() {
        let req = ContextRequest {
            min_relevance: 1.5,
            ..Default::default()
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn serialization_roundtrip() {
        let req = ContextRequest::default();
        let json = serde_json::to_string(&req).unwrap();
        let decoded: ContextRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.max_tokens, decoded.max_tokens);
    }
}
