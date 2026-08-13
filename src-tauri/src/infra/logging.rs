use std::time::{Duration, Instant};

use tracing::{Span, debug, error, info, warn};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

use crate::core::result::{AppError, Result};

/// Structured logging with tracing.
///
/// Every log line carries correlation IDs (`request_id`, `operation_id`) and,
/// for failures, the full error taxonomy (`error_code`, `severity`,
/// `component`, `recoverable`) so logs are machine-queryable and every error
/// is traceable end-to-end.
///
/// Default format is human-readable; set `NEXUS_LOG_FORMAT=json` for
/// machine consumption. Filter level via `RUST_LOG` (default: `info`).
pub fn init_logging() -> Result<()> {
    let format = std::env::var("NEXUS_LOG_FORMAT").unwrap_or_else(|_| "text".into());
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let registry = tracing_subscriber::registry().with(filter);

    if format == "json" {
        registry
            .with(
                fmt::layer()
                    .json()
                    .with_current_span(true)
                    .with_span_list(true)
                    .with_timer(fmt::time::UtcTime::rfc_3339()),
            )
            .try_init()
            .map_err(|e| AppError::Internal(format!("failed to init json logging: {e}")))?;
    } else {
        registry
            .with(
                fmt::layer()
                    .with_target(true)
                    .with_timer(fmt::time::UtcTime::rfc_3339()),
            )
            .try_init()
            .map_err(|e| AppError::Internal(format!("failed to init logging: {e}")))?;
    }

    Ok(())
}

/// A globally-unique correlation id for a request entering the system.
pub fn new_request_id() -> String {
    Uuid::new_v4().to_string()
}

/// A unique id for one logical operation within a request.
pub fn new_operation_id() -> String {
    Uuid::new_v4().to_string()
}

/// Guard for a timed operation.
///
/// Creates a tracing span carrying `request_id`/`operation_id`/`component`,
/// records duration on drop, and logs the final outcome. Usage:
///
/// ```
/// # use nexus::infra::logging::Operation;
/// let _op = Operation::begin("search", "retrieval", "req-123");
/// ```
pub struct Operation {
    component: &'static str,
    name: &'static str,
    request_id: String,
    operation_id: String,
    started: Instant,
    span: Span,
    outcome: Outcome,
}

#[derive(Clone, Copy)]
enum Outcome {
    Pending,
    Succeeded,
    Failed,
}

impl Operation {
    /// Start a timed operation with correlation ids.
    pub fn begin(name: &'static str, component: &'static str, request_id: &str) -> Self {
        let operation_id = new_operation_id();
        let span = tracing::info_span!(
            "op",
            name = name,
            component = component,
            request_id = %request_id,
            operation_id = %operation_id,
        );
        span.in_scope(
            || debug!(%name, %component, %request_id, %operation_id, "operation started"),
        );
        Operation {
            component,
            name,
            request_id: request_id.to_string(),
            operation_id,
            started: Instant::now(),
            span,
            outcome: Outcome::Pending,
        }
    }

    /// Mark the operation as failed with the error taxonomy attached.
    pub fn fail(&mut self, error: &AppError) {
        self.outcome = Outcome::Failed;
        let _entered = self.span.enter();
        let elapsed = self.started.elapsed();
        log_error_with_duration(error, elapsed, &self.request_id, &self.operation_id);
    }

    /// Mark the operation as successful, logging duration.
    pub fn succeed(&mut self) {
        self.outcome = Outcome::Succeeded;
        let _entered = self.span.enter();
        let elapsed = self.started.elapsed();
        info!(
            component = self.component,
            name = self.name,
            %self.request_id,
            %self.operation_id,
            duration_ms = elapsed.as_millis() as u64,
            "operation completed"
        );
    }
}

impl Drop for Operation {
    fn drop(&mut self) {
        if let Outcome::Pending = self.outcome {
            let elapsed = self.started.elapsed();
            debug!(
                component = self.component,
                name = self.name,
                %self.request_id,
                %self.operation_id,
                duration_ms = elapsed.as_millis() as u64,
                "operation dropped without explicit outcome"
            );
        }
    }
}

/// Log an error at the level implied by its severity, attaching the full
/// taxonomy so log consumers can group and page on stable fields.
pub fn log_error(error: &AppError) {
    log_error_with_duration(error, Duration::ZERO, "", "");
}

fn log_error_with_duration(
    error: &AppError,
    duration: Duration,
    request_id: &str,
    operation_id: &str,
) {
    let code = error.code();
    let severity = error.severity();
    let component = error.component();
    let recoverable = error.recoverable();
    let duration_ms = duration.as_millis() as u64;
    let rid = if request_id.is_empty() {
        None
    } else {
        Some(request_id)
    };
    let oid = if operation_id.is_empty() {
        None
    } else {
        Some(operation_id)
    };

    match severity {
        crate::core::result::Severity::Info => {
            info!(%code, %component, recoverable, duration_ms, rid, oid, error = %error, "operation degraded");
        }
        crate::core::result::Severity::Warning => {
            warn!(%code, %component, recoverable, duration_ms, rid, oid, error = %error, "operation warning");
        }
        crate::core::result::Severity::Error => {
            error!(%code, %component, recoverable, duration_ms, rid, oid, error = %error, "operation failed");
        }
        crate::core::result::Severity::Fatal => {
            error!(%code, %component, recoverable, duration_ms, rid, oid, error = %error, "FATAL failure");
        }
    }
}

/// Convenience wrapper: run a fallible closure inside a timed, correlated
/// operation, logging the taxonomy on failure and returning the error.
pub fn run_operation<T, F>(
    name: &'static str,
    component: &'static str,
    request_id: &str,
    f: F,
) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    let mut op = Operation::begin(name, component, request_id);
    let result = f();
    match &result {
        Ok(_) => op.succeed(),
        Err(e) => op.fail(e),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::result::AppError;

    #[test]
    fn ids_are_unique() {
        let a = new_request_id();
        let b = new_request_id();
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
    }

    #[test]
    fn operation_success_logs_duration() {
        let request_id = new_request_id();
        let result = run_operation("test_op", "test", &request_id, || Ok::<_, AppError>(42));
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn operation_failure_returns_error() {
        let request_id = new_request_id();
        let result = run_operation("test_fail", "test", &request_id, || {
            Err::<(), _>(AppError::mcp_failure("boom"))
        });
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code().as_str(), "MCP_FAILURE");
        assert!(err.recoverable());
        assert_eq!(err.component().as_str(), "mcp");
    }

    #[test]
    fn error_taxonomy_mapping() {
        let e = AppError::index_corruption("file");
        assert!(!e.recoverable());
        assert_eq!(e.severity(), crate::core::result::Severity::Fatal);
        assert_eq!(e.code().as_str(), "INDEX_CORRUPTION");
    }

    #[test]
    fn init_logging_does_not_panic() {
        // Can only init once per process; this guards the signature only.
        let _ = init_logging();
    }
}
