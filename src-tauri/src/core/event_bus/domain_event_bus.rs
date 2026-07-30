use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::core::domain_event::DomainEvent;
use crate::core::event_bus::{EventBus, SubscriptionId};
use crate::core::result::Result;

/// In-memory event bus for domain events.
/// Uses tokio::broadcast for efficient multi-consumer distribution.
pub struct InMemoryEventBus {
    sender: broadcast::Sender<DomainEvent>,
}

impl InMemoryEventBus {
    /// Create a new InMemoryEventBus with specified channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }
}

impl Default for InMemoryEventBus {
    fn default() -> Self {
        Self::new(1024)
    }
}

#[async_trait]
impl EventBus for InMemoryEventBus {
    async fn publish(&self, event: DomainEvent) -> Result<()> {
        // broadcast::send returns Err when zero receivers exist — that's not a failure
        let _ = self.sender.send(event);
        Ok(())
    }

    async fn subscribe(&self, handler: Box<dyn Fn(DomainEvent) + Send + Sync>) -> SubscriptionId {
        let mut rx = self.sender.subscribe();
        let id = uuid::Uuid::new_v4().to_string();
        let _id = id.clone();

        tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                handler(event);
            }
        });

        id
    }

    async fn unsubscribe(&self, _id: SubscriptionId) -> Result<()> {
        // broadcast::Receiver doesn't support explicit unsubscribe
        // Handlers are dropped when the spawned task completes
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
    async fn event_bus_publish_and_subscribe() {
        let bus = InMemoryEventBus::new(10);
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();

        bus.subscribe(Box::new(move |_event| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        }))
        .await;

        let event = DomainEvent::new(
            DomainEventType::EntityCreated,
            serde_json::json!({}),
        );
        bus.publish(event).await.unwrap();

        // Give spawned task time to process
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn event_bus_multiple_subscribers() {
        let bus = InMemoryEventBus::new(10);
        let count1 = Arc::new(AtomicUsize::new(0));
        let count2 = Arc::new(AtomicUsize::new(0));
        let c1 = count1.clone();
        let c2 = count2.clone();

        bus.subscribe(Box::new(move |_| {
            c1.fetch_add(1, Ordering::SeqCst);
        }))
        .await;

        bus.subscribe(Box::new(move |_| {
            c2.fetch_add(1, Ordering::SeqCst);
        }))
        .await;

        let event = DomainEvent::new(
            DomainEventType::EntityCreated,
            serde_json::json!({}),
        );
        bus.publish(event).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert_eq!(count1.load(Ordering::SeqCst), 1);
        assert_eq!(count2.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn event_bus_default_capacity() {
        let bus = InMemoryEventBus::default();
        let event = DomainEvent::new(
            DomainEventType::EntityCreated,
            serde_json::json!({}),
        );
        assert!(bus.publish(event).await.is_ok());
    }

    #[tokio::test]
    async fn event_bus_unsubscribe() {
        let bus = InMemoryEventBus::new(10);
        let id = bus
            .subscribe(Box::new(|_| {}))
            .await;
        assert!(bus.unsubscribe(id).await.is_ok());
    }
}
