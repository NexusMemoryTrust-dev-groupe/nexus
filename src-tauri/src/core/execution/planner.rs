use async_trait::async_trait;

use crate::core::execution::types::{ExecutionState, Plan, Step, StepStatus};
use crate::core::result::Result;

/// Creates execution plans from intent + context.
#[async_trait]
pub trait Planner: Send + Sync {
    /// Build a new plan from a natural-language intent and current context.
    async fn create_plan(&self, intent: &str) -> Result<Plan>;

    /// Rebuild a plan after a step failure.
    async fn replan(&self, state: &ExecutionState, error: &str) -> Result<Plan>;
}

/// Keyword-based planner — no LLM dependency.
/// Splits intent on "; " or ". " to produce one step per sentence.
pub struct SimplePlanner;

impl SimplePlanner {
    pub fn new() -> Self {
        Self
    }

    /// Split a free-text intent into action/target pairs.
    /// Simple heuristic: each sentence becomes one step.
    fn parse_intent(intent: &str) -> Vec<Step> {
        // Split on ';' only — '.' breaks file paths like /tmp/file.txt
        let sentences: Vec<&str> = intent
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if sentences.is_empty() {
            return vec![Step {
                id: uuid::Uuid::new_v4().to_string(),
                action: "noop".to_string(),
                target: String::new(),
                params: serde_json::json!({}),
                status: StepStatus::Pending,
            }];
        }

        sentences
            .iter()
            .enumerate()
            .map(|(i, sentence)| {
                // First word = action, rest = target
                let mut parts = sentence.splitn(2, ' ');
                let action = parts.next().unwrap_or("noop").to_string();
                let target = parts.next().unwrap_or("").to_string();

                Step {
                    id: format!("step-{}-{}", i, uuid::Uuid::new_v4()),
                    action,
                    target,
                    params: serde_json::json!({}),
                    status: StepStatus::Pending,
                }
            })
            .collect()
    }
}

#[async_trait]
impl Planner for SimplePlanner {
    async fn create_plan(&self, intent: &str) -> Result<Plan> {
        let steps = Self::parse_intent(intent);
        Ok(Plan {
            id: uuid::Uuid::new_v4().to_string(),
            steps,
            created_at: chrono::Utc::now(),
        })
    }

    async fn replan(&self, state: &ExecutionState, error: &str) -> Result<Plan> {
        // On error, retry the failed step with a "retry" prefix
        let failed_step = state
            .plan
            .steps
            .iter()
            .find(|s| matches!(s.status, StepStatus::Failed));

        let steps = if let Some(step) = failed_step {
            vec![Step {
                id: format!("retry-{}", uuid::Uuid::new_v4()),
                action: "retry".to_string(),
                target: step.target.clone(),
                params: serde_json::json!({ "original_action": step.action, "error": error }),
                status: StepStatus::Pending,
            }]
        } else {
            vec![Step {
                id: format!("replan-{}", uuid::Uuid::new_v4()),
                action: "replan".to_string(),
                target: error.to_string(),
                params: serde_json::json!({}),
                status: StepStatus::Pending,
            }]
        };

        Ok(Plan {
            id: uuid::Uuid::new_v4().to_string(),
            steps,
            created_at: chrono::Utc::now(),
        })
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_plan_single_intent() {
        let planner = SimplePlanner::new();
        let plan = planner.create_plan("read /tmp/file.txt").await.unwrap();
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].action, "read");
        assert_eq!(plan.steps[0].target, "/tmp/file.txt");
    }

    #[tokio::test]
    async fn create_plan_multi_step() {
        let planner = SimplePlanner::new();
        let plan = planner
            .create_plan("read /tmp/a.txt; write /tmp/b.txt")
            .await
            .unwrap();
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].action, "read");
        assert_eq!(plan.steps[1].action, "write");
    }

    #[tokio::test]
    async fn create_plan_empty_intent() {
        let planner = SimplePlanner::new();
        let plan = planner.create_plan("").await.unwrap();
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].action, "noop");
    }

    #[tokio::test]
    async fn create_plan_semicolon_separated() {
        let planner = SimplePlanner::new();
        let plan = planner
            .create_plan("analyze data; generate report")
            .await
            .unwrap();
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].action, "analyze");
        assert_eq!(plan.steps[1].action, "generate");
    }

    #[tokio::test]
    async fn replan_on_failure() {
        let planner = SimplePlanner::new();
        let state = ExecutionState {
            plan: Plan {
                id: "p1".to_string(),
                steps: vec![Step {
                    id: "s1".to_string(),
                    action: "read".to_string(),
                    target: "/missing".to_string(),
                    params: serde_json::json!({}),
                    status: StepStatus::Failed,
                }],
                created_at: chrono::Utc::now(),
            },
            current_step: 0,
            status: crate::core::execution::types::ExecutionStatus::Failed,
            results: vec![],
        };
        let plan = planner.replan(&state, "file not found").await.unwrap();
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].action, "retry");
    }

    #[tokio::test]
    async fn replan_no_failure() {
        let planner = SimplePlanner::new();
        let state = ExecutionState {
            plan: Plan {
                id: "p1".to_string(),
                steps: vec![],
                created_at: chrono::Utc::now(),
            },
            current_step: 0,
            status: crate::core::execution::types::ExecutionStatus::Running,
            results: vec![],
        };
        let plan = planner.replan(&state, "unknown error").await.unwrap();
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].action, "replan");
    }
}
