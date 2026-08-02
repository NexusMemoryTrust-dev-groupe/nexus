use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Request context carrying security metadata about the current operation.
/// Thread-safe, cloneable, and serializable.
/// Used for audit logging and security checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestContext {
    pub user_id: String,
    pub session_id: String,
    pub device_id: String,
    pub correlation_id: String,
    pub timestamp: DateTime<Utc>,
}

impl RequestContext {
    /// Create a new RequestContext with auto-generated correlation ID and timestamp.
    pub fn new(user_id: String, session_id: String, device_id: String) -> Self {
        Self {
            user_id,
            session_id,
            device_id,
            correlation_id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_context_new() {
        let ctx = RequestContext::new(
            "user-1".to_string(),
            "session-1".to_string(),
            "device-1".to_string(),
        );

        assert_eq!(ctx.user_id, "user-1");
        assert_eq!(ctx.session_id, "session-1");
        assert_eq!(ctx.device_id, "device-1");
        assert!(!ctx.correlation_id.is_empty());
        assert!(uuid::Uuid::parse_str(&ctx.correlation_id).is_ok());
    }

    #[test]
    fn request_context_timestamp_is_recent() {
        let ctx = RequestContext::new("u".to_string(), "s".to_string(), "d".to_string());
        let now = Utc::now();
        let diff = (now - ctx.timestamp).num_milliseconds();
        assert!(diff < 1000);
    }

    #[test]
    fn request_context_serialization() {
        let ctx = RequestContext::new("u1".to_string(), "s1".to_string(), "d1".to_string());
        let json = serde_json::to_string(&ctx).unwrap();
        let decoded: RequestContext = serde_json::from_str(&json).unwrap();
        assert_eq!(ctx.user_id, decoded.user_id);
        assert_eq!(ctx.correlation_id, decoded.correlation_id);
    }

    #[test]
    fn request_context_correlation_ids_are_unique() {
        let c1 = RequestContext::new("u".to_string(), "s".to_string(), "d".to_string());
        let c2 = RequestContext::new("u".to_string(), "s".to_string(), "d".to_string());
        assert_ne!(c1.correlation_id, c2.correlation_id);
    }

    #[test]
    fn request_context_clone() {
        let ctx = RequestContext::new("u".to_string(), "s".to_string(), "d".to_string());
        let cloned = ctx.clone();
        assert_eq!(ctx.user_id, cloned.user_id);
        assert_eq!(ctx.correlation_id, cloned.correlation_id);
    }
}
