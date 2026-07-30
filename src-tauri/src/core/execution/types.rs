use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Status of an individual step within a plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

/// Overall status of an execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionStatus {
    Planning,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

/// A single step in a plan — an atomic unit of work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub action: String,
    pub target: String,
    pub params: serde_json::Value,
    pub status: StepStatus,
}

/// An ordered sequence of steps to accomplish a goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub steps: Vec<Step>,
    pub created_at: DateTime<Utc>,
}

/// Result of executing a single step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_id: String,
    pub success: bool,
    pub output: serde_json::Value,
    pub error: Option<String>,
}

/// Live state of an in-progress or completed execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionState {
    pub plan: Plan,
    pub current_step: usize,
    pub status: ExecutionStatus,
    pub results: Vec<StepResult>,
}

/// A saved snapshot of execution state for history / replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionVersion {
    pub id: String,
    pub state: ExecutionState,
    pub created_at: DateTime<Utc>,
}

/// Single audit-log entry emitted during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionLog {
    pub timestamp: DateTime<Utc>,
    pub event: String,
    pub details: String,
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_step() -> Step {
        Step {
            id: "step-1".to_string(),
            action: "read".to_string(),
            target: "/tmp/file.txt".to_string(),
            params: serde_json::json!({}),
            status: StepStatus::Pending,
        }
    }

    fn sample_plan() -> Plan {
        Plan {
            id: "plan-1".to_string(),
            steps: vec![sample_step()],
            created_at: Utc::now(),
        }
    }

    #[test]
    fn step_status_eq() {
        assert_eq!(StepStatus::Pending, StepStatus::Pending);
        assert_ne!(StepStatus::Pending, StepStatus::Running);
    }

    #[test]
    fn execution_status_eq() {
        assert_eq!(ExecutionStatus::Running, ExecutionStatus::Running);
        assert_ne!(ExecutionStatus::Running, ExecutionStatus::Completed);
    }

    #[test]
    fn plan_serialization() {
        let plan = sample_plan();
        let json = serde_json::to_string(&plan).unwrap();
        let decoded: Plan = serde_json::from_str(&json).unwrap();
        assert_eq!(plan.id, decoded.id);
        assert_eq!(plan.steps.len(), decoded.steps.len());
    }

    #[test]
    fn step_result_success() {
        let sr = StepResult {
            step_id: "s1".to_string(),
            success: true,
            output: serde_json::json!({"ok": true}),
            error: None,
        };
        assert!(sr.success);
        assert!(sr.error.is_none());
    }

    #[test]
    fn step_result_failure() {
        let sr = StepResult {
            step_id: "s1".to_string(),
            success: false,
            output: serde_json::json!(null),
            error: Some("timeout".to_string()),
        };
        assert!(!sr.success);
        assert_eq!(sr.error.unwrap(), "timeout");
    }

    #[test]
    fn execution_state_serialization() {
        let state = ExecutionState {
            plan: sample_plan(),
            current_step: 0,
            status: ExecutionStatus::Running,
            results: vec![],
        };
        let json = serde_json::to_string(&state).unwrap();
        let decoded: ExecutionState = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.status, ExecutionStatus::Running);
        assert_eq!(decoded.current_step, 0);
    }

    #[test]
    fn execution_version_serialization() {
        let v = ExecutionVersion {
            id: "v1".to_string(),
            state: ExecutionState {
                plan: sample_plan(),
                current_step: 0,
                status: ExecutionStatus::Completed,
                results: vec![],
            },
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&v).unwrap();
        let decoded: ExecutionVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "v1");
    }

    #[test]
    fn execution_log_timestamp() {
        let log = ExecutionLog {
            timestamp: Utc::now(),
            event: "step_started".to_string(),
            details: "step-1".to_string(),
        };
        assert!(!log.event.is_empty());
    }
}
