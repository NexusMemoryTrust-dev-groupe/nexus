use async_trait::async_trait;

use crate::core::entity_id::EntityId;
use crate::core::result::Result;
use crate::core::versioning::causality_record::CausalityRecord;

/// Service for tracing causality chains — "why did this change?" and "what did this affect?".
#[async_trait]
pub trait CausalityChain: Send + Sync {
    /// Trace all causes that led to a specific version.
    async fn trace_causes(
        &self,
        entity_id: &EntityId,
        version_id: &str,
    ) -> Result<Vec<CausalityRecord>>;

    /// Find all effects triggered by a specific cause.
    async fn find_effects(&self, cause_id: &str) -> Result<Vec<CausalityRecord>>;

    /// Record a new causality link.
    async fn record_causality(
        &self,
        entity_id: &EntityId,
        version_id: &str,
        reason: &str,
        affected: &[EntityId],
    ) -> Result<()>;
}

#[cfg(test)]
mod tests {
    // CausalityChain is a trait — concrete implementations tested in storage/sqlite/
}
