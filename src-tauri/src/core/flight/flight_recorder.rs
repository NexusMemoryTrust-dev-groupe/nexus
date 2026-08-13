//! Flight Recorder — бортовой самописец операций (Система 5).
//!
//! Записывает каждый значимый шаг экосистемы в журнал полёта: кто, что, когда
//! и с каким результатом. В отличие от audit (цепочка *решений* по одной
//! памяти), flight recorder — это *хроника операций* всей системы: создания
//! памяти, конфликтов, карантина, rehearsal, вызовов скиллов и MCP-инструментов.
//!
//! Журнал можно воспроизвести (replay): по entity_id строится полная цепочка
//! того, что происходило с сущностью, — «бортовой самописец» для отладки и
//! объяснения поведения системы.
//!
//! Типы и чистые функции в этом модуле не зависят от БД и тестируются
//! юнит-тестами; хранилище живёт в `storage/sqlite/flight_recorder_repository.rs`.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::core::domain_event::{DomainEvent, DomainEventType};
use crate::core::result::Result;

/// Категория записи — из какого подразделения экосистемы пришёл шаг.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FlightCategory {
    Memory,
    Conflict,
    Firewall,
    Rehearsal,
    Radar,
    Skill,
    Context,
    Team,
    Versioning,
    Mcp,
    System,
}

impl FlightCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Conflict => "conflict",
            Self::Firewall => "firewall",
            Self::Rehearsal => "rehearsal",
            Self::Radar => "radar",
            Self::Skill => "skill",
            Self::Context => "context",
            Self::Team => "team",
            Self::Versioning => "versioning",
            Self::Mcp => "mcp",
            Self::System => "system",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "memory" => Self::Memory,
            "conflict" => Self::Conflict,
            "firewall" => Self::Firewall,
            "rehearsal" => Self::Rehearsal,
            "radar" => Self::Radar,
            "skill" => Self::Skill,
            "context" => Self::Context,
            "team" => Self::Team,
            "versioning" => Self::Versioning,
            "mcp" => Self::Mcp,
            _ => Self::System,
        }
    }
}

/// Исход записи.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FlightOutcome {
    Success,
    Error,
    Blocked,
    Skipped,
}

impl FlightOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
            Self::Blocked => "blocked",
            Self::Skipped => "skipped",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "error" => Self::Error,
            "blocked" => Self::Blocked,
            "skipped" => Self::Skipped,
            _ => Self::Success,
        }
    }
}

/// Статус сессии полёта.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FlightSessionStatus {
    Active,
    Closed,
}

impl FlightSessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Closed => "closed",
        }
    }

    pub fn parse(s: &str) -> Self {
        if s == "closed" {
            Self::Closed
        } else {
            Self::Active
        }
    }
}

/// Сессия полёта: период активности с одной целью.
#[derive(Debug, Clone, Serialize)]
pub struct FlightSession {
    pub id: String,
    pub title: String,
    pub purpose: String,
    pub actor: String,
    pub source: String,
    pub status: FlightSessionStatus,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

impl FlightSession {
    pub fn new(title: &str, purpose: &str, actor: &str, source: &str) -> Self {
        Self {
            id: crate::core::entity_id::EntityId::new().to_string(),
            title: title.to_string(),
            purpose: purpose.to_string(),
            actor: actor.to_string(),
            source: source.to_string(),
            status: FlightSessionStatus::Active,
            started_at: Utc::now(),
            ended_at: None,
        }
    }
}

/// Одна запись журнала полёта — атомарный шаг с результатом.
#[derive(Debug, Clone, Serialize)]
pub struct FlightRecord {
    pub id: String,
    pub session_id: Option<String>,
    pub recorded_at: DateTime<Utc>,
    pub actor: String,
    pub category: FlightCategory,
    pub action: String,
    pub entity_type: String,
    pub entity_id: String,
    pub summary: String,
    pub details: serde_json::Value,
    pub duration_ms: i64,
    pub outcome: FlightOutcome,
}

impl FlightRecord {
    /// Создать запись с текущим временем.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: Option<String>,
        actor: &str,
        category: FlightCategory,
        action: &str,
        entity_type: &str,
        entity_id: &str,
        summary: &str,
        details: serde_json::Value,
        duration_ms: i64,
        outcome: FlightOutcome,
    ) -> Self {
        Self {
            id: crate::core::entity_id::EntityId::new().to_string(),
            session_id,
            recorded_at: Utc::now(),
            actor: actor.to_string(),
            category,
            action: action.to_string(),
            entity_type: entity_type.to_string(),
            entity_id: entity_id.to_string(),
            summary: summary.to_string(),
            details,
            duration_ms,
            outcome,
        }
    }

