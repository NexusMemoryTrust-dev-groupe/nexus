use async_trait::async_trait;
use serde_json::Value;

use crate::core::execution::sandbox::Sandbox;
use crate::core::result::Result;

/// A tool that can be invoked by the ActionExecutor.
/// Each tool has a name, description, parameter validation, and async execution.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique identifier for this tool (e.g. "file", "git").
    fn name(&self) -> &str;

    /// Human-readable description of what this tool does.
    fn description(&self) -> &str;

    /// Execute the tool with the given parameters inside a sandbox.
    async fn execute(&self, params: &Value, sandbox: &Sandbox) -> Result<Value>;

    /// Validate parameters before execution. Returns Err if params are invalid.
    fn validate_params(&self, params: &Value) -> Result<()>;
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }
        fn description(&self) -> &str {
            "Echoes input back"
        }
        async fn execute(&self, params: &Value, _sandbox: &Sandbox) -> Result<Value> {
            Ok(params.clone())
        }
        fn validate_params(&self, _params: &Value) -> Result<()> {
            Ok(())
        }
    }

    struct StrictTool;

    #[async_trait]
    impl Tool for StrictTool {
        fn name(&self) -> &str {
            "strict"
        }
        fn description(&self) -> &str {
            "Requires 'value' param"
        }
        async fn execute(&self, params: &Value, _sandbox: &Sandbox) -> Result<Value> {
            Ok(params.clone())
        }
        fn validate_params(&self, params: &Value) -> Result<()> {
            if params.get("value").is_none() {
                return Err(crate::core::AppError::Validation(
                    "Missing 'value' parameter".to_string(),
                ));
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn echo_tool_executes() {
        let tool = EchoTool;
        let sandbox = Sandbox::new();
        let params = serde_json::json!({"msg": "hello"});
        let result = tool.execute(&params, &sandbox).await.unwrap();
        assert_eq!(result, params);
    }

    #[test]
    fn echo_tool_metadata() {
        let tool = EchoTool;
        assert_eq!(tool.name(), "echo");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn strict_tool_validate_ok() {
        let tool = StrictTool;
        let params = serde_json::json!({"value": 42});
        assert!(tool.validate_params(&params).is_ok());
    }

    #[test]
    fn strict_tool_validate_missing() {
        let tool = StrictTool;
        let params = serde_json::json!({});
        assert!(tool.validate_params(&params).is_err());
    }
}
