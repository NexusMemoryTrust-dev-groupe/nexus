use serde::{Deserialize, Serialize};

use crate::core::entity_id::EntityId;
use crate::core::graph::entity::Entity;
use crate::core::graph::entity_types::EntityType;
use crate::core::graph::graph_store::GraphStore;
use crate::core::graph::relationship::Relationship;

/// Serializable graph data for Tauri IPC.
#[derive(Serialize, Deserialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub entity_type: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub id: String,
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub relationship_type: String,
    pub weight: f64,
    pub created_at: String,
}

impl From<Entity> for GraphNode {
    fn from(e: Entity) -> Self {
        Self {
            id: e.id.as_str().to_string(),
            entity_type: e.entity_type.as_str().to_string(),
            title: e.title,
            description: e.description,
            status: format!("{:?}", e.status),
            created_at: e.created_at.to_rfc3339(),
            updated_at: e.updated_at.to_rfc3339(),
        }
    }
}

impl From<Relationship> for GraphEdge {
    fn from(r: Relationship) -> Self {
        Self {
            id: r.id.as_str().to_string(),
            source_entity_id: r.source_entity_id.as_str().to_string(),
            target_entity_id: r.target_entity_id.as_str().to_string(),
            relationship_type: r.relationship_type.as_str().to_string(),
            weight: r.weight,
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

fn open_repo() -> Result<crate::storage::sqlite::SqliteGraphRepository, String> {
    let conn = crate::db::open_connection()?;
    crate::storage::sqlite::SqliteGraphRepository::new(conn).map_err(|e| e.to_string())
}

/// Get the full graph (all entities and relationships).
#[tauri::command]
pub async fn get_graph() -> Result<GraphData, String> {
    let repo = open_repo()?;

    // Get all entities across all types
    let mut nodes: Vec<GraphNode> = Vec::new();
    for entity_type in [
        EntityType::Person,
        EntityType::Organization,
        EntityType::Project,
        EntityType::Document,
        EntityType::Meeting,
        EntityType::Decision,
        EntityType::Task,
        EntityType::Technology,
        EntityType::Memory,
    ] {
        let entities = repo
            .get_entities_by_type(&entity_type)
            .await
            .map_err(|e| e.to_string())?;
        nodes.extend(entities.into_iter().map(GraphNode::from));
    }

    // Get all relationships
    let mut edges = Vec::new();
    for node in &nodes {
        if let Ok(entity_id) = EntityId::parse(&node.id) {
            if let Ok(rels) = repo.get_entity_relationships(&entity_id).await {
                edges.extend(rels.into_iter().map(GraphEdge::from));
            }
        }
    }

    Ok(GraphData { nodes, edges })
}

/// Get a single entity by ID.
#[tauri::command]
pub async fn get_entity(id: String) -> Result<Option<GraphNode>, String> {
    let entity_id = EntityId::parse(&id).map_err(|e| e.to_string())?;
    let repo = open_repo()?;
    let entity = repo.get_entity(&entity_id).await.map_err(|e| e.to_string())?;
    Ok(entity.map(GraphNode::from))
}

/// Create a new entity in the graph.
#[tauri::command]
pub async fn create_entity(
    entity_type: String,
    title: String,
    description: String,
) -> Result<GraphNode, String> {
    let repo = open_repo()?;
    let et = EntityType::from_str(&entity_type);
    let entity = Entity::new(et, title, description);
    entity.validate().map_err(|e| e.to_string())?;
    let _id = repo.add_entity(&entity).await.map_err(|e| e.to_string())?;
    Ok(GraphNode::from(entity))
}

/// Get all projects (entities with type Project).
#[tauri::command]
pub async fn get_projects() -> Result<Vec<GraphNode>, String> {
    let repo = open_repo()?;
    let entities = repo
        .get_entities_by_type(&EntityType::Project)
        .await
        .map_err(|e| e.to_string())?;
    Ok(entities.into_iter().map(GraphNode::from).collect())
}

/// Get all entities linked to a project via relationships (direction: project → entity).
#[tauri::command]
pub async fn get_project_entities(project_id: String) -> Result<GraphData, String> {
    let repo = open_repo()?;
    let project_entity_id = EntityId::parse(&project_id).map_err(|e| e.to_string())?;

    // Verify the project exists
    let _project = repo
        .get_entity(&project_entity_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project {} not found", project_id))?;

    // Get all relationships involving this project
    let rels = repo
        .get_entity_relationships(&project_entity_id)
        .await
        .map_err(|e| e.to_string())?;

    // Collect unique entity IDs from the other end of relationships
    let mut entity_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for rel in &rels {
        let other_id = if rel.source_entity_id == project_entity_id {
            &rel.target_entity_id
        } else {
            &rel.source_entity_id
        };
        entity_ids.insert(other_id.as_str().to_string());
    }

    // Fetch all linked entities
    let mut nodes: Vec<GraphNode> = Vec::new();
    for eid_str in &entity_ids {
        if let Ok(eid) = EntityId::parse(eid_str) {
            if let Ok(Some(entity)) = repo.get_entity(&eid).await {
                nodes.push(GraphNode::from(entity));
            }
        }
    }

    // All relationships for the response
    let edges: Vec<GraphEdge> = rels.into_iter().map(GraphEdge::from).collect();

    Ok(GraphData { nodes, edges })
}

/// Link an entity to a project (creates a relationship: project --Uses--> entity).
#[tauri::command]
pub async fn link_entity_to_project(
    project_id: String,
    entity_id: String,
    relationship_type: Option<String>,
    weight: Option<f64>,
) -> Result<GraphEdge, String> {
    use crate::core::graph::relationship::Relationship;
    use crate::core::graph::relationship_types::RelationshipType;

    let repo = open_repo()?;
    let project_eid = EntityId::parse(&project_id).map_err(|e| e.to_string())?;
    let entity_eid = EntityId::parse(&entity_id).map_err(|e| e.to_string())?;

    // Verify both entities exist
    repo.get_entity(&project_eid)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project {} not found", project_id))?;
    repo.get_entity(&entity_eid)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Entity {} not found", entity_id))?;

    let rel_type = relationship_type
        .map(|s| RelationshipType::from_str(&s))
        .unwrap_or(RelationshipType::Uses);
    let w = weight.unwrap_or(0.8);

    let rel = Relationship::new(project_eid, entity_eid, rel_type, w).map_err(|e| e.to_string())?;
    let _rel_id = repo.add_relationship(&rel).await.map_err(|e| e.to_string())?;
    Ok(GraphEdge::from(rel))
}

/// Delete a relationship by ID.
#[tauri::command]
pub async fn delete_relationship(relationship_id: String) -> Result<(), String> {
    let repo = open_repo()?;
    let rid = EntityId::parse(&relationship_id).map_err(|e| e.to_string())?;
    repo.delete_relationship(&rid).await.map_err(|e| e.to_string())
}

/// Update an entity (project or any other type).
#[tauri::command]
pub async fn update_entity(
    id: String,
    title: Option<String>,
    description: Option<String>,
    metadata: Option<std::collections::HashMap<String, serde_json::Value>>,
) -> Result<GraphNode, String> {
    let repo = open_repo()?;
    let entity_id = EntityId::parse(&id).map_err(|e| e.to_string())?;
    let mut entity = repo
        .get_entity(&entity_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Entity {} not found", id))?;

    if let Some(t) = title {
        entity.title = t;
    }
    if let Some(d) = description {
        entity.description = d;
    }
    if let Some(m) = metadata {
        for (k, v) in m {
            entity.metadata.insert(k, v);
        }
    }
    entity.updated_at = chrono::Utc::now();
    repo.update_entity(&entity).await.map_err(|e| e.to_string())?;
    Ok(GraphNode::from(entity))
}

/// Get entity metadata (or empty map if entity has none).
#[tauri::command]
pub async fn get_entity_metadata(id: String) -> Result<std::collections::HashMap<String, serde_json::Value>, String> {
    let repo = open_repo()?;
    let entity_id = EntityId::parse(&id).map_err(|e| e.to_string())?;
    let entity = repo
        .get_entity(&entity_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Entity {} not found", id))?;
    Ok(entity.metadata)
}

/// Delete an entity by ID.
/// Also cleans up memory_entity_links and relationships to prevent orphaned data.
#[tauri::command]
pub async fn delete_entity(id: String) -> Result<(), String> {
    let entity_id = EntityId::parse(&id).map_err(|e| e.to_string())?;

    // Clean up memory_entity_links first
    let links_conn = crate::db::open_connection().map_err(|e| e.to_string())?;
    let links_repo = crate::storage::sqlite::SqliteMemoryEntityLinkRepository::new(links_conn)
        .map_err(|e| e.to_string())?;
    use crate::storage::sqlite::memory_entity_links_repository::MemoryEntityLinkRepository;
    links_repo.delete_links_for_entity(&entity_id).await.map_err(|e| e.to_string())?;

    // Clean up relationships involving this entity
    let repo = open_repo()?;
    let rels = repo.get_entity_relationships(&entity_id).await.map_err(|e| e.to_string())?;
    for rel in rels {
        let _ = repo.delete_relationship(&rel.id).await;
    }

    repo.delete_entity(&entity_id).await.map_err(|e| e.to_string())
}
