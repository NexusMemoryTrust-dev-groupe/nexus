//! RequestContext — security metadata carried by every critical command
//! (plan 4.4).
//!
//! The context answers three questions before any privileged operation:
//! *who* is acting (actor / agent / user), *where* (project scope), and *what
//! they are allowed to see* (permissions + sensitivity scope). Commands that
//! mutate or expose memory MUST receive a `RequestContext` and consult it via
//! [`RequestContext::can_access`] / [`RequestContext::allows_sensitivity`]
//! before proceeding — this is what makes audit logs meaningful and keeps
//! one agent from reading another agent's restricted memories.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::memory::agent_permissions::Sensitivity;

/// Who is performing the operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Actor {
    /// A human user (the app owner).
    User,
    /// An AI agent acting through the MCP surface.
    Agent,
    /// Internal maintenance (indexer, rehearsal, backup, doctor).
    System,
}

impl Actor {
    pub fn as_str(&self) -> &'static str {
        match self {
            Actor::User => "user",
            Actor::Agent => "agent",
            Actor::System => "system",
        }
    }
}

/// Request context carrying security metadata about the current operation.
/// Thread-safe, cloneable, and serializable.
/// Used for audit logging and security checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestContext {
    pub user_id: String,
    pub session_id: String,
    pub device_id: String,
    /// Unique id of this logical request — matches the `request_id` field of
    /// structured logs (plan 4.4) so an audit event can be correlated with the
    /// log line that produced it.
    pub request_id: String,
    pub actor: Actor,
    /// Id of the agent making the request (when `actor == Agent`).
    pub agent_id: Option<String>,
    /// Project the request is scoped to; `None` means global scope.
    pub project_id: Option<String>,
    /// Memory categories the actor is allowed to read
    /// (e.g. `["architecture", "code", "decisions"]` from
    /// `agent_permissions::CATEGORY_*`). An empty list denies everything.
    pub permissions: Vec<String>,
    /// Maximum memory sensitivity the actor may read (inclusive). A scope of
    /// `Sensitivity::Project` allows `Public` and `Project` but denies
    /// `Restricted` and `Private`.
    pub sensitivity_scope: Sensitivity,
    pub timestamp: DateTime<Utc>,
}

impl RequestContext {
    /// Create a new RequestContext with auto-generated request ID, timestamp,
    /// and the most restrictive defaults: no permissions, no sensitivity.
    /// Callers must grant access explicitly via the builder methods.
    pub fn new(user_id: String, session_id: String, device_id: String) -> Self {
        Self {
            user_id,
            session_id,
            device_id,
            request_id: uuid::Uuid::new_v4().to_string(),
            actor: Actor::User,
            agent_id: None,
            project_id: None,
            permissions: Vec::new(),
            sensitivity_scope: Sensitivity::Public,
            timestamp: Utc::now(),
        }
    }

    /// Convenience context for a human acting through the local UI: a `User`
    /// actor with a fresh request id. Mutations are allowed (`can_mutate`),
    /// data access still requires explicit permission grants.
    pub fn user() -> Self {
        Self::new("user".to_string(), "ui".to_string(), "desktop".to_string())
    }

    /// Convenience context for an agent acting through the MCP surface: an
    /// `Agent` actor with a fresh request id. Identity alone grants nothing —
    /// read access needs permission categories, mutation needs the `"write"`
    /// permission (plan 4.6: identity ≠ authorization).
    pub fn agent(agent_id: &str) -> Self {
        Self::new("agent".to_string(), "mcp".to_string(), "mcp".to_string()).with_agent(agent_id)
    }

    /// Grant a list of memory categories (plan 4.4).
    pub fn with_permissions(mut self, categories: &[&str]) -> Self {
        self.permissions = categories.iter().map(|c| c.to_string()).collect();
        self
    }

    /// Raise the sensitivity scope the actor may read (plan 4.4).
    pub fn with_sensitivity_scope(mut self, scope: Sensitivity) -> Self {
        self.sensitivity_scope = scope;
        self
    }

    /// Mark the request as coming from an agent (plan 4.6: identity ≠
    /// authorization — the agent id is *not* a permission grant).
    pub fn with_agent(mut self, agent_id: &str) -> Self {
        self.actor = Actor::Agent;
        self.agent_id = Some(agent_id.to_string());
        self
    }

    /// Scope the request to a project (plan 4.4).
    pub fn for_project(mut self, project_id: &str) -> Self {
        self.project_id = Some(project_id.to_string());
        self
    }

    /// Whether the actor may read the given memory category.
    pub fn can_access(&self, category: &str) -> bool {
        self.permissions.iter().any(|p| p == category)
    }

    /// Whether the actor may read memories of the given sensitivity.
    pub fn allows_sensitivity(&self, sensitivity: Sensitivity) -> bool {
        sensitivity <= self.sensitivity_scope
    }

    /// Whether this context is allowed to perform a *mutation* — only humans
    /// and system maintenance may mutate; agents are read-only unless granted
    /// the `"write"` permission explicitly.
    pub fn can_mutate(&self) -> bool {
        self.actor == Actor::User
            || self.actor == Actor::System
            || self.permissions.iter().any(|p| p == "write")
    }