    /// Создать успешную запись (самый частый случай).
    #[allow(clippy::too_many_arguments)]
    pub fn success(
        session_id: Option<String>,
        actor: &str,
        category: FlightCategory,
        action: &str,
        entity_type: &str,
        entity_id: &str,
        summary: &str,
        details: serde_json::Value,
        duration_ms: i64,
    ) -> Self {
        Self::new(
            session_id,
            actor,
            category,
            action,
            entity_type,
            entity_id,
            summary,
            details,
            duration_ms,
            FlightOutcome::Success,
        )
    }
}

/// Агрегированная статистика по журналу — сколько чего было сделано.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FlightStats {
    pub total_records: u64,
    pub total_sessions: u64,
    pub active_sessions: u64,
    /// Счётчики по категориям: category.as_str() -> число записей.
    pub by_category: std::collections::BTreeMap<String, u64>,
    /// Счётчики по исходу: outcome.as_str() -> число записей.
    pub by_outcome: std::collections::BTreeMap<String, u64>,
}

/// Репозиторий журнала полёта.
#[async_trait::async_trait]
pub trait FlightRepository: Send + Sync {
    /// Открыть сессию полёта.
    async fn create_session(&self, session: &FlightSession) -> Result<()>;

    /// Закрыть активную сессию (проставить ended_at, статус closed).
    async fn close_session(&self, session_id: &str) -> Result<()>;

    /// Активные сессии (для «что сейчас происходит»).
    async fn list_active_sessions(&self, limit: u32) -> Result<Vec<FlightSession>>;

    /// Добавить запись в журнал полёта.
    async fn add_record(&self, record: &FlightRecord) -> Result<()>;

    /// Последние записи (лимит; опционально фильтр по категории).
    async fn recent_records(&self, limit: u32, category: Option<&str>)
    -> Result<Vec<FlightRecord>>;

    /// Все записи одной сессии, хронологически.
    async fn session_records(&self, session_id: &str) -> Result<Vec<FlightRecord>>;

    /// Полная цепочка записей по сущности — «воспроизведение полёта» сущности.
    async fn entity_replay(&self, entity_type: &str, entity_id: &str) -> Result<Vec<FlightRecord>>;

    /// Сводная статистика по всему журналу.
    async fn stats(&self) -> Result<FlightStats>;
}

/// Строит запись полёта из доменного события (мост event_bus → самописец).
///
/// Чистая функция: на входе событие, на выходе готовая запись. Сессию
/// проставляет вызывающий (обычно None — события автономны).
pub fn record_from_domain_event(event: &DomainEvent) -> FlightRecord {
    let (category, action, summary, entity_type, entity_id) =
        classify_domain_event(event.event_type.clone(), &event.payload);

    FlightRecord::success(
        None,
        event
            .metadata
            .get("actor")
            .map(|s| s.as_str())
            .unwrap_or("system"),
        category,
        &action,
        &entity_type,
        &entity_id,
        &summary,
        event.payload.clone(),
        0,
    )
}

