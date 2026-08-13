use serde::{Deserialize, Serialize};

use crate::core::entity_id::EntityId;
use crate::core::memory::memory_record::MemoryRecord;
use crate::core::memory::memory_repository::MemoryRepository;
use crate::core::memory::types::{MemoryFeedback, MemorySource};

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
    // Memory Trust lifecycle (V12)
    pub memory_state: String,
    pub supersedes_id: Option<String>,
    pub superseded_by_id: Option<String>,
    pub confirmed_at: Option<String>,
    pub confirmed_by: Option<String>,
    pub expires_at: Option<String>,
    pub feedback: MemoryFeedback,
    // Cognitive layer provenance (V18)
    pub layer_confidence: f64,
    pub layer_reason: String,
    pub layer_updated_at: Option<String>,
    pub layer_history: Vec<LayerHistoryDto>,
}

#[derive(Serialize, Deserialize)]
pub struct LayerHistoryDto {
    pub layer: String,
    pub confidence: f64,
    pub reason: String,
    pub at: String,
    pub by: String,
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
            linked_entity_ids: r
                .linked_entity_ids
                .iter()
                .map(|id| id.as_str().to_string())
                .collect(),
            latest_version_id: r.latest_version_id,
            status: format!("{:?}", r.status),
            layer: format!("{:?}", r.layer),
            attached_files: r
                .attached_files
                .into_iter()
                .map(|f| AttachedFileDto {
                    name: f.name,
                    path: f.path,
                    size_bytes: f.size_bytes,
                    mime_type: f.mime_type,
                })
                .collect(),
            memory_state: r.memory_state.as_str().to_string(),
            supersedes_id: r.supersedes_id,
            superseded_by_id: r.superseded_by_id,
            confirmed_at: r.confirmed_at.map(|dt| dt.to_rfc3339()),
            confirmed_by: r.confirmed_by,
            expires_at: r.expires_at.map(|dt| dt.to_rfc3339()),
            feedback: r.feedback,
            layer_confidence: r.layer_confidence,
            layer_reason: r.layer_reason,
            layer_updated_at: r.layer_updated_at.map(|dt| dt.to_rfc3339()),
            layer_history: r
                .layer_history
                .into_iter()
                .map(|e| LayerHistoryDto {
                    layer: e.layer.as_str().to_string(),
                    confidence: e.confidence,
                    reason: e.reason,
                    at: e.at,
                    by: e.by.as_str().to_string(),
                })
                .collect(),
        }
    }
}

fn open_repo() -> Result<crate::storage::sqlite::SqliteMemoryRepository, String> {
    let conn = crate::db::open_connection()?;
    crate::storage::sqlite::SqliteMemoryRepository::new(conn).map_err(|e| e.to_string())
}

