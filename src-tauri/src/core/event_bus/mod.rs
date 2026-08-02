pub mod application_event_bus;
pub mod domain_event_bus;
pub mod integration_event_bus;

use async_trait::async_trait;

use crate::core::domain_event::DomainEvent;
use crate::core::result::Result;

/// Unique identifier for a subscription.
pub type SubscriptionId = String;

/// Trait for all event buses.
/// Event buses handle publishing and subscribing to domain events.
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publish an event to all subscribers.
    async fn publish(&self, event: DomainEvent) -> Result<()>;

    /// Subscribe to events with a handler.
    /// Returns a SubscriptionId for later unsubscription.
    async fn subscribe(&self, handler: Box<dyn Fn(DomainEvent) + Send + Sync>) -> SubscriptionId;

    /// Unsubscribe by subscription ID.
    async fn unsubscribe(&self, id: SubscriptionId) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_id_is_string() {
        let id: SubscriptionId = "test-123".to_string();
        assert_eq!(id, "test-123");
    }
}
