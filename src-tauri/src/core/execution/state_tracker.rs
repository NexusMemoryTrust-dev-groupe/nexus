use std::sync::Mutex;

use crate::core::execution::types::{ExecutionLog, ExecutionState};
use crate::core::result::Result;

/// Tracks execution state and maintains an audit log.
/// All methods take `&self` (interior mutability) so the trait is dyn-safe behind Arc.
pub trait ExecutionStateTracker: Send + Sync {
    /// Replace the current state snapshot.
    fn update_state(&self, state: &ExecutionState) -> Result<()>;

    /// Return a clone of the latest state.
    fn get_state(&self) -> ExecutionState;

    /// Return a clone of the full audit log.
    fn get_log(&self) -> Vec<ExecutionLog>;

    /// Append an entry to the audit log.
    fn log_event(&self, event: &str, details: &str) -> Result<()>;
}

/// In-memory state tracker with interior mutability (Mutex).
pub struct InMemoryStateTracker {
    state: Mutex<ExecutionState>,
    log: Mutex<Vec<ExecutionLog>>,
}

impl InMemoryStateTracker {
    pub fn new(initial_state: ExecutionState) -> Self {
        Self {
            state: Mutex::new(initial_state),
            log: Mutex::new(Vec::new()),
        }
    }
}

impl ExecutionStateTracker for InMemoryStateTracker {
    fn update_state(&self, state: &ExecutionState) -> Result<()> {
        let mut current = self
            .state
            .lock()
            .map_err(|e| crate::core::AppError::Internal(format!("Lock poisoned: {}", e)))?;
        *current = state.clone();
        Ok(())
    }

    fn get_state(&self) -> ExecutionState {
        self.state.lock().unwrap().clone()
    }

    fn get_log(&self) -> Vec<ExecutionLog> {
        self.log.lock().unwrap().clone()
    }

    fn log_event(&self, event: &str, details: &str) -> Result<()> {
        let mut log = self
            .log
            .lock()
            .map_err(|e| crate::core::AppError::Internal(format!("Lock poisoned: {}", e)))?;
        log.push(ExecutionLog {
            timestamp: chrono::Utc::now(),
            event: event.to_string(),
            details: details.to_string(),
        });
        Ok(())
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::execution::types::{ExecutionStatus, Plan};

    fn make_state() -> ExecutionState {
        ExecutionState {
            plan: Plan {
                id: "p1".to_string(),
                steps: vec![],
                created_at: chrono::Utc::now(),
            },
            current_step: 0,
            status: ExecutionStatus::Planning,
            results: vec![],
        }
    }

    #[test]
    fn new_tracker_starts_empty_log() {
        let tracker = InMemoryStateTracker::new(make_state());
        assert!(tracker.get_log().is_empty());
    }

    #[test]
    fn update_and_get_state() {
        let tracker = InMemoryStateTracker::new(make_state());
        let mut new_state = make_state();
        new_state.status = ExecutionStatus::Running;
        tracker.update_state(&new_state).unwrap();
        assert_eq!(tracker.get_state().status, ExecutionStatus::Running);
    }

    #[test]
    fn log_event_adds_entry() {
        let tracker = InMemoryStateTracker::new(make_state());
        tracker.log_event("step_started", "step-1").unwrap();
        assert_eq!(tracker.get_log().len(), 1);
        assert_eq!(tracker.get_log()[0].event, "step_started");
    }

    #[test]
    fn log_multiple_events() {
        let tracker = InMemoryStateTracker::new(make_state());
        tracker.log_event("a", "1").unwrap();
        tracker.log_event("b", "2").unwrap();
        tracker.log_event("c", "3").unwrap();
        assert_eq!(tracker.get_log().len(), 3);
    }
}