/// After the conflict detector flags both sides of a contradiction, reconcile
/// the conflict groups table so the pair becomes a resolvable open group.
/// Best-effort: failures are logged, never fatal for the create/update call.
async fn reconcile_conflict_groups_after_detect() {
    if let Err(e) = crate::commands::conflict::sync_conflict_groups().await {
        eprintln!("[nexus] conflict group reconciliation failed: {}", e);
    }
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
    let record = repo
        .get_by_id(&entity_id)
        .await
        .map_err(|e| e.to_string())?;
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
    let mut record = MemoryRecord::new(
        title,
        content,
        author.unwrap_or_else(|| "user".to_string()),
        MemorySource::Manual,
    )
    .map_err(|e| e.to_string())?;
    crate::core::memory::memory_lifecycle::auto_classify(&mut record);

    // Memory Firewall (System 4): every ingress path must be screened before
    // the content enters the store. Block → Err; Quarantine → content is parked
    // in the quarantine table and the caller learns the id.
    crate::commands::firewall::screen_ingress(
        &record.title,
        &record.content,
        &record.author,
        "Manual",
    )
    .await?;

    let _id = repo.save(&record).await.map_err(|e| e.to_string())?;

    // Memory Trust: check the new memory against the existing pool. If it says
    // something different about the same topic, both sides are flagged
    // Conflicted and the trust UI can ask the user to decide.
    crate::core::memory::memory_lifecycle::detect_and_mark_conflicts(&repo, &record)
        .await
        .map_err(|e| e.to_string())?;

    // The detector may have created contradictions вЂ” turn them into resolvable
    // open conflict groups so the Conflicts view / MCP can surface them.
    reconcile_conflict_groups_after_detect().await;

    // The conflict detector may have demoted this record to Conflicted in the
    // database. Re-read it so the returned DTO reflects the persisted state
    // instead of the pre-check struct (which would claim Current wrongly).
    let saved = repo
        .get_by_id(&record.id)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or(record);

    // Index for semantic search off-thread: fingerprints used to be written only
    // when someone called the MCP tool by hand, so semantic search stayed empty.
    crate::core::context::indexer::spawn_index_memory(
        &saved.id,
        &saved.title,
        &saved.summary,
        &saved.content,
    );
    Ok(MemoryDto::from(saved))
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
    crate::core::memory::memory_lifecycle::auto_classify(&mut record);

    // Memory Firewall (System 4): screen every ingress path.
    crate::commands::firewall::screen_ingress(
        &record.title,
        &record.content,
        &record.author,
        "Manual",
    )
    .await?;

    let _id = repo.save(&record).await.map_err(|e| e.to_string())?;

    crate::core::context::indexer::spawn_index_memory(
        &record.id,
        &record.title,
        &record.summary,
        &record.content,
    );

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
    // The edited text may change what the memory *is* вЂ” re-classify unless the
    // user pinned the layer explicitly.
    crate::core::memory::memory_lifecycle::auto_classify(&mut record);
    // Must be update(), not save(): save() issues an INSERT and fails with a
    // UNIQUE constraint violation on the existing primary key.
    repo.update(&record).await.map_err(|e| e.to_string())?;

    // Memory Trust: an edited memory may now contradict something else in the
    // pool вЂ” re-run the conflict check.
    crate::core::memory::memory_lifecycle::detect_and_mark_conflicts(&repo, &record)
        .await
        .map_err(|e| e.to_string())?;

    // Reconcile conflict groups for any newly flagged contradiction.
    reconcile_conflict_groups_after_detect().await;

    // Re-index: the stored embedding describes the *old* text, so leaving it in
    // place makes semantic search return this memory for queries that no longer
    // match its content.
    crate::core::context::indexer::spawn_index_memory(
        &record.id,
        &record.title,
        &record.summary,
        &record.content,
    );

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
    links_repo
        .delete_links_for_memory(&entity_id)
        .await
        .map_err(|e| e.to_string())?;

    let repo = open_repo()?;
    repo.delete(&entity_id).await.map_err(|e| e.to_string())?;

    // Drop the embedding too, otherwise semantic search keeps scoring a memory
    // that no longer exists and callers get IDs that resolve to nothing.
    crate::core::context::indexer::spawn_forget_memory(&entity_id);

    Ok(())
}

