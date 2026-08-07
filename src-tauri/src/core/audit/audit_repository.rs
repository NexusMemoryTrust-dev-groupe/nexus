use async_trait::async_trait;

use crate::core::audit::AuditEvent;
use crate::core::entity_id::EntityId;
use crate::core::result::Result;

/// Repository trait for the append-only decision journal.
#[async_trait]
pub trait AuditRepository: Send + Sync {
    /// Append one audit event to the journal.
    async fn add_event(&self, event: &AuditEvent) -> Result<()>;

    /// All events for one memory, chronological.
    async fn list_events(&self, memory_id: &EntityId) -> Result<Vec<AuditEvent>>;
}
