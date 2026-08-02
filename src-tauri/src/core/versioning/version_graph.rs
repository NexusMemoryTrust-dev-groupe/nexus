use async_trait::async_trait;

use crate::core::entity_id::EntityId;
use crate::core::result::Result;
use crate::core::versioning::automatic_commit::AutomaticCommit;
use crate::core::versioning::version_edge::VersionEdgeType;

/// Service for querying the version graph — lineage, dependents, and edges.
#[async_trait]
pub trait VersionGraph: Send + Sync {
    /// Get the full version lineage (all commits) for an entity, ordered by version number.
    async fn get_lineage(&self, entity_id: &EntityId) -> Result<Vec<AutomaticCommit>>;

    /// Get all commits that depend on (were triggered by) a specific version.
    async fn get_dependents(&self, version_id: &str) -> Result<Vec<AutomaticCommit>>;

    /// Add a directed edge between two versions in the graph.
    async fn add_edge(&self, from: &str, to: &str, edge_type: VersionEdgeType) -> Result<()>;
}

#[cfg(test)]
mod tests {
    // VersionGraph is a trait — concrete implementations tested in storage/sqlite/
}