/// Сопоставляет доменное событие категории/действию/сущности журнала полёта.
fn classify_domain_event(
    event_type: DomainEventType,
    payload: &serde_json::Value,
) -> (FlightCategory, String, String, String, String) {
    let entity_id = payload
        .get("record_id")
        .or_else(|| payload.get("entity_id"))
        .or_else(|| payload.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    match event_type {
        DomainEventType::MemoryRecordCreated => (
            FlightCategory::Memory,
            "create_memory".to_string(),
            "Memory record created".to_string(),
            "MemoryRecord".to_string(),
            entity_id,
        ),
        DomainEventType::MemoryRecordUpdated => (
            FlightCategory::Memory,
            "update_memory".to_string(),
            "Memory record updated".to_string(),
            "MemoryRecord".to_string(),
            entity_id,
        ),
        DomainEventType::EntityCreated => (
            FlightCategory::System,
            "entity_created".to_string(),
            "Entity created".to_string(),
            "Entity".to_string(),
            entity_id,
        ),
        DomainEventType::EntityUpdated => (
            FlightCategory::System,
            "entity_updated".to_string(),
            "Entity updated".to_string(),
            "Entity".to_string(),
            entity_id,
        ),
        DomainEventType::EntityDeleted => (
            FlightCategory::System,
            "entity_deleted".to_string(),
            "Entity deleted".to_string(),
            "Entity".to_string(),
            entity_id,
        ),
        DomainEventType::RelationshipCreated => (
            FlightCategory::Context,
            "relationship_created".to_string(),
            "Relationship created".to_string(),
            "Relationship".to_string(),
            entity_id,
        ),
        DomainEventType::RelationshipDeleted => (
            FlightCategory::Context,
            "relationship_deleted".to_string(),
            "Relationship deleted".to_string(),
            "Relationship".to_string(),
            entity_id,
        ),
        DomainEventType::ExecutionCompleted => (
            FlightCategory::System,
            "execution_completed".to_string(),
            "Execution completed".to_string(),
            "Execution".to_string(),
            entity_id,
        ),
        DomainEventType::DecisionMade => (
            FlightCategory::Conflict,
            "decision_made".to_string(),
            "Decision made".to_string(),
            "Decision".to_string(),
            entity_id,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::domain_event::DomainEvent;

    #[test]
    fn category_str_roundtrip() {
        let categories = [
            FlightCategory::Memory,
            FlightCategory::Conflict,
            FlightCategory::Firewall,
            FlightCategory::Rehearsal,
            FlightCategory::Radar,
            FlightCategory::Skill,
            FlightCategory::Context,
            FlightCategory::Team,
            FlightCategory::Versioning,
            FlightCategory::Mcp,
            FlightCategory::System,
        ];
        for c in categories {
            assert_eq!(FlightCategory::parse(c.as_str()), c);
        }
        assert_eq!(
            FlightCategory::parse("unknown-thing"),
            FlightCategory::System
        );
    }

    #[test]
    fn outcome_str_roundtrip() {
        let outcomes = [
            FlightOutcome::Success,
            FlightOutcome::Error,
            FlightOutcome::Blocked,
            FlightOutcome::Skipped,
        ];
        for o in outcomes {
            assert_eq!(FlightOutcome::parse(o.as_str()), o);
        }
    }

    #[test]
    fn session_status_parse() {
        assert_eq!(
            FlightSessionStatus::parse("active"),
            FlightSessionStatus::Active
        );
        assert_eq!(
            FlightSessionStatus::parse("closed"),
            FlightSessionStatus::Closed
        );
        assert_eq!(
            FlightSessionStatus::parse("garbage"),
            FlightSessionStatus::Active
        );
    }

    #[test]
    fn new_session_is_active_and_timestamped() {
        let s = FlightSession::new("Test run", "prove the recorder", "agent", "mcp");
        assert_eq!(s.status, FlightSessionStatus::Active);
        assert!(s.ended_at.is_none());
        assert!(!s.id.is_empty());
        assert_eq!(s.actor, "agent");
        assert_eq!(s.source, "mcp");
    }

    #[test]
    fn record_success_defaults() {
        let r = FlightRecord::success(
            None,
            "user",
            FlightCategory::Firewall,
            "quarantine",
            "MemoryRecord",
            "mem-1",
            "Content quarantined",
            serde_json::json!({"reasons": ["pii"]}),
            12,
        );
        assert_eq!(r.outcome, FlightOutcome::Success);
        assert_eq!(r.category, FlightCategory::Firewall);
        assert_eq!(r.entity_id, "mem-1");
        assert_eq!(r.duration_ms, 12);
        assert_eq!(r.details["reasons"][0], "pii");
    }

    #[test]
    fn record_from_memory_created_event() {
        let event = DomainEvent::new(
            DomainEventType::MemoryRecordCreated,
            serde_json::json!({"record_id": "mem-abc"}),
        );
        let record = record_from_domain_event(&event);
        assert_eq!(record.category, FlightCategory::Memory);
        assert_eq!(record.action, "create_memory");
        assert_eq!(record.entity_type, "MemoryRecord");
        assert_eq!(record.entity_id, "mem-abc");
        assert_eq!(record.outcome, FlightOutcome::Success);
        assert_eq!(record.session_id, None);
    }

    #[test]
    fn record_from_decision_event() {
        let event = DomainEvent::new(
            DomainEventType::DecisionMade,
            serde_json::json!({"entity_id": "grp-7"}),
        );
        let record = record_from_domain_event(&event);
        assert_eq!(record.category, FlightCategory::Conflict);
        assert_eq!(record.action, "decision_made");
        assert_eq!(record.entity_id, "grp-7");
    }

    #[test]
    fn record_carries_actor_metadata() {
        let event = DomainEvent::new(DomainEventType::EntityCreated, serde_json::json!({}));
        let record = record_from_domain_event(&event);
        assert_eq!(record.actor, "system");
    }

    #[test]
    fn flight_stats_default_is_empty() {
        let stats = FlightStats::default();
        assert_eq!(stats.total_records, 0);
        assert!(stats.by_category.is_empty());
        assert!(stats.by_outcome.is_empty());
    }
}