    /// Human-readable actor label for audit/detail fields: `"user"`,
    /// `"system"`, or `"agent:<id>"` (plan 4.4 — audit records who acted).
    pub fn actor_label(&self) -> String {
        match (&self.actor, &self.agent_id) {
            (Actor::Agent, Some(id)) => format!("agent:{}", id),
            _ => self.actor.as_str().to_string(),
        }
    }

    /// The gate critical commands MUST pass before mutating (plan 4.4):
    /// returns the operation error when the actor lacks write permission.
    pub fn ensure_can_mutate(&self) -> Result<(), String> {
        if self.can_mutate() {
            Ok(())
        } else {
            Err(format!(
                "Forbidden: actor '{}' has no write permission",
                self.actor_label()
            ))
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
        assert!(!ctx.request_id.is_empty());
        assert!(uuid::Uuid::parse_str(&ctx.request_id).is_ok());
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
        assert_eq!(ctx.request_id, decoded.request_id);
    }

    #[test]
    fn request_context_request_ids_are_unique() {
        let c1 = RequestContext::new("u".to_string(), "s".to_string(), "d".to_string());
        let c2 = RequestContext::new("u".to_string(), "s".to_string(), "d".to_string());
        assert_ne!(c1.request_id, c2.request_id);
    }

    #[test]
    fn request_context_clone() {
        let ctx = RequestContext::new("u".to_string(), "s".to_string(), "d".to_string());
        let cloned = ctx.clone();
        assert_eq!(ctx.user_id, cloned.user_id);
        assert_eq!(ctx.request_id, cloned.request_id);
    }

    #[test]
    fn default_context_denies_data_access() {
        // Deny-by-default for *data*: no permission categories and only the
        // Public sensitivity level. (Mutation is a separate concern — see
        // `can_mutate`.)
        let ctx = RequestContext::new("u".to_string(), "s".to_string(), "d".to_string());
        assert!(!ctx.can_access("architecture"), "no permissions by default");
        assert!(
            !ctx.allows_sensitivity(Sensitivity::Restricted),
            "default scope is Public, Restricted must be denied"
        );
    }

    #[test]
    fn permissions_grant_access() {
        let ctx = RequestContext::new("u".to_string(), "s".to_string(), "d".to_string())
            .with_permissions(&["architecture", "code"])
            .with_sensitivity_scope(Sensitivity::Restricted);
        assert!(ctx.can_access("architecture"));
        assert!(ctx.can_access("code"));
        assert!(!ctx.can_access("secrets"));
        assert!(ctx.allows_sensitivity(Sensitivity::Project));
        assert!(ctx.allows_sensitivity(Sensitivity::Restricted));
        assert!(!ctx.allows_sensitivity(Sensitivity::Private));
    }

    #[test]
    fn agent_identity_is_not_authorization() {
        // An agent with an identity has NO permissions until granted — the
        // whole point of plan 4.6.
        let ctx = RequestContext::new("u".to_string(), "s".to_string(), "d".to_string())
            .with_agent("claude-code");
        assert_eq!(ctx.actor, Actor::Agent);
        assert_eq!(ctx.agent_id.as_deref(), Some("claude-code"));
        assert!(!ctx.can_access("secrets"), "identity alone grants nothing");
        assert!(!ctx.can_mutate(), "agents cannot mutate by default");
        // Granting "write" explicitly unlocks mutation.
        let writer = ctx.clone().with_permissions(&["write"]);
        assert!(writer.can_mutate());
    }

    #[test]
    fn system_and_user_can_mutate() {
        let user = RequestContext::new("u".to_string(), "s".to_string(), "d".to_string());
        // A user context with permissions is fully trusted for writes.
        assert!(user.clone().with_permissions(&["write"]).can_mutate());
    }

    #[test]
    fn actor_labels_are_readable() {
        assert_eq!(RequestContext::user().actor_label(), "user");
        assert_eq!(
            RequestContext::agent("claude-code").actor_label(),
            "agent:claude-code"
        );
        let system = RequestContext::new("s".to_string(), "x".to_string(), "x".to_string());
        let mut system = system;
        system.actor = Actor::System;
        assert_eq!(system.actor_label(), "system");
    }

    #[test]
    fn ensure_can_mutate_gate() {
        // Agent identity alone must be rejected — the gate is what makes the
        // permission explicit (plan 4.4: RequestContext mandatory, identity
        // is not authorization).
        let agent = RequestContext::agent("claude-code");
        assert!(agent.ensure_can_mutate().is_err(), "plain agent denied");
        let writer = agent.clone().with_permissions(&["write"]);
        assert!(writer.ensure_can_mutate().is_ok(), "writer allowed");
        assert!(RequestContext::user().ensure_can_mutate().is_ok());
        let err = agent.ensure_can_mutate().unwrap_err();
        assert!(err.contains("no write permission"), "clear error: {}", err);
        assert!(err.contains("agent:claude-code"), "actor in error: {}", err);
    }

    #[test]
    fn project_scoping() {
        let ctx = RequestContext::new("u".to_string(), "s".to_string(), "d".to_string())
            .for_project("proj-42");
        assert_eq!(ctx.project_id.as_deref(), Some("proj-42"));
        let global = RequestContext::new("u".to_string(), "s".to_string(), "d".to_string());
        assert_eq!(global.project_id, None);
    }

    #[test]
    fn actor_codes_are_stable() {
        assert_eq!(Actor::User.as_str(), "user");
        assert_eq!(Actor::Agent.as_str(), "agent");
        assert_eq!(Actor::System.as_str(), "system");
    }
}
