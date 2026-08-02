use std::sync::Arc;

use crate::core::execution::sandbox::Sandbox;
use crate::core::execution::tool_router::ToolRouter;
use crate::core::execution::types::{ExecutionState, ExecutionStatus, Plan, StepResult};
use crate::core::result::Result;

/// Executes steps by routing them to the appropriate tool.
#[async_trait::async_trait]
pub trait ActionExecutor: Send + Sync {
    /// Execute a single step and return its result.
    async fn execute_step(
        &self,
        step_id: &str,
        state: &mut ExecutionState,
        sandbox: &Sandbox,
    ) -> Result<StepResult>;

    /// Execute every step in a plan, returning the final ExecutionState.
    async fn execute_plan(&self, plan: &Plan, sandbox: &Sandbox) -> Result<ExecutionState>;
}

/// Default executor that delegates to the ToolRouter.
pub struct DefaultActionExecutor {
    router: Arc<dyn ToolRouter>,
}

impl DefaultActionExecutor {
    pub fn new(router: Arc<dyn ToolRouter>) -> Self {
        Self { router }
    }
}

#[async_trait::async_trait]
impl ActionExecutor for DefaultActionExecutor {
    async fn execute_step(
        &self,
        step_id: &str,
        state: &mut ExecutionState,
        sandbox: &Sandbox,
    ) -> Result<StepResult> {
        let step = state
            .plan
            .steps
            .iter()
            .find(|s| s.id == step_id)
            .ok_or_else(|| {
                crate::core::AppError::NotFound(format!("Step not found: {}", step_id))
            })?;

        let tool = self.router.route(&step.action)?;

        match tool.execute(&step.params, sandbox).await {
            Ok(output) => Ok(StepResult {
                step_id: step.id.clone(),
                success: true,
                output,
                error: None,
            }),
            Err(e) => Ok(StepResult {
                step_id: step.id.clone(),
                success: false,
                output: serde_json::json!(null),
                error: Some(e.to_string()),
            }),
        }
    }

    async fn execute_plan(&self, plan: &Plan, sandbox: &Sandbox) -> Result<ExecutionState> {
        let mut state = ExecutionState {
            plan: plan.clone(),
            current_step: 0,
            status: ExecutionStatus::Running,
            results: vec![],
        };

        for step in &plan.steps {
            let result = self.execute_step(&step.id, &mut state, sandbox).await?;
            state.results.push(result);
            state.current_step += 1;

            if !state.results.last().unwrap().success {
                state.status = ExecutionStatus::Failed;
                break;
            }
        }

        if state.status == ExecutionStatus::Running {
            state.status = ExecutionStatus::Completed;
        }

        Ok(state)
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::execution::tool::Tool;
    use crate::core::execution::tool_router::DefaultToolRouter;
    use crate::core::execution::types::{Step, StepStatus};
    use async_trait::async_trait;
    use std::sync::Arc;

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
            _sandbox: &Sandbox,
        ) -> Result<serde_json::Value> {
            Ok(serde_json::json!({ "ok": true }))
        }
        fn validate_params(&self, _params: &serde_json::Value) -> Result<()> {
            Ok(())
        }
    }

    struct FailingTool;

    #[async_trait]
    impl Tool for FailingTool {
        fn name(&self) -> &str {
            "fail"
        }
        fn description(&self) -> &str {
            "always fails"
        }
        async fn execute(
            &self,
            _params: &serde_json::Value,
            _sandbox: &Sandbox,
        ) -> Result<serde_json::Value> {
            Err(crate::core::AppError::Internal("boom".to_string()))
        }
        fn validate_params(&self, _params: &serde_json::Value) -> Result<()> {
            Ok(())
        }
    }

    fn make_executor(tools: Vec<Arc<dyn Tool>>) -> DefaultActionExecutor {
        let mut router = DefaultToolRouter::new();
        for t in tools {
            router.register(t);
        }
        DefaultActionExecutor::new(Arc::new(router))
    }

    fn make_step(id: &str, action: &str) -> Step {
        Step {
            id: id.to_string(),
            action: action.to_string(),
            target: String::new(),
            params: serde_json::json!({}),
            status: StepStatus::Pending,
        }
    }

    #[tokio::test]
    async fn execute_plan_success() {
        let executor = make_executor(vec![Arc::new(StubTool {
            tool_name: "run".to_string(),
        })]);
        let plan = Plan {
            id: "p1".to_string(),
            steps: vec![make_step("s1", "run"), make_step("s2", "run")],
            created_at: chrono::Utc::now(),
        };
        let state = executor.execute_plan(&plan, &Sandbox::new()).await.unwrap();
        assert_eq!(state.status, ExecutionStatus::Completed);
        assert_eq!(state.results.len(), 2);
        assert!(state.results.iter().all(|r| r.success));
    }

    #[tokio::test]
    async fn execute_plan_stops_on_failure() {
        let executor = make_executor(vec![
            Arc::new(StubTool {
                tool_name: "ok".to_string(),
            }),
            Arc::new(FailingTool),
        ]);
        let plan = Plan {
            id: "p1".to_string(),
            steps: vec![make_step("s1", "ok"), make_step("s2", "fail")],
            created_at: chrono::Utc::now(),
        };
        let state = executor.execute_plan(&plan, &Sandbox::new()).await.unwrap();
        assert_eq!(state.status, ExecutionStatus::Failed);
        assert_eq!(state.results.len(), 2);
        assert!(!state.results[1].success);
    }

    #[tokio::test]
    async fn execute_single_step() {
        let executor = make_executor(vec![Arc::new(StubTool {
            tool_name: "echo".to_string(),
        })]);
        let mut state = ExecutionState {
            plan: Plan {
                id: "p1".to_string(),
                steps: vec![make_step("s1", "echo")],
                created_at: chrono::Utc::now(),
            },
            current_step: 0,
            status: ExecutionStatus::Running,
            results: vec![],
        };
        let result = executor
            .execute_step("s1", &mut state, &Sandbox::new())
            .await
            .unwrap();
        assert!(result.success);
    }
}
