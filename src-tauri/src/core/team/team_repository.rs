use async_trait::async_trait;

use crate::core::entity_id::EntityId;
use crate::core::result::Result;
use crate::core::team::TeamMember;

/// Repository trait for team member persistence.
#[async_trait]
pub trait TeamRepository: Send + Sync {
    /// Persist a new team member.
    async fn add_member(&self, member: &TeamMember) -> Result<()>;

    /// Retrieve a member by their ID.
    async fn get_member(&self, id: &EntityId) -> Result<Option<TeamMember>>;

    /// Retrieve a member by their unique name.
    async fn get_member_by_name(&self, name: &str) -> Result<Option<TeamMember>>;

    /// List all team members.
    async fn list_members(&self) -> Result<Vec<TeamMember>>;

    /// Update an existing member in place.
    async fn update_member(&self, member: &TeamMember) -> Result<()>;

    /// Delete a team member permanently.
    async fn remove_member(&self, id: &EntityId) -> Result<()>;
}
