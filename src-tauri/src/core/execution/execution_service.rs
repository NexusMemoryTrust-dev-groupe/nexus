use std::sync::Arc;

use crate::core::execution::action_executor::{ActionExecutor, DefaultActionExecutor};
use crate::core::execution::planner::{Planner, SimplePlanner};
use crate::core::execution::sandbox::Sandbox;
use crate::core::execution::state_tracker::{ExecutionStateTracker, InMemoryStateTracker};
use crate::core::execution::tool_router::ToolRouter;
use crate::core::execution::types::{ExecutionState, ExecutionStatus, ExecutionVersion, Plan};
use crate::core::result::Result;

/// Orchestrates the full execution lifecycle: plan → execute → track → log.
pub struct ExecutionService {
    planner: Arc<dyn Planner>,
    executor: Arc<dyn ActionExecutor>,
    tracker: Arc<dyn ExecutionStateTracker>,
    sandbox: Sandbox,
}

impl ExecutionService {
    pub fn new(
        planner: Arc<dyn Planner>,
        executor: Arc<dyn ActionExecutor>,
        tracker: Arc<dyn ExecutionStateTracker>,
        sandbox: Sandbox,
    ) -> Self {
        Self {
            planner,
            executor,
            tracker,
            sandbox,
        }
    }

    /// Create a convenience builder with SimplePlanner + DefaultActionExecutor.
    pub fn build(router: Arc<dyn ToolRouter>, sandbox: Sandbox) -> Self {
        let planner = Arc::new(SimplePlanner::new());
        let executor = Arc::new(DefaultActionExecutor::new(router));

        // Minimal initial state — will be replaced on execute
        let initial = ExecutionState {
            plan: Plan {
                id: "init".to_string(),
                steps: vec![],
                created_at: chrono::Utc::now(),
            },
            current_step: 0,
            status: ExecutionStatus::Planning,
            results: vec![],
        };
        let tracker = Arc::new(InMemoryStateTracker::new(initial));

        Self::new(planner, executor, tracker, sandbox)
    }

    /// Full lifecycle: plan → execute → return final state.
    pub async fn execute(&self, intent: &str) -> Result<ExecutionState> {
        // 1. Plan
        let plan = self.planner.create_plan(intent).await?;
        self.tracker
            .log_event("plan_created", &format!("{} steps", plan.steps.len()))?;

        // 2. Execute
        let state = self.executor.execute_plan(&plan, &self.sandbox).await?;

        // 3. Log result
        let status_str = format!("{:?}", state.status);
        self.tracker
            .log_event("execution_finished", &status_str)?;

        Ok(state)
    }

    /// Create a versioned snapshot of the current state.
    pub fn snapshot(&self, state: &ExecutionState) -> ExecutionVersion {
        ExecutionVersion {
            id: uuid::Uuid::new_v4().to_string(),
            state: state.clone(),
            created_at: chrono::Utc::now(),
        }
    }

    /// Attempt to recover from a failed execution via replan + re-execute.
    pub async fn recover(&self, state: &ExecutionState, error: &str) -> Result<ExecutionState> {
        self.tracker
            .log_event("recovery_started", error)?;

        let plan = self.planner.replan(state, error).await?;
        self.tracker
            .log_event("replan_created", &format!("{} steps", plan.steps.len()))?;

        let new_state = self.executor.execute_plan(&plan, &self.sandbox).await?;

        let status_str = format!("{:?}", new_state.status);
        self.tracker
            .log_event("recovery_finished", &status_str)?;

        Ok(new_state)
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::execution::tool::Tool;
    use crate::core::execution::tool_router::DefaultToolRouter;
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
        async fn execute(&self, _params: &serde_json::Value, _sandbox: &Sandbox) -> Result<serde_json::Value> {
            Ok(serde_json::json!({ "ok": true }))
        }
        fn validate_params(&self, _params: &serde_json::Value) -> Result<()> {
            Ok(())
        }
    }

    fn make_service(tools: Vec<Arc<dyn Tool>>) -> ExecutionService {
        let mut router = DefaultToolRouter::new();
        for t in tools {
            router.register(t);
        }
        ExecutionService::build(Arc::new(router), Sandbox::new())
    }

    #[tokio::test]
    async fn execute_intent() {
        let svc = make_service(vec![Arc::new(StubTool {
            tool_name: "read".to_string(),
        })]);
        let state = svc.execute("read /tmp/file.txt").await.unwrap();
        assert_eq!(state.status, ExecutionStatus::Completed);
    }

    #[tokio::test]
    async fn execute_creates_snapshot() {
        let svc = make_service(vec![Arc::new(StubTool {
            tool_name: "analyze".to_string(),
        })]);
        let state = svc.execute("analyze data").await.unwrap();
        let version = svc.snapshot(&state);
        assert!(!version.id.is_empty());
        assert_eq!(version.state.status, ExecutionStatus::Completed);
    }

    #[tokio::test]
    async fn execute_logs_events() {
        let svc = make_service(vec![Arc::new(StubTool {
            tool_name: "run".to_string(),
        })]);
        let _ = svc.execute("run task").await.unwrap();
        // tracker should have plan_created + execution_finished
        let log = svc.tracker.get_log();
        assert!(log.iter().any(|e| e.event == "plan_created"));
        assert!(log.iter().any(|e| e.event == "execution_finished"));
    }
}
