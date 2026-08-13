use thiserror::Error;

/// Production error taxonomy (Production Readiness Gate, item 4).
///
/// Every failure in the system maps onto one of these stable codes. The code
/// is the *kind* of problem; `AppError::code()` derives it from the variant so
/// no call site has to annotate anything — classification is automatic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// Retrieval returned nothing relevant for a valid query.
    RetrievalMiss,
    /// Retrieval returned the wrong context (relevance failure).
    WrongContext,
    /// A known conflict was not detected.
    ConflictMiss,
    /// A conflict was flagged where none exists.
    FalseConflict,
    /// A memory is stale/aged and should not drive decisions.
    StaleMemory,
    /// The operation was denied by permissions/firewall.
    PermissionDenied,
    /// A permission boundary was bypassed.
    PermissionBypass,
    /// Provenance chain was lost or corrupted.
    ProvenanceLoss,
    /// Token budget was exceeded while building context.
    TokenOverflow,
    /// A schema migration failed.
    MigrationFailure,
    /// The semantic index is corrupted.
    IndexCorruption,
    /// An MCP call failed.
    McpFailure,
    /// Entity or record not found.
    NotFound,
    /// Input failed validation.
    Validation,
    /// Internal invariant violated.
    Internal,
    /// Authentication missing.
    Unauthorized,
    /// Action forbidden.
    Forbidden,
    /// State conflict (e.g. illegal lifecycle transition).
    Conflict,
    /// Storage/database failure.
    Database,
    /// Configuration failure.
    Configuration,
    /// Serialization failure.
    Serialization,
    /// I/O failure.
    Io,
    /// Security boundary violation.
    Security,
    /// A module was requested before it loaded.
    ModuleNotLoaded,
    /// The operation was cancelled (plan 5.4).
    Cancelled,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::RetrievalMiss => "RETRIEVAL_MISS",
            ErrorCode::WrongContext => "WRONG_CONTEXT",
            ErrorCode::ConflictMiss => "CONFLICT_MISS",
            ErrorCode::FalseConflict => "FALSE_CONFLICT",
            ErrorCode::StaleMemory => "STALE_MEMORY",
            ErrorCode::PermissionDenied => "PERMISSION_DENIED",
            ErrorCode::PermissionBypass => "PERMISSION_BYPASS",
            ErrorCode::ProvenanceLoss => "PROVENANCE_LOSS",
            ErrorCode::TokenOverflow => "TOKEN_OVERFLOW",
            ErrorCode::MigrationFailure => "MIGRATION_FAILURE",
            ErrorCode::IndexCorruption => "INDEX_CORRUPTION",
            ErrorCode::McpFailure => "MCP_FAILURE",
            ErrorCode::NotFound => "NOT_FOUND",
            ErrorCode::Validation => "VALIDATION",
            ErrorCode::Internal => "INTERNAL",
            ErrorCode::Unauthorized => "UNAUTHORIZED",
            ErrorCode::Forbidden => "FORBIDDEN",
            ErrorCode::Conflict => "CONFLICT",
            ErrorCode::Database => "DATABASE",
            ErrorCode::Configuration => "CONFIGURATION",
            ErrorCode::Serialization => "SERIALIZATION",
            ErrorCode::Io => "IO",
            ErrorCode::Security => "SECURITY",
            ErrorCode::ModuleNotLoaded => "MODULE_NOT_LOADED",
            ErrorCode::Cancelled => "CANCELLED",
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Severity of an error — drives log level and UI treatment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informational — operation completed but with a caveat.
    Info,
    /// The operation failed but the system stays healthy.
    Warning,
    /// The operation failed; the affected subsystem is degraded.
    Error,
    /// The failure prevents the application from continuing safely.
    Fatal,
}

/// The subsystem that produced the error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Component {
    Retrieval,
    Conflict,
    Context,
    Lifecycle,
    Graph,
    Storage,
    Migration,
    Index,
    Mcp,
    Security,
    Audit,
    Backup,
    Versioning,
    Rehearsal,
    Skills,
    Config,
    Ui,
    Unknown,
}

