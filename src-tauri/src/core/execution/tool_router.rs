use std::collections::HashMap;
use std::sync::Arc;

use crate::core::execution::tool::Tool;
use crate::core::result::{AppError, Result};

/// Routes action names to concrete Tool implementations.
pub trait ToolRouter: Send + Sync {
    /// Return the tool registered for the given action name.
    fn route(&self, action: &str) -> Result<Arc<dyn Tool>>;

    /// List every registered tool.
    fn available_tools(&self) -> Vec<Arc<dyn Tool>>;
}

/// Default router backed by a name→tool map.
pub struct DefaultToolRouter {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl Default for DefaultToolRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultToolRouter {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool. Overwrites any previous tool with the same name.
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }
}

impl ToolRouter for DefaultToolRouter {
    fn route(&self, action: &str) -> Result<Arc<dyn Tool>> {
        self.tools
            .get(action)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Tool not found: {}", action)))
    }

    fn available_tools(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.values().cloned().collect()
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct StubTool {
        tool_name: String,
    }

    #[async_trait]
    impl Tool for StubTool {
        fn name(&self) -> &str {
            &self.tool_name
        }
        fn description(&self) -> &str {
            "stub"
        }
        async fn execute(
            &self,
            _params: &serde_json::Value,
            _sandbox: &crate::core::execution::sandbox::Sandbox,
        ) -> Result<serde_json::Value> {
            Ok(serde_json::json!({ "tool": self.tool_name }))
        }
        fn validate_params(&self, _params: &serde_json::Value) -> Result<()> {
            Ok(())
        }
    }

    fn make_tool(name: &str) -> Arc<dyn Tool> {
        Arc::new(StubTool {
            tool_name: name.to_string(),
        })
    }

    #[test]
    fn route_existing_tool() {
        let mut router = DefaultToolRouter::new();
        router.register(make_tool("file"));
        assert!(router.route("file").is_ok());
    }

    #[test]
    fn route_missing_tool() {
        let router = DefaultToolRouter::new();
        assert!(router.route("nonexistent").is_err());
    }

    #[test]
    fn available_tools_count() {
        let mut router = DefaultToolRouter::new();
        router.register(make_tool("a"));
        router.register(make_tool("b"));
        assert_eq!(router.available_tools().len(), 2);
    }

    #[test]
    fn register_overwrites() {
        let mut router = DefaultToolRouter::new();
        router.register(make_tool("file"));
        router.register(make_tool("file"));
        assert_eq!(router.available_tools().len(), 1);
    }
}
