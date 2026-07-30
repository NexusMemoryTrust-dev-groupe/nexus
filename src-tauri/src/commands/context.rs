use serde::{Deserialize, Serialize};

use crate::core::context::context_package::ContextPackage;
use crate::core::context::context_builder::{ContextBuilder, ContextBuilderImpl};
use crate::core::context::context_request::ContextRequest;
use crate::core::graph::entity::Entity;
use crate::core::graph::relationship::Relationship;
use crate::core::memory::memory_record::MemoryRecord;

/// Serializable context package for Tauri IPC.
#[derive(Serialize, Deserialize)]
pub struct ContextDto {
    pub id: String,
    pub entities: Vec<EntityDto>,
    pub relationships: Vec<RelationshipDto>,
    pub memory_records: Vec<MemoryRecordDto>,
    pub user_intent: IntentDto,
    pub created_at: String,
    pub token_count: u32,
}

#[derive(Serialize, Deserialize)]
pub struct EntityDto {
    pub id: String,
    pub entity_type: String,
    pub title: String,
    pub description: String,
}

#[derive(Serialize, Deserialize)]
pub struct RelationshipDto {
    pub id: String,
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub relationship_type: String,
    pub weight: f64,
}

#[derive(Serialize, Deserialize)]
pub struct MemoryRecordDto {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub content: String,
    pub layer: String,
}

#[derive(Serialize, Deserialize)]
pub struct IntentDto {
    pub query: String,
    pub intent_type: String,
    pub confidence: f64,
}

impl From<ContextPackage> for ContextDto {
    fn from(pkg: ContextPackage) -> Self {
        Self {
            id: pkg.id,
            entities: pkg.entities.into_iter().map(EntityDto::from).collect(),
            relationships: pkg.relationships.into_iter().map(RelationshipDto::from).collect(),
            memory_records: pkg.memory_records.into_iter().map(MemoryRecordDto::from).collect(),
            user_intent: IntentDto {
                query: pkg.user_intent.query,
                intent_type: format!("{:?}", pkg.user_intent.intent_type),
                confidence: pkg.user_intent.confidence,
            },
            created_at: pkg.created_at.to_rfc3339(),
            token_count: pkg.token_count,
        }
    }
}

impl From<Entity> for EntityDto {
    fn from(e: Entity) -> Self {
        Self {
            id: e.id.as_str().to_string(),
            entity_type: e.entity_type.as_str().to_string(),
            title: e.title,
            description: e.description,
        }
    }
}

impl From<Relationship> for RelationshipDto {
    fn from(r: Relationship) -> Self {
        Self {
            id: r.id.as_str().to_string(),
            source_entity_id: r.source_entity_id.as_str().to_string(),
            target_entity_id: r.target_entity_id.as_str().to_string(),
            relationship_type: r.relationship_type.as_str().to_string(),
            weight: r.weight,
        }
    }
}

impl From<MemoryRecord> for MemoryRecordDto {
    fn from(r: MemoryRecord) -> Self {
        Self {
            id: r.id.as_str().to_string(),
            title: r.title,
            summary: r.summary,
            content: r.content,
            layer: format!("{:?}", r.layer),
        }
    }
}

/// Build a context package for a query using the full M4 pipeline.
#[tauri::command]
pub async fn build_context(query: String) -> Result<ContextDto, String> {
    let mem_conn = crate::db::open_connection()?;
    let graph_conn = crate::db::open_connection()?;

    let memory_repo = crate::storage::sqlite::SqliteMemoryRepository::new(mem_conn)
        .map_err(|e| e.to_string())?;
    let graph_repo = crate::storage::sqlite::SqliteGraphRepository::new(graph_conn)
        .map_err(|e| e.to_string())?;

    // Use the full ContextBuilderImpl pipeline:
    // Intent Detection → Graph Seeding → Expansion → Memory Injection → Ranking → Compression
    let builder = ContextBuilderImpl::new(graph_repo, memory_repo);
    let request = ContextRequest {
        query,
        ..Default::default()
    };

    let pkg = builder.build(&request).await.map_err(|e| e.to_string())?;

    Ok(ContextDto::from(pkg))
}
