use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::core::domain_event::DomainEvent;
use crate::core::event_bus::{EventBus, SubscriptionId};
use crate::core::result::Result;

/// In-memory event bus for integration events.
/// Integration events coordinate between modules and external systems.
pub struct InMemoryIntegrationEventBus {
    sender: broadcast::Sender<DomainEvent>,
}

impl InMemoryIntegrationEventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }
}

impl Default for InMemoryIntegrationEventBus {
    fn default() -> Self {
        Self::new(128)
    }
}

#[async_trait]
impl EventBus for InMemoryIntegrationEventBus {
    async fn publish(&self, event: DomainEvent) -> Result<()> {
        let _ = self.sender.send(event);
        Ok(())
    }

    async fn subscribe(&self, handler: Box<dyn Fn(DomainEvent) + Send + Sync>) -> SubscriptionId {
        let mut rx = self.sender.subscribe();
        let id = uuid::Uuid::new_v4().to_string();

        tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                handler(event);
            }
        });

        id
    }

    async fn unsubscribe(&self, _id: SubscriptionId) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::domain_event::DomainEventType;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn integration_event_bus_publish() {
        let bus = InMemoryIntegrationEventBus::new(10);
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();

        bus.subscribe(Box::new(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        }))
        .await;

        let event = DomainEvent::new(
            DomainEventType::MemoryRecordCreated,
            serde_json::json!({"id": "mem-1"}),
        );
        bus.publish(event).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn integration_event_bus_default() {
        let bus = InMemoryIntegrationEventBus::default();
        let event = DomainEvent::new(
            DomainEventType::MemoryRecordUpdated,
            serde_json::json!({}),
        );
        assert!(bus.publish(event).await.is_ok());
    }
}
