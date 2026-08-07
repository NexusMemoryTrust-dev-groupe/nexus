use serde::{Deserialize, Serialize};

use crate::core::context::context_builder::{ContextBuilder, ContextBuilderImpl};
use crate::core::context::context_cache::global_cache;
use crate::core::context::context_package::ContextPackage;
use crate::core::context::context_request::ContextRequest;
use crate::core::context::context_service::ContextService;
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

    // ── Auditability ──
    //
    // `provenance` answers "why is this in my context?" for every item the
    // pipeline touched, including the ones it discarded and the reason. Without
    // it the engine is a black box: the user sees a result and has to trust it.
    //
    // `baseline_tokens` is what the same material would have cost the model read
    // in full, so the saving shown next to it is a measurement rather than a
    // claim.
    pub provenance: crate::core::context::provenance::Provenance,
    pub baseline_tokens: u32,
    pub candidate_entities: u32,
    pub candidate_memories: u32,
    /// `"exact"` when counted with the real BPE vocabulary, `"estimated"` otherwise.
    pub token_method: String,
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
            relationships: pkg
                .relationships
                .into_iter()
                .map(RelationshipDto::from)
                .collect(),
            memory_records: pkg
                .memory_records
                .into_iter()
                .map(MemoryRecordDto::from)
                .collect(),
            user_intent: IntentDto {
                query: pkg.user_intent.query,
                intent_type: format!("{:?}", pkg.user_intent.intent_type),
                confidence: pkg.user_intent.confidence,
            },
            created_at: pkg.created_at.to_rfc3339(),
            token_count: pkg.token_count,
            provenance: pkg.provenance,
            baseline_tokens: pkg.baseline_tokens,
            candidate_entities: pkg.candidate_entities,
            candidate_memories: pkg.candidate_memories,
            token_method: crate::core::tokenizer::method().as_str().to_string(),
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

/// Build a context package for a query using the full M4 pipeline with caching.
#[tauri::command]
pub async fn build_context(query: String) -> Result<ContextDto, String> {
    let mem_conn = crate::db::open_connection()?;
    let graph_conn = crate::db::open_connection()?;
    let snapshot_conn = crate::db::open_connection()?;

    let memory_repo =
        crate::storage::sqlite::SqliteMemoryRepository::new(mem_conn).map_err(|e| e.to_string())?;
    let graph_repo = crate::storage::sqlite::SqliteGraphRepository::new(graph_conn)
        .map_err(|e| e.to_string())?;
    let snapshot_repo =
        crate::storage::sqlite::context_repository::SqliteContextRepository::new(snapshot_conn)
            .map_err(|e| e.to_string())?;

    let builder = ContextBuilderImpl::new(graph_repo, memory_repo);
    let cache = global_cache();
    let service = ContextService::new(builder, cache, snapshot_repo);

    let request = ContextRequest {
        query: query.clone(),
        ..Default::default()
    };

    let start = std::time::Instant::now();
    let pkg = service
        .build_context(&request)
        .await
        .map_err(|e| e.to_string())?;

    // Record savings for this interaction, including measured latency.
    let mut measurement = crate::commands::savings::SavingsMeasurement::from_package(&pkg);
    measurement.latency_ms = start.elapsed().as_millis() as u32;
    crate::commands::savings::record_savings(
        &measurement,
        &query,
        &format!("{:?}", pkg.user_intent.intent_type),
    );

    Ok(ContextDto::from(pkg))
}

/// A rendered context package, ready to paste into any model.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportDto {
    pub content: String,
    pub format: String,
    pub tokens: u32,
    pub token_method: String,
    pub filename: String,
}

/// Render a context package for a model outside OpenCode.
///
/// Without this, the context engine is only useful to whoever wires up our MCP
/// server. Exporting the same package as Markdown, JSON, or bare text means the
/// work Nexus does is portable: paste it into ChatGPT, Claude, a local model, or
/// feed the JSON to another program.
///
/// `format` accepts `markdown`, `json`, or `plain`.
#[tauri::command]
pub async fn export_context(query: String, format: Option<String>) -> Result<ExportDto, String> {
    use crate::core::context::export::{self, ExportFormat};

    let fmt = match format {
        Some(f) => ExportFormat::parse(&f).map_err(|e| e.to_string())?,
        None => ExportFormat::Markdown,
    };

    let mem_conn = crate::db::open_connection()?;
    let graph_conn = crate::db::open_connection()?;

    let memory_repo =
        crate::storage::sqlite::SqliteMemoryRepository::new(mem_conn).map_err(|e| e.to_string())?;
    let graph_repo = crate::storage::sqlite::SqliteGraphRepository::new(graph_conn)
        .map_err(|e| e.to_string())?;

    let builder = ContextBuilderImpl::new(graph_repo, memory_repo);
    let request = ContextRequest {
        query: query.clone(),
        ..Default::default()
    };
    let pkg = builder.build(&request).await.map_err(|e| e.to_string())?;

    let rendered = export::export(&pkg, fmt).map_err(|e| e.to_string())?;

    Ok(ExportDto {
        content: rendered.content,
        format: rendered.format.extension().to_string(),
        tokens: rendered.tokens,
        token_method: rendered.token_method,
        filename: rendered.filename,
    })
}

/// Build a context package centered on a specific entity with configurable depth.
#[tauri::command]
pub async fn build_context_for_entity(
    entity_id: String,
    depth: Option<u32>,
) -> Result<ContextDto, String> {
    let eid = crate::core::entity_id::EntityId::parse(&entity_id).map_err(|e| e.to_string())?;
    let depth = depth.unwrap_or(2);

    let mem_conn = crate::db::open_connection()?;
    let graph_conn = crate::db::open_connection()?;

    let memory_repo =
        crate::storage::sqlite::SqliteMemoryRepository::new(mem_conn).map_err(|e| e.to_string())?;
    let graph_repo = crate::storage::sqlite::SqliteGraphRepository::new(graph_conn)
        .map_err(|e| e.to_string())?;

    let builder = ContextBuilderImpl::new(graph_repo, memory_repo);
    let start = std::time::Instant::now();
    let pkg = builder
        .build_for_entity(&eid, depth)
        .await
        .map_err(|e| e.to_string())?;

    // Record savings for this entity context build, including measured latency.
    let mut measurement = crate::commands::savings::SavingsMeasurement::from_package(&pkg);
    measurement.latency_ms = start.elapsed().as_millis() as u32;
    crate::commands::savings::record_savings(
        &measurement,
        &format!("entity:{}", entity_id),
        "EntityContext",
    );

    Ok(ContextDto::from(pkg))
}
