use serde::{Deserialize, Serialize};

use crate::core::entity_id::EntityId;
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::memory::types::MemorySource;

/// Serializable memory record for Tauri IPC.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryDto {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
    pub author: String,
    pub source: String,
    pub confidence_score: f64,
    pub importance_score: f64,
    pub visibility: String,
    pub capture_mode: String,
    pub project_space_id: Option<String>,
    pub linked_entity_ids: Vec<String>,
    pub latest_version_id: Option<String>,
    pub status: String,
    pub layer: String,
    pub attached_files: Vec<AttachedFileDto>,
}

#[derive(Serialize, Deserialize)]
pub struct AttachedFileDto {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub mime_type: String,
}

impl From<MemoryRecord> for MemoryDto {
    fn from(r: MemoryRecord) -> Self {
        Self {
            id: r.id.as_str().to_string(),
            title: r.title,
            summary: r.summary,
            content: r.content,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
            author: r.author,
            source: match r.source {
                MemorySource::Manual => "Manual",
                MemorySource::Git => "Git",
                MemorySource::Telegram => "Telegram",
                MemorySource::Email => "Email",
                MemorySource::Meeting => "Meeting",
                MemorySource::Document => "Document",
                MemorySource::AiGenerated => "AiGenerated",
                MemorySource::Compressed => "Compressed",
            }
            .to_string(),
            confidence_score: r.confidence_score,
            importance_score: r.importance_score,
            visibility: format!("{:?}", r.visibility),
            capture_mode: format!("{:?}", r.capture_mode),
            project_space_id: r.project_space_id.map(|id| id.as_str().to_string()),
            linked_entity_ids: r.linked_entity_ids.iter().map(|id| id.as_str().to_string()).collect(),
            latest_version_id: r.latest_version_id,
            status: format!("{:?}", r.status),
            layer: format!("{:?}", r.layer),
            attached_files: r.attached_files.into_iter().map(|f| AttachedFileDto {
                name: f.name,
                path: f.path,
                size_bytes: f.size_bytes,
                mime_type: f.mime_type,
            }).collect(),
        }
    }
}

fn open_repo() -> Result<crate::storage::sqlite::SqliteMemoryRepository, String> {
    let conn = crate::db::open_connection()?;
    crate::storage::sqlite::SqliteMemoryRepository::new(conn).map_err(|e| e.to_string())
}

/// Get all memory records.
#[tauri::command]
pub async fn get_memories() -> Result<Vec<MemoryDto>, String> {
    let repo = open_repo()?;
    let records = repo.list(100, 0).await.map_err(|e| e.to_string())?;
    Ok(records.into_iter().map(MemoryDto::from).collect())
}

/// Get a single memory record by ID.
#[tauri::command]
pub async fn get_memory(id: String) -> Result<Option<MemoryDto>, String> {
    let entity_id = EntityId::parse(&id).map_err(|e| e.to_string())?;
    let repo = open_repo()?;
    let record = repo.get_by_id(&entity_id).await.map_err(|e| e.to_string())?;
    Ok(record.map(MemoryDto::from))
}

/// Create a new memory record.
#[tauri::command]
pub async fn create_memory(
    title: String,
    content: String,
    author: Option<String>,
) -> Result<MemoryDto, String> {
    let repo = open_repo()?;
    let record = MemoryRecord::new(
        title,
        content,
        author.unwrap_or_else(|| "user".to_string()),
        MemorySource::Manual,
    )
    .map_err(|e| e.to_string())?;
    let _id = repo.save(&record).await.map_err(|e| e.to_string())?;
    Ok(MemoryDto::from(record))
}

/// Search memory records by query string.
#[tauri::command]
pub async fn search_memories(query: String) -> Result<Vec<MemoryDto>, String> {
    let repo = open_repo()?;
    let records = repo.search(&query).await.map_err(|e| e.to_string())?;
    Ok(records.into_iter().map(MemoryDto::from).collect())
}

/// Get memories linked to a specific project.
#[tauri::command]
pub async fn get_project_memories(project_id: String) -> Result<Vec<MemoryDto>, String> {
    let repo = open_repo()?;
    let entity_id = EntityId::parse(&project_id).map_err(|e| e.to_string())?;
    let records = repo
        .get_by_project(&entity_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(records.into_iter().map(MemoryDto::from).collect())
}

/// Create a memory record linked to a project.
#[tauri::command]
pub async fn create_project_memory(
    project_id: String,
    title: String,
    content: String,
    author: Option<String>,
) -> Result<MemoryDto, String> {
    let repo = open_repo()?;
    let entity_id = EntityId::parse(&project_id).map_err(|e| e.to_string())?;
    let mut record = MemoryRecord::new(
        title,
        content,
        author.unwrap_or_else(|| "user".to_string()),
        MemorySource::Manual,
    )
    .map_err(|e| e.to_string())?;
    record.project_space_id = Some(entity_id);
    let _id = repo.save(&record).await.map_err(|e| e.to_string())?;
    Ok(MemoryDto::from(record))
}

/// Update an existing memory record.
#[tauri::command]
pub async fn update_memory(
    id: String,
    title: Option<String>,
    content: Option<String>,
    summary: Option<String>,
) -> Result<MemoryDto, String> {
    let repo = open_repo()?;
    let entity_id = EntityId::parse(&id).map_err(|e| e.to_string())?;
    let mut record = repo
        .get_by_id(&entity_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Memory {} not found", id))?;

    if let Some(t) = title {
        record.title = t;
    }
    if let Some(c) = content {
        record.content = c;
    }
    if let Some(s) = summary {
        record.summary = s;
    }
    record.touch();
    repo.save(&record).await.map_err(|e| e.to_string())?;
    Ok(MemoryDto::from(record))
}

/// Delete a memory record by ID.
/// Also cleans up memory_entity_links to prevent orphaned data.
#[tauri::command]
pub async fn delete_memory(id: String) -> Result<(), String> {
    let entity_id = EntityId::parse(&id).map_err(|e| e.to_string())?;

    // Clean up memory_entity_links first
    let links_conn = crate::db::open_connection().map_err(|e| e.to_string())?;
    let links_repo = crate::storage::sqlite::SqliteMemoryEntityLinkRepository::new(links_conn)
        .map_err(|e| e.to_string())?;
    use crate::storage::sqlite::memory_entity_links_repository::MemoryEntityLinkRepository;
    links_repo.delete_links_for_memory(&entity_id).await.map_err(|e| e.to_string())?;

    let repo = open_repo()?;
    repo.delete(&entity_id).await.map_err(|e| e.to_string())
}