impl Component {
    pub fn as_str(&self) -> &'static str {
        match self {
            Component::Retrieval => "retrieval",
            Component::Conflict => "conflict",
            Component::Context => "context",
            Component::Lifecycle => "lifecycle",
            Component::Graph => "graph",
            Component::Storage => "storage",
            Component::Migration => "migration",
            Component::Index => "index",
            Component::Mcp => "mcp",
            Component::Security => "security",
            Component::Audit => "audit",
            Component::Backup => "backup",
            Component::Versioning => "versioning",
            Component::Rehearsal => "rehearsal",
            Component::Skills => "skills",
            Component::Config => "config",
            Component::Ui => "ui",
            Component::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for Component {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Unified application error type.
/// All business logic errors flow through this enum.
/// Exceptions only for crashes — everything else is Result<T>.
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Database: {0}")]
    Database(String),

    #[error("Configuration: {0}")]
    Configuration(String),

    #[error("Serialization: {0}")]
    Serialization(String),

    #[error("IO: {0}")]
    Io(String),

    #[error("Security: {0}")]
    Security(String),

    #[error("Module not loaded: {0}")]
    ModuleNotLoaded(String),

    /// Retrieval returned nothing relevant. Carries the query for diagnostics.
    #[error("Retrieval miss: {0}")]
    RetrievalMiss(String),

    /// A conflict engine failure (detection or resolution).
    #[error("Conflict engine: {0}")]
    ConflictEngine(String),

    /// The semantic index is corrupted / unusable.
    #[error("Index corruption: {0}")]
    IndexCorruption(String),

    /// An MCP operation failed.
    #[error("MCP failure: {0}")]
    McpFailure(String),

    /// A backup or restore operation failed.
    #[error("Backup failure: {0}")]
    BackupFailure(String),

    /// A schema migration failed.
    #[error("Migration failure: {0}")]
    MigrationFailure(String),

    /// A token budget was exceeded.
    #[error("Token overflow: {0}")]
    TokenOverflow(String),

    /// The operation was cancelled by the caller (plan 5.4). Not an error in
    /// the failure sense — the work simply stopped at a checkpoint and the
    /// system is in a consistent state.
    #[error("Cancelled: {0}")]
    Cancelled(String),
}

impl AppError {
    /// Stable taxonomy code for the error. Used for structured logs, audit and
    /// MCP error responses so consumers can match on a code, not a message.
    pub fn code(&self) -> ErrorCode {
        match self {
            AppError::NotFound(_) => ErrorCode::NotFound,
            AppError::Validation(_) => ErrorCode::Validation,
            AppError::Internal(_) => ErrorCode::Internal,
            AppError::Unauthorized => ErrorCode::Unauthorized,
            AppError::Forbidden(_) => ErrorCode::Forbidden,
            AppError::Conflict(_) => ErrorCode::Conflict,
            AppError::Database(_) => ErrorCode::Database,
            AppError::Configuration(_) => ErrorCode::Configuration,
            AppError::Serialization(_) => ErrorCode::Serialization,
            AppError::Io(_) => ErrorCode::Io,
            AppError::Security(_) => ErrorCode::Security,
            AppError::ModuleNotLoaded(_) => ErrorCode::ModuleNotLoaded,
            AppError::RetrievalMiss(_) => ErrorCode::RetrievalMiss,
            AppError::ConflictEngine(_) => ErrorCode::ConflictMiss,
            AppError::IndexCorruption(_) => ErrorCode::IndexCorruption,
            AppError::McpFailure(_) => ErrorCode::McpFailure,
            AppError::BackupFailure(_) => ErrorCode::Internal,
            AppError::MigrationFailure(_) => ErrorCode::MigrationFailure,
            AppError::TokenOverflow(_) => ErrorCode::TokenOverflow,
            AppError::Cancelled(_) => ErrorCode::Cancelled,
        }
    }

    /// Severity of the failure. Defaults are safe; callers can special-case.
    pub fn severity(&self) -> Severity {
        match self {
            AppError::Validation(_)
            | AppError::Forbidden(_)
            | AppError::Unauthorized
            | AppError::NotFound(_) => Severity::Warning,
            AppError::Internal(_)
            | AppError::Database(_)
            | AppError::Serialization(_)
            | AppError::Io(_)
            | AppError::Configuration(_)
            | AppError::RetrievalMiss(_)
            | AppError::ConflictEngine(_)
            | AppError::McpFailure(_)
            | AppError::BackupFailure(_)
            | AppError::MigrationFailure(_)
            | AppError::TokenOverflow(_) => Severity::Error,
            AppError::Cancelled(_) => Severity::Warning,
            AppError::Security(_) | AppError::IndexCorruption(_) | AppError::ModuleNotLoaded(_) => {
                Severity::Fatal
            }
            AppError::Conflict(_) => Severity::Warning,
        }
    }

    /// The subsystem that produced this error. Most variants map to a single
    /// component; multi-purpose variants are disambiguated by the caller when
    /// needed via [`AppError::with_component`]-style helpers at construction.
    pub fn component(&self) -> Component {
        match self {
            AppError::NotFound(_) => Component::Storage,
            AppError::Validation(_) => Component::Ui,
            AppError::Internal(_) => Component::Unknown,
            AppError::Unauthorized | AppError::Forbidden(_) | AppError::Security(_) => {
                Component::Security
            }
            AppError::Conflict(_) => Component::Lifecycle,
            AppError::Database(_) => Component::Storage,
            AppError::Configuration(_) => Component::Config,
            AppError::Serialization(_) => Component::Storage,
            AppError::Io(_) => Component::Storage,
            AppError::ModuleNotLoaded(_) => Component::Unknown,
            AppError::RetrievalMiss(_) => Component::Retrieval,
            AppError::ConflictEngine(_) => Component::Conflict,
            AppError::IndexCorruption(_) => Component::Index,
            AppError::McpFailure(_) => Component::Mcp,
            AppError::BackupFailure(_) => Component::Backup,
            AppError::MigrationFailure(_) => Component::Migration,
            AppError::TokenOverflow(_) => Component::Context,
            AppError::Cancelled(_) => Component::Unknown,
        }
    }

    /// Whether retrying the operation could succeed. Persistent failures
    /// (corruption, permissions) are not recoverable by a blind retry.
    pub fn recoverable(&self) -> bool {
        match self {
            AppError::NotFound(_)
            | AppError::Validation(_)
            | AppError::Unauthorized
            | AppError::Forbidden(_)
            | AppError::Security(_)
            | AppError::IndexCorruption(_)
            | AppError::ModuleNotLoaded(_) => false,
            AppError::Internal(_)
            | AppError::Conflict(_)
            | AppError::Database(_)
            | AppError::Configuration(_)
            | AppError::Serialization(_)
            | AppError::Io(_)
            | AppError::RetrievalMiss(_)
            | AppError::ConflictEngine(_)
            | AppError::McpFailure(_)
            | AppError::BackupFailure(_)
            | AppError::MigrationFailure(_)
            | AppError::TokenOverflow(_)
            | AppError::Cancelled(_) => true,
        }
    }

    /// Convenience constructor for cancellation (plan 5.4).
    pub fn cancelled(what: impl Into<String>) -> Self {
        AppError::Cancelled(what.into())
    }

    /// Convenience constructors for the taxonomy variants, so call sites read
    /// `AppError::retrieval_miss("query")` instead of spelling the variant.
    pub fn retrieval_miss(query: impl Into<String>) -> Self {
        AppError::RetrievalMiss(query.into())
    }

    pub fn conflict_engine(reason: impl Into<String>) -> Self {
        AppError::ConflictEngine(reason.into())
    }

    pub fn index_corruption(reason: impl Into<String>) -> Self {
        AppError::IndexCorruption(reason.into())
    }

    pub fn mcp_failure(reason: impl Into<String>) -> Self {
        AppError::McpFailure(reason.into())
    }

    pub fn backup_failure(reason: impl Into<String>) -> Self {
        AppError::BackupFailure(reason.into())
    }

    pub fn migration_failure(reason: impl Into<String>) -> Self {
        AppError::MigrationFailure(reason.into())
    }

    pub fn token_overflow(reason: impl Into<String>) -> Self {
        AppError::TokenOverflow(reason.into())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Database(err.to_string())
    }
}

/// Convenience Result alias for application errors.
pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_error_display_not_found() {
        let err = AppError::NotFound("entity-123".to_string());
        assert_eq!(err.to_string(), "Not found: entity-123");
    }

    #[test]
    fn app_error_display_validation() {
        let err = AppError::Validation("bad input".to_string());
        assert_eq!(err.to_string(), "Validation failed: bad input");
    }

    #[test]
    fn app_error_display_unauthorized() {
        let err = AppError::Unauthorized;
        assert_eq!(err.to_string(), "Unauthorized");
    }

    #[test]
    fn app_error_from_serde_json() {
        let serde_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let app_err: AppError = serde_err.into();
        assert!(matches!(app_err, AppError::Serialization(_)));
    }

    #[test]
    fn app_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let app_err: AppError = io_err.into();
        assert!(matches!(app_err, AppError::Io(_)));
    }

    #[test]
    fn app_error_from_rusqlite() {
        let sqlite_err = rusqlite::Error::ExecuteReturnedResults;
        let app_err: AppError = sqlite_err.into();
        assert!(matches!(app_err, AppError::Database(_)));
    }

    #[test]
    fn result_ok_type() {
        let r: Result<i32> = Ok(42);
        assert!(matches!(r, Ok(42)));
    }

    #[test]
    fn result_err_type() {
        let r: Result<i32> = Err(AppError::Internal("test".to_string()));
        assert!(r.is_err());
    }
}
