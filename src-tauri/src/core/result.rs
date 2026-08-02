use thiserror::Error;

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
