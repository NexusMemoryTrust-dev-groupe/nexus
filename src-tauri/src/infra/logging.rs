use tracing_subscriber::{EnvFilter, fmt};

use crate::core::result::Result;

/// Initialize structured logging with tracing.
/// Uses RUST_LOG environment variable for filter configuration.
/// Default level: info.
pub fn init_logging() -> Result<()> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_logging_does_not_panic() {
        // Note: can only call init_logging once per process
        // This test just verifies the function signature is correct
        let _ = init_logging();
    }
}
