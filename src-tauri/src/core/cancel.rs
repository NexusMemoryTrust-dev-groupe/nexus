//! Cooperative cancellation for long-running operations (plan 5.4).
//!
//! Heavy work — indexing, embedding, retrieval, rehearsal, backup — can take
//! seconds or minutes. When the user navigates away or shuts the app down,
//! that work must stop at a checkpoint, leave the system in a consistent
//! state and report `AppError::Cancelled` instead of silently continuing or
//! dying mid-write.
//!
//! [`CancelToken`] is a cheap, cloneable, thread-safe flag: heavy loops call
//! [`CancelToken::check`] between batches and bail out with
//! `AppError::cancelled(...)` as soon as a cancel is requested. The token is
//! *cooperative* — a loop that never checks the token cannot be cancelled,
//! so every heavy operation MUST check it at least once per batch.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::core::result::{AppError, Result};

/// A handle to cancel cooperative work.
///
/// Cloneable and `Send + Sync`: the holder can be passed to worker threads
/// while the caller keeps a copy to cancel from anywhere.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    cancelled: Arc<AtomicBool>,
}

impl CancelToken {
    /// A fresh, uncancelled token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Request cancellation. Idempotent; subsequent calls are no-ops.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Whether cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Bail out if cancellation was requested.
    ///
    /// Call this at every batch boundary in a heavy loop:
    /// ```no_run
    /// # use nexus::core::cancel::CancelToken;
    /// # use nexus::core::result::Result;
    /// fn heavy_loop(token: &CancelToken) -> Result<()> {
    ///     loop {
    ///         token.check("indexing")?; // stops as soon as cancel is requested
    ///         // ... one batch of work ...
    ///     }
    /// }
    /// ```
    pub fn check(&self, what: &str) -> Result<()> {
        if self.is_cancelled() {
            return Err(AppError::cancelled(what.to_string()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_token_is_not_cancelled() {
        let t = CancelToken::new();
        assert!(!t.is_cancelled());
        t.check("test").expect("fresh token must not fail");
    }

    #[test]
    fn cancel_is_observed() {
        let t = CancelToken::new();
        t.cancel();
        assert!(t.is_cancelled());
        let err = t.check("indexing").unwrap_err();
        assert!(err.to_string().contains("Cancelled"));
        assert!(err.to_string().contains("indexing"));
        assert_eq!(err.code().as_str(), "CANCELLED");
    }

    #[test]
    fn cancel_is_idempotent() {
        let t = CancelToken::new();
        t.cancel();
        t.cancel(); // no panic, no state change
        assert!(t.is_cancelled());
    }

    #[test]
    fn clones_share_state() {
        let t = CancelToken::new();
        let worker = t.clone();
        // The worker sees cancels issued through the original handle.
        t.cancel();
        assert!(worker.is_cancelled());
        assert!(worker.check("embedding").is_err());
    }

    #[test]
    fn clone_can_cancel_original() {
        let t = CancelToken::new();
        let worker = t.clone();
        worker.cancel();
        assert!(t.is_cancelled());
    }

    #[test]
    fn check_returns_ok_when_not_cancelled() {
        let t = CancelToken::new();
        for i in 0..100 {
            t.check("rehearsal").expect("no cancel yet");
            let _ = i;
        }
    }
}
