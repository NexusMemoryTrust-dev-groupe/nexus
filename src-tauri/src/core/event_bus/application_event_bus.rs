use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::core::domain_event::DomainEvent;
use crate::core::event_bus::{EventBus, SubscriptionId};
use crate::core::result::Result;

/// In-memory event bus for application events.
/// Application events coordinate use cases and workflows.
pub struct InMemoryApplicationEventBus {
    sender: broadcast::Sender<DomainEvent>,
}

impl InMemoryApplicationEventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }
}

impl Default for InMemoryApplicationEventBus {
    fn default() -> Self {
        Self::new(256)
    }
}

#[async_trait]
impl EventBus for InMemoryApplicationEventBus {
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
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn application_event_bus_publish() {
        let bus = InMemoryApplicationEventBus::new(10);
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();

        bus.subscribe(Box::new(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        }))
        .await;

        let event = DomainEvent::new(
            DomainEventType::ExecutionCompleted,
            serde_json::json!({"step": 1}),
        );
        bus.publish(event).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn application_event_bus_default() {
        let bus = InMemoryApplicationEventBus::default();
        let event = DomainEvent::new(DomainEventType::DecisionMade, serde_json::json!({}));
        assert!(bus.publish(event).await.is_ok());
    }
}
