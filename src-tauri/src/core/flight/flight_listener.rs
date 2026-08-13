//! Flight Recorder listener — мост event_bus → журнал полёта.
//!
//! Подписывается на доменную шину событий (как versioning_listener) и
//! оседает каждое событие в журнале полёта: категория, действие, сущность,
//! результат. Это расширяет существующий event_bus — ничего не заменяя,
//! самописец просто слушает и записывает.

use std::sync::Arc;

use crate::core::domain_event::DomainEvent;
use crate::core::flight::flight_recorder::{
    FlightRecord, FlightRepository, record_from_domain_event,
};

/// Создаёт обработчик шины событий, который пишет каждое событие в журнал
/// полёта. Асинхронная запись выполняется в отдельной задаче, чтобы не
/// блокировать шину.
pub fn create_flight_listener(
    repository: Arc<dyn FlightRepository>,
) -> Box<dyn Fn(DomainEvent) + Send + Sync> {
    Box::new(move |event: DomainEvent| {
        let repo = repository.clone();
        let record = record_from_domain_event(&event);
        let event_id = event.id.clone();

        tokio::spawn(async move {
            match repo.add_record(&record).await {
                Ok(()) => {
                    tracing::trace!("Flight recorder logged {} ({})", record.action, event_id);
                }
                Err(e) => {
                    tracing::warn!("Flight recorder failed to log event {}: {}", event_id, e);
                }
            }
        });
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::domain_event::DomainEventType;
    use crate::core::flight::flight_recorder::{
        FlightCategory, FlightRecord, FlightSession, FlightStats,
    };
    use crate::core::result::Result;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingRepo {
        calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl FlightRepository for CountingRepo {
        async fn create_session(&self, _session: &FlightSession) -> Result<()> {
            Ok(())
        }
        async fn close_session(&self, _session_id: &str) -> Result<()> {
            Ok(())
        }
        async fn list_active_sessions(&self, _limit: u32) -> Result<Vec<FlightSession>> {
            Ok(vec![])
        }
        async fn add_record(&self, _record: &FlightRecord) -> Result<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn recent_records(
            &self,
            _limit: u32,
            _category: Option<&str>,
        ) -> Result<Vec<FlightRecord>> {
            Ok(vec![])
        }
        async fn session_records(&self, _session_id: &str) -> Result<Vec<FlightRecord>> {
            Ok(vec![])
        }
        async fn entity_replay(
            &self,
            _entity_type: &str,
            _entity_id: &str,
        ) -> Result<Vec<FlightRecord>> {
            Ok(vec![])
        }
        async fn stats(&self) -> Result<FlightStats> {
            Ok(FlightStats::default())
        }
    }

    #[tokio::test]
    async fn listener_records_memory_created_event() {
        let repo = Arc::new(CountingRepo {
            calls: AtomicUsize::new(0),
        });
        let handler = create_flight_listener(repo.clone());

        let event = DomainEvent::new(
            DomainEventType::MemoryRecordCreated,
            serde_json::json!({"record_id": "mem-1"}),
        );
        handler(event);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(repo.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn listener_records_any_event_type() {
        let repo = Arc::new(CountingRepo {
            calls: AtomicUsize::new(0),
        });
        let handler = create_flight_listener(repo.clone());

        // Слушатель не фильтрует: каждый доменное событие — запись полёта.
        for event_type in [
            DomainEventType::EntityCreated,
            DomainEventType::EntityUpdated,
            DomainEventType::EntityDeleted,
            DomainEventType::RelationshipCreated,
            DomainEventType::RelationshipDeleted,
            DomainEventType::MemoryRecordCreated,
            DomainEventType::MemoryRecordUpdated,
            DomainEventType::ExecutionCompleted,
            DomainEventType::DecisionMade,
        ] {
            handler(DomainEvent::new(event_type, serde_json::json!({})));
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(repo.calls.load(Ordering::SeqCst), 9);
    }

    #[test]
    fn record_from_memory_event_is_categorised() {
        let event = DomainEvent::new(
            DomainEventType::MemoryRecordCreated,
            serde_json::json!({"record_id": "mem-x"}),
        );
        let record = record_from_domain_event(&event);
        assert_eq!(record.category, FlightCategory::Memory);
        assert_eq!(record.action, "create_memory");
        assert_eq!(record.entity_id, "mem-x");
    }
}