/// Explicitly set a memory's cognitive layer (user choice). Records provenance.
#[tauri::command]
pub async fn set_memory_layer(
    id: String,
    layer: String,
    reason: Option<String>,
) -> Result<MemoryDto, String> {
    let entity_id = EntityId::parse(&id).map_err(|e| e.to_string())?;
    let parsed_layer = crate::core::memory::types::MemoryLayer::parse(&layer);
    let repo = open_repo()?;
    let mut record = repo
        .get_by_id(&entity_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Memory {} not found", id))?;

    record.layer = parsed_layer.clone();
    record.layer_confidence = 1.0;
    record.layer_reason = reason.unwrap_or_else(|| "user-assigned layer".to_string());
    record.layer_updated_at = Some(chrono::Utc::now());
    record
        .layer_history
        .push(crate::core::memory::types::LayerHistoryEntry {
            layer: parsed_layer,
            confidence: 1.0,
            reason: record.layer_reason.clone(),
            at: chrono::Utc::now().to_rfc3339(),
            by: crate::core::memory::types::LayerAssignment::User,
        });
    record.touch();
    repo.update(&record).await.map_err(|e| e.to_string())?;
    Ok(MemoryDto::from(record))
}

/// Re-run the signature classifier on a memory and persist the result.
#[tauri::command]
pub async fn reclassify_memory(id: String) -> Result<MemoryDto, String> {
    let entity_id = EntityId::parse(&id).map_err(|e| e.to_string())?;
    let repo = open_repo()?;
    let mut record = repo
        .get_by_id(&entity_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Memory {} not found", id))?;

    let pinned_by_user = record
        .layer_history
        .last()
        .map(|e| e.by == crate::core::memory::types::LayerAssignment::User)
        .unwrap_or(false);
    if !pinned_by_user {
        let classification = crate::core::memory::layer::LayerClassifier::classify(
            &record.title,
            &record.content,
            record.source.clone(),
            record.memory_state.clone(),
            record.importance_score,
        );
        record.layer = classification.layer;
        record.layer_confidence = classification.confidence;
        record.layer_reason = classification.reason;
        record.layer_updated_at = Some(chrono::Utc::now());
    }
    record.touch();
    repo.update(&record).await.map_err(|e| e.to_string())?;
    Ok(MemoryDto::from(record))
}

/// Full layer history of a memory, newest first.
#[tauri::command]
pub async fn get_layer_history(id: String) -> Result<Vec<LayerHistoryDto>, String> {
    let entity_id = EntityId::parse(&id).map_err(|e| e.to_string())?;
    let repo = open_repo()?;
    let record = repo
        .get_by_id(&entity_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Memory {} not found", id))?;
    let mut history = record.layer_history;
    crate::core::memory::types::LayerHistoryEntry::sort_newest_first(&mut history);
    Ok(history
        .into_iter()
        .map(|e| LayerHistoryDto {
            layer: e.layer.as_str().to_string(),
            confidence: e.confidence,
            reason: e.reason,
            at: e.at,
            by: e.by.as_str().to_string(),
        })
        .collect())
}

/// Distribution of cognitive layers across the memory pool.
#[tauri::command]
pub async fn get_layer_stats() -> Result<Vec<crate::core::memory::memory_service::LayerStat>, String>
{
    let repo = open_repo()?;
    let records = repo.list(10_000, 0).await.map_err(|e| e.to_string())?;
    let mut by_layer: std::collections::HashMap<String, (u64, f64)> =
        std::collections::HashMap::new();
    for r in &records {
        let entry = by_layer
            .entry(r.layer.as_str().to_string())
            .or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += r.layer_confidence;
    }
    let mut stats: Vec<crate::core::memory::memory_service::LayerStat> = by_layer
        .into_iter()
        .map(
            |(layer, (count, conf_sum))| crate::core::memory::memory_service::LayerStat {
                layer,
                count,
                mean_confidence: if count == 0 {
                    0.0
                } else {
                    conf_sum / count as f64
                },
            },
        )
        .collect();
    stats.sort_by_key(|s| std::cmp::Reverse(s.count));
    Ok(stats)
}

/// Nexus Memory Score — панель здоровья памяти проекта (Knowledge Nav 2.0).
/// Считает покрытие, свежесть, согласованность, доверие, избыточность,
/// конфликтность и зрелость знаний; итоговое здоровье 0–100%.
#[tauri::command]
pub async fn get_memory_score() -> Result<crate::core::memory::memory_score::MemoryScore, String> {
    use crate::core::memory::memory_score::compute_score;

    let repo = open_repo()?;
    let records = repo.list(100_000, 0).await.map_err(|e| e.to_string())?;

    // Число сущностей графа — знаменатель для coverage.
    let entities_total = crate::db::open_connection()
        .ok()
        .and_then(|conn| {
            conn.query_row("SELECT COUNT(*) FROM graph_entities", [], |row| {
                row.get::<_, i64>(0)
            })
            .ok()
        })
        .unwrap_or(0)
        .max(0) as u32;

    Ok(compute_score(&records, entities_total))
}
