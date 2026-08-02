use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;

use crate::core::entity_id::EntityId;
use crate::core::graph::entity::{Entity, EntityStatus};
use crate::core::graph::entity_identity::EntityIdentityService;
use crate::core::graph::entity_types::EntityType;
use crate::core::graph::graph_query::{
    GraphQuery, GraphQueryRequest, GraphQueryResult, TimelineEvent,
};
use crate::core::graph::graph_store::GraphStore;
use crate::core::graph::graph_traversal::{GraphNeighborhood, GraphTraversal, SubGraph};
use crate::core::graph::relationship::Relationship;
use crate::core::graph::relationship_types::RelationshipType;
use crate::core::result::{AppError, Result};

const ENTITY_COLS: &str = "id, entity_type, title, description, created_at, updated_at, status, metadata_json, canonical_id";
const REL_COLS: &str = "id, source_entity_id, target_entity_id, relationship_type, weight, created_at, created_by, metadata_json";

/// SQLite-backed graph store implementing GraphStore, GraphTraversal, GraphQuery.
pub struct SqliteGraphRepository {
    conn: Mutex<Connection>,
}

impl SqliteGraphRepository {
    pub fn new(conn: Connection) -> Result<Self> {
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

fn lock(conn: &Mutex<Connection>) -> Result<std::sync::MutexGuard<'_, Connection>> {
    conn.lock().map_err(|e| AppError::Internal(e.to_string()))
}

fn row_to_entity(row: &rusqlite::Row) -> rusqlite::Result<Entity> {
    let id: String = row.get(0)?;
    let entity_type_str: String = row.get(1)?;
    let title: String = row.get(2)?;
    let description: String = row.get(3)?;
    let created_at: String = row.get(4)?;
    let updated_at: String = row.get(5)?;
    let status_str: String = row.get(6)?;
    let metadata_json: String = row.get(7)?;
    let canonical_id: Option<String> = row.get(8)?;

    let metadata: HashMap<String, serde_json::Value> =
        serde_json::from_str(&metadata_json).unwrap_or_default();

    Ok(Entity {
        id: EntityId::parse(&id)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        entity_type: EntityType::from(entity_type_str.as_str()),
        title,
        description,
        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        status: match status_str.as_str() {
            "Active" => EntityStatus::Active,
            "Archived" => EntityStatus::Archived,
            "Merged" => EntityStatus::Merged,
            _ => EntityStatus::Active,
        },
        metadata,
        canonical_id,
    })
}

fn row_to_relationship(row: &rusqlite::Row) -> rusqlite::Result<Relationship> {
    let id: String = row.get(0)?;
    let source_id: String = row.get(1)?;
    let target_id: String = row.get(2)?;
    let rel_type_str: String = row.get(3)?;
    let weight: f64 = row.get(4)?;
    let created_at: String = row.get(5)?;
    let created_by: String = row.get(6)?;
    let metadata_json: String = row.get(7)?;

    let metadata: HashMap<String, serde_json::Value> =
        serde_json::from_str(&metadata_json).unwrap_or_default();

    Ok(Relationship {
        id: EntityId::parse(&id)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        source_entity_id: EntityId::parse(&source_id)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        target_entity_id: EntityId::parse(&target_id)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        relationship_type: RelationshipType::from(rel_type_str.as_str()),
        weight,
        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        created_by,
        metadata,
    })
}

// ── GraphStore ───────────────────────────────────────────────────────

#[async_trait]
impl GraphStore for SqliteGraphRepository {
    async fn add_entity(&self, entity: &Entity) -> Result<EntityId> {
        let conn = lock(&self.conn)?;
        let meta_json = serde_json::to_string(&entity.metadata)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        conn.execute(
            "INSERT INTO graph_entities (id, entity_type, title, description, created_at, updated_at, status, metadata_json, canonical_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entity.id.as_str(),
                entity.entity_type.as_str(),
                entity.title,
                entity.description,
                entity.created_at.to_rfc3339(),
                entity.updated_at.to_rfc3339(),
                format!("{:?}", entity.status),
                meta_json,
                entity.canonical_id,
            ],
        ).map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(entity.id.clone())
    }

    async fn get_entity(&self, id: &EntityId) -> Result<Option<Entity>> {
        let conn = lock(&self.conn)?;
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM graph_entities WHERE id = ?1",
                ENTITY_COLS
            ))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        stmt.query_row(params![id.as_str()], row_to_entity)
            .optional()
            .map_err(|e| AppError::Internal(e.to_string()))
    }

    async fn update_entity(&self, entity: &Entity) -> Result<()> {
        let conn = lock(&self.conn)?;
        let meta_json = serde_json::to_string(&entity.metadata)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let rows = conn
            .execute(
                "UPDATE graph_entities SET entity_type = ?2, title = ?3, description = ?4,
             updated_at = ?5, status = ?6, metadata_json = ?7, canonical_id = ?8 WHERE id = ?1",
                params![
                    entity.id.as_str(),
                    entity.entity_type.as_str(),
                    entity.title,
                    entity.description,
                    entity.updated_at.to_rfc3339(),
                    format!("{:?}", entity.status),
                    meta_json,
                    entity.canonical_id,
                ],
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;
        if rows == 0 {
            return Err(AppError::NotFound(format!(
                "Entity {} not found",
                entity.id.as_str()
            )));
        }
        Ok(())
    }

    async fn delete_entity(&self, id: &EntityId) -> Result<()> {
        let conn = lock(&self.conn)?;
        let rows = conn
            .execute(
                "DELETE FROM graph_entities WHERE id = ?1",
                params![id.as_str()],
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;
        if rows == 0 {
            return Err(AppError::NotFound(format!(
                "Entity {} not found",
                id.as_str()
            )));
        }
        Ok(())
    }

    async fn add_relationship(&self, relationship: &Relationship) -> Result<EntityId> {
        let conn = lock(&self.conn)?;
        let meta_json = serde_json::to_string(&relationship.metadata)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        conn.execute(
            "INSERT INTO graph_relationships (id, source_entity_id, target_entity_id, relationship_type, weight, created_at, created_by, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                relationship.id.as_str(),
                relationship.source_entity_id.as_str(),
                relationship.target_entity_id.as_str(),
                relationship.relationship_type.as_str(),
                relationship.weight,
                relationship.created_at.to_rfc3339(),
                relationship.created_by,
                meta_json,
            ],
        ).map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(relationship.id.clone())
    }

    async fn get_relationship(&self, id: &EntityId) -> Result<Option<Relationship>> {
        let conn = lock(&self.conn)?;
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM graph_relationships WHERE id = ?1",
                REL_COLS
            ))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        stmt.query_row(params![id.as_str()], row_to_relationship)
            .optional()
            .map_err(|e| AppError::Internal(e.to_string()))
    }

    async fn delete_relationship(&self, id: &EntityId) -> Result<()> {
        let conn = lock(&self.conn)?;
        let rows = conn
            .execute(
                "DELETE FROM graph_relationships WHERE id = ?1",
                params![id.as_str()],
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;
        if rows == 0 {
            return Err(AppError::NotFound(format!(
                "Relationship {} not found",
                id.as_str()
            )));
        }
        Ok(())
    }

    async fn get_entity_relationships(&self, entity_id: &EntityId) -> Result<Vec<Relationship>> {
        let conn = lock(&self.conn)?;
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM graph_relationships WHERE source_entity_id = ?1 OR target_entity_id = ?1",
                REL_COLS
            ))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let rows = stmt
            .query_map(params![entity_id.as_str()], row_to_relationship)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        rows.map(|r| r.map_err(|e| AppError::Internal(e.to_string())))
            .collect()
    }

    async fn get_entities_by_type(&self, entity_type: &EntityType) -> Result<Vec<Entity>> {
        let conn = lock(&self.conn)?;
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM graph_entities WHERE entity_type = ?1",
                ENTITY_COLS
            ))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let rows = stmt
            .query_map(params![entity_type.as_str()], row_to_entity)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        rows.map(|r| r.map_err(|e| AppError::Internal(e.to_string())))
            .collect()
    }

    async fn search_entities(&self, query: &str) -> Result<Vec<Entity>> {
        let conn = lock(&self.conn)?;
        let words: Vec<&str> = query.split_whitespace().collect();
        if words.is_empty() {
            return Ok(vec![]);
        }
        // Build WHERE clause: each word must match title OR description (AND across words)
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        for (i, word) in words.iter().enumerate() {
            let pattern = format!("%{}%", word);
            conditions.push(format!(
                "(title LIKE ?{} OR description LIKE ?{})",
                i + 1,
                i + 1
            ));
            params.push(Box::new(pattern));
        }
        let where_clause = conditions.join(" AND ");
        let sql = format!(
            "SELECT {} FROM graph_entities WHERE {}",
            ENTITY_COLS, where_clause
        );
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt
            .query_map(param_refs.as_slice(), row_to_entity)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        rows.map(|r| r.map_err(|e| AppError::Internal(e.to_string())))
            .collect()
    }

    async fn count_entities(&self) -> Result<u64> {
        let conn = lock(&self.conn)?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM graph_entities", [], |row| row.get(0))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(count as u64)
    }

    async fn count_relationships(&self) -> Result<u64> {
        let conn = lock(&self.conn)?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM graph_relationships", [], |row| {
                row.get(0)
            })
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(count as u64)
    }
}

// ── GraphTraversal ───────────────────────────────────────────────────

#[async_trait]
impl GraphTraversal for SqliteGraphRepository {
    async fn get_neighbors(&self, entity_id: &EntityId, depth: u32) -> Result<GraphNeighborhood> {
        let center = self.get_entity(entity_id).await?.ok_or_else(|| {
            AppError::NotFound(format!("Entity {} not found", entity_id.as_str()))
        })?;

        let mut visited: HashSet<String> = HashSet::new();
        let mut visited_rels: HashSet<String> = HashSet::new();
        let mut all_entities: Vec<Entity> = Vec::new();
        let mut all_relationships: Vec<Relationship> = Vec::new();
        let mut frontier: VecDeque<EntityId> = VecDeque::new();
        frontier.push_back(entity_id.clone());
        visited.insert(entity_id.as_str().to_string());

        for _ in 0..depth {
            let level_size = frontier.len();
            for _ in 0..level_size {
                if let Some(eid) = frontier.pop_front() {
                    let rels = self.get_entity_relationships(&eid).await?;
                    for rel in &rels {
                        if !visited_rels.contains(rel.id.as_str()) {
                            visited_rels.insert(rel.id.as_str().to_string());
                            all_relationships.push(rel.clone());

                            let neighbor_id = if rel.source_entity_id == eid {
                                &rel.target_entity_id
                            } else {
                                &rel.source_entity_id
                            };

                            if !visited.contains(neighbor_id.as_str()) {
                                visited.insert(neighbor_id.as_str().to_string());
                                if let Ok(Some(entity)) = self.get_entity(neighbor_id).await {
                                    frontier.push_back(neighbor_id.clone());
                                    all_entities.push(entity);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(GraphNeighborhood {
            center,
            entities: all_entities,
            relationships: all_relationships,
        })
    }

    async fn get_distance(&self, from: &EntityId, to: &EntityId) -> Result<Option<u32>> {
        if from == to {
            return Ok(Some(0));
        }

        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<(EntityId, u32)> = VecDeque::new();
        queue.push_back((from.clone(), 0));
        visited.insert(from.as_str().to_string());

        while let Some((eid, dist)) = queue.pop_front() {
            if dist >= 10 {
                continue;
            }
            let rels = self.get_entity_relationships(&eid).await?;
            for rel in &rels {
                let neighbor_id = if rel.source_entity_id == eid {
                    &rel.target_entity_id
                } else {
                    &rel.source_entity_id
                };
                if neighbor_id == to {
                    return Ok(Some(dist + 1));
                }
                if !visited.contains(neighbor_id.as_str()) {
                    visited.insert(neighbor_id.as_str().to_string());
                    queue.push_back((neighbor_id.clone(), dist + 1));
                }
            }
        }
        Ok(None)
    }

    async fn find_path(
        &self,
        from: &EntityId,
        to: &EntityId,
        max_depth: u32,
    ) -> Result<Option<Vec<EntityId>>> {
        if from == to {
            return Ok(Some(vec![from.clone()]));
        }

        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<(EntityId, Vec<EntityId>)> = VecDeque::new();
        queue.push_back((from.clone(), vec![from.clone()]));
        visited.insert(from.as_str().to_string());

        while let Some((eid, path)) = queue.pop_front() {
            if path.len() as u32 > max_depth {
                continue;
            }
            let rels = self.get_entity_relationships(&eid).await?;
            for rel in &rels {
                let neighbor_id = if rel.source_entity_id == eid {
                    &rel.target_entity_id
                } else {
                    &rel.source_entity_id
                };
                if neighbor_id == to {
                    let mut result = path.clone();
                    result.push(neighbor_id.clone());
                    return Ok(Some(result));
                }
                if !visited.contains(neighbor_id.as_str()) {
                    visited.insert(neighbor_id.as_str().to_string());
                    let mut new_path = path.clone();
                    new_path.push(neighbor_id.clone());
                    queue.push_back((neighbor_id.clone(), new_path));
                }
            }
        }
        Ok(None)
    }

    async fn get_subgraph(&self, entity_id: &EntityId, radius: u32) -> Result<SubGraph> {
        let neighborhood = self.get_neighbors(entity_id, radius).await?;
        let mut entities = neighborhood.entities;
        entities.push(neighborhood.center);
        Ok(SubGraph {
            entities,
            relationships: neighborhood.relationships,
        })
    }
}

// ── GraphQuery ───────────────────────────────────────────────────────

#[async_trait]
impl GraphQuery for SqliteGraphRepository {
    async fn query(&self, request: &GraphQueryRequest) -> Result<GraphQueryResult> {
        let conn = lock(&self.conn)?;

        // Build entity query
        let mut conditions: Vec<String> = Vec::new();
        let mut param_values: Vec<String> = Vec::new();

        if let Some(ref et) = request.entity_type {
            conditions.push("entity_type = ?".to_string());
            param_values.push(et.as_str().to_string());
        }

        let mut entity_sql = format!("SELECT {} FROM graph_entities", ENTITY_COLS);
        if !conditions.is_empty() {
            entity_sql.push_str(" WHERE ");
            entity_sql.push_str(&conditions.join(" AND "));
        }

        let limit = request.limit;
        let mut stmt = conn
            .prepare(&entity_sql)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = param_values
            .iter()
            .map(|s| s as &dyn rusqlite::types::ToSql)
            .collect();

        let entities: Vec<Entity> = stmt
            .query_map(param_refs.as_slice(), row_to_entity)
            .map_err(|e| AppError::Internal(e.to_string()))?
            .take(limit as usize)
            .map(|r| r.map_err(|e| AppError::Internal(e.to_string())))
            .collect::<Result<Vec<_>>>()?;

        // Build relationship query
        let mut rel_conditions: Vec<String> = Vec::new();
        let mut rel_params: Vec<String> = Vec::new();

        if let Some(ref rt) = request.relationship_type {
            rel_conditions.push("relationship_type = ?".to_string());
            rel_params.push(rt.as_str().to_string());
        }
        if let Some(min_w) = request.min_weight {
            rel_conditions.push("weight >= ?".to_string());
            rel_params.push(min_w.to_string());
        }

        let mut rel_sql = format!("SELECT {} FROM graph_relationships", REL_COLS);
        if !rel_conditions.is_empty() {
            rel_sql.push_str(" WHERE ");
            rel_sql.push_str(&rel_conditions.join(" AND "));
        }

        let mut rel_stmt = conn
            .prepare(&rel_sql)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let rel_param_refs: Vec<&dyn rusqlite::types::ToSql> = rel_params
            .iter()
            .map(|s| s as &dyn rusqlite::types::ToSql)
            .collect();

        let relationships: Vec<Relationship> = rel_stmt
            .query_map(rel_param_refs.as_slice(), row_to_relationship)
            .map_err(|e| AppError::Internal(e.to_string()))?
            .take(limit as usize)
            .map(|r| r.map_err(|e| AppError::Internal(e.to_string())))
            .collect::<Result<Vec<_>>>()?;

        let total = entities.len() as f64;
        Ok(GraphQueryResult {
            entities,
            relationships,
            score: if total > 0.0 { 1.0 } else { 0.0 },
        })
    }

    async fn get_knowledge_density(&self, entity_id: &EntityId) -> Result<f64> {
        let neighborhood = self.get_neighbors(entity_id, 1).await?;
        let neighbor_count = neighborhood.entities.len() as f64;
        if neighbor_count <= 1.0 {
            return Ok(0.0);
        }
        let actual_edges = neighborhood.relationships.len() as f64;
        let possible_edges = neighbor_count * (neighbor_count - 1.0) / 2.0;
        if possible_edges == 0.0 {
            Ok(0.0)
        } else {
            Ok(actual_edges / possible_edges)
        }
    }

    async fn get_timeline(&self, entity_id: &EntityId) -> Result<Vec<TimelineEvent>> {
        let entity = self.get_entity(entity_id).await?.ok_or_else(|| {
            AppError::NotFound(format!("Entity {} not found", entity_id.as_str()))
        })?;

        let rels = self.get_entity_relationships(entity_id).await?;
        let mut events: Vec<TimelineEvent> = rels
            .into_iter()
            .map(|r| TimelineEvent {
                entity: entity.clone(),
                relationship: Some(r.clone()),
                timestamp: r.created_at,
            })
            .collect();
        events.sort_by_key(|b| std::cmp::Reverse(b.timestamp));
        Ok(events)
    }
}

// ── EntityIdentityService ────────────────────────────────────────────

#[async_trait]
impl EntityIdentityService for SqliteGraphRepository {
    async fn find_duplicates(&self, entity: &Entity) -> Result<Vec<Entity>> {
        let all = self.search_entities(&entity.title).await?;
        Ok(all
            .into_iter()
            .filter(|e| e.id != entity.id && e.entity_type == entity.entity_type)
            .collect())
    }

    async fn merge_entities(&self, primary: &EntityId, duplicates: &[EntityId]) -> Result<Entity> {
        let mut primary_entity = self.get_entity(primary).await?.ok_or_else(|| {
            AppError::NotFound(format!("Primary entity {} not found", primary.as_str()))
        })?;

        for dup_id in duplicates {
            if let Ok(Some(mut dup_entity)) = self.get_entity(dup_id).await {
                // Merge metadata
                for (k, v) in dup_entity.metadata.drain() {
                    primary_entity.metadata.entry(k).or_insert(v);
                }
                // Redirect relationships (scope lock before await)
                {
                    let rels = self.get_entity_relationships(dup_id).await?;
                    let conn = lock(&self.conn)?;
                    for rel in &rels {
                        if rel.source_entity_id == *dup_id {
                            conn.execute(
                                "UPDATE graph_relationships SET source_entity_id = ?1 WHERE id = ?2",
                                params![primary.as_str(), rel.id.as_str()],
                            ).map_err(|e| AppError::Internal(e.to_string()))?;
                        }
                        if rel.target_entity_id == *dup_id {
                            conn.execute(
                                "UPDATE graph_relationships SET target_entity_id = ?1 WHERE id = ?2",
                                params![primary.as_str(), rel.id.as_str()],
                            ).map_err(|e| AppError::Internal(e.to_string()))?;
                        }
                    }
                }
                // Mark duplicate as merged
                dup_entity.status = EntityStatus::Merged;
                dup_entity.canonical_id = Some(primary.as_str().to_string());
                self.update_entity(&dup_entity).await?;
            }
        }

        self.update_entity(&primary_entity).await?;
        Ok(primary_entity)
    }

    async fn get_canonical(&self, entity_id: &EntityId) -> Result<Entity> {
        let entity = self.get_entity(entity_id).await?.ok_or_else(|| {
            AppError::NotFound(format!("Entity {} not found", entity_id.as_str()))
        })?;

        if let Some(ref canonical_id) = entity.canonical_id
            && let Ok(canonical) = EntityId::parse(canonical_id)
            && let Ok(Some(e)) = self.get_entity(&canonical).await
        {
            return Ok(e);
        }
        Ok(entity)
    }

    async fn resolve_alias(&self, name: &str) -> Result<Option<Entity>> {
        let results = self.search_entities(name).await?;
        Ok(results.into_iter().next())
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> SqliteGraphRepository {
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::sqlite::schema::apply_migrations(&conn).unwrap();
        SqliteGraphRepository::new(conn).unwrap()
    }

    fn sample_entity(title: &str) -> Entity {
        Entity::new(EntityType::Person, title.to_string(), "desc".to_string())
    }

    #[tokio::test]
    async fn add_and_get_entity() {
        let r = repo();
        let e = sample_entity("Alice");
        let id = r.add_entity(&e).await.unwrap();
        let fetched = r.get_entity(&id).await.unwrap().unwrap();
        assert_eq!(fetched.title, "Alice");
    }

    #[tokio::test]
    async fn get_nonexistent_entity() {
        let r = repo();
        let result = r.get_entity(&EntityId::new()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn update_entity() {
        let r = repo();
        let mut e = sample_entity("Alice");
        r.add_entity(&e).await.unwrap();
        e.title = "Alice Updated".to_string();
        r.update_entity(&e).await.unwrap();
        let fetched = r.get_entity(&e.id).await.unwrap().unwrap();
        assert_eq!(fetched.title, "Alice Updated");
    }

    #[tokio::test]
    async fn update_nonexistent_entity() {
        let r = repo();
        let e = sample_entity("Ghost");
        assert!(r.update_entity(&e).await.is_err());
    }

    #[tokio::test]
    async fn delete_entity() {
        let r = repo();
        let e = sample_entity("Alice");
        let id = r.add_entity(&e).await.unwrap();
        r.delete_entity(&id).await.unwrap();
        assert!(r.get_entity(&id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_entity() {
        let r = repo();
        assert!(r.delete_entity(&EntityId::new()).await.is_err());
    }

    #[tokio::test]
    async fn add_and_get_relationship() {
        let r = repo();
        let e1 = sample_entity("A");
        let e2 = sample_entity("B");
        let id1 = r.add_entity(&e1).await.unwrap();
        let id2 = r.add_entity(&e2).await.unwrap();

        let rel = Relationship::new(id1, id2, RelationshipType::Created, 0.8).unwrap();
        let rel_id = r.add_relationship(&rel).await.unwrap();
        let fetched = r.get_relationship(&rel_id).await.unwrap().unwrap();
        assert_eq!(fetched.weight, 0.8);
    }

    #[tokio::test]
    async fn delete_relationship() {
        let r = repo();
        let e1 = sample_entity("A");
        let e2 = sample_entity("B");
        let id1 = r.add_entity(&e1).await.unwrap();
        let id2 = r.add_entity(&e2).await.unwrap();
        let rel = Relationship::new(id1, id2, RelationshipType::Uses, 0.5).unwrap();
        let rel_id = r.add_relationship(&rel).await.unwrap();
        r.delete_relationship(&rel_id).await.unwrap();
        assert!(r.get_relationship(&rel_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn get_entity_relationships_filters() {
        let r = repo();
        let e1 = sample_entity("A");
        let e2 = sample_entity("B");
        let e3 = sample_entity("C");
        let id1 = r.add_entity(&e1).await.unwrap();
        let id2 = r.add_entity(&e2).await.unwrap();
        let id3 = r.add_entity(&e3).await.unwrap();

        let rel1 =
            Relationship::new(id1.clone(), id2.clone(), RelationshipType::Owns, 1.0).unwrap();
        let rel2 =
            Relationship::new(id2.clone(), id3.clone(), RelationshipType::Uses, 0.5).unwrap();
        r.add_relationship(&rel1).await.unwrap();
        r.add_relationship(&rel2).await.unwrap();

        let rels = r.get_entity_relationships(&id2).await.unwrap();
        assert_eq!(rels.len(), 2);
    }

    #[tokio::test]
    async fn get_entities_by_type() {
        let r = repo();
        r.add_entity(&sample_entity("Alice")).await.unwrap();
        r.add_entity(&sample_entity("Bob")).await.unwrap();
        let proj = Entity::new(EntityType::Project, "Proj".to_string(), "d".to_string());
        r.add_entity(&proj).await.unwrap();

        let persons = r.get_entities_by_type(&EntityType::Person).await.unwrap();
        assert_eq!(persons.len(), 2);
        let projects = r.get_entities_by_type(&EntityType::Project).await.unwrap();
        assert_eq!(projects.len(), 1);
    }

    #[tokio::test]
    async fn search_entities() {
        let r = repo();
        r.add_entity(&sample_entity("Alice Smith")).await.unwrap();
        r.add_entity(&sample_entity("Bob Jones")).await.unwrap();

        let results = r.search_entities("Alice").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Alice Smith");
    }

    #[tokio::test]
    async fn count_entities_and_relationships() {
        let r = repo();
        assert_eq!(r.count_entities().await.unwrap(), 0);
        r.add_entity(&sample_entity("A")).await.unwrap();
        assert_eq!(r.count_entities().await.unwrap(), 1);

        let e2 = sample_entity("B");
        let id2 = r.add_entity(&e2).await.unwrap();
        let rel = Relationship::new(
            r.get_entities_by_type(&EntityType::Person).await.unwrap()[0]
                .id
                .clone(),
            id2,
            RelationshipType::RelatedTo,
            0.3,
        )
        .unwrap();
        r.add_relationship(&rel).await.unwrap();
        assert_eq!(r.count_relationships().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn get_distance_same_entity() {
        let r = repo();
        let e = sample_entity("A");
        let id = r.add_entity(&e).await.unwrap();
        assert_eq!(r.get_distance(&id, &id).await.unwrap(), Some(0));
    }

    #[tokio::test]
    async fn get_distance_direct() {
        let r = repo();
        let e1 = sample_entity("A");
        let e2 = sample_entity("B");
        let id1 = r.add_entity(&e1).await.unwrap();
        let id2 = r.add_entity(&e2).await.unwrap();
        let rel =
            Relationship::new(id1.clone(), id2.clone(), RelationshipType::RelatedTo, 1.0).unwrap();
        r.add_relationship(&rel).await.unwrap();
        assert_eq!(r.get_distance(&id1, &id2).await.unwrap(), Some(1));
        assert_eq!(r.get_distance(&id2, &id1).await.unwrap(), Some(1));
    }

    #[tokio::test]
    async fn get_distance_no_path() {
        let r = repo();
        let e1 = sample_entity("A");
        let e2 = sample_entity("B");
        let id1 = r.add_entity(&e1).await.unwrap();
        let id2 = r.add_entity(&e2).await.unwrap();
        assert_eq!(r.get_distance(&id1, &id2).await.unwrap(), None);
    }

    #[tokio::test]
    async fn find_path_direct() {
        let r = repo();
        let e1 = sample_entity("A");
        let e2 = sample_entity("B");
        let id1 = r.add_entity(&e1).await.unwrap();
        let id2 = r.add_entity(&e2).await.unwrap();
        let rel =
            Relationship::new(id1.clone(), id2.clone(), RelationshipType::RelatedTo, 1.0).unwrap();
        r.add_relationship(&rel).await.unwrap();
        let path = r.find_path(&id1, &id2, 5).await.unwrap().unwrap();
        assert_eq!(path.len(), 2);
    }

    #[tokio::test]
    async fn find_path_no_path() {
        let r = repo();
        let e1 = sample_entity("A");
        let e2 = sample_entity("B");
        let id1 = r.add_entity(&e1).await.unwrap();
        let id2 = r.add_entity(&e2).await.unwrap();
        assert!(r.find_path(&id1, &id2, 3).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn knowledge_density_empty() {
        let r = repo();
        let e = sample_entity("A");
        let id = r.add_entity(&e).await.unwrap();
        assert_eq!(r.get_knowledge_density(&id).await.unwrap(), 0.0);
    }

    #[tokio::test]
    async fn knowledge_density_with_connections() {
        let r = repo();
        let e1 = sample_entity("A");
        let e2 = sample_entity("B");
        let e3 = sample_entity("C");
        let id1 = r.add_entity(&e1).await.unwrap();
        let id2 = r.add_entity(&e2).await.unwrap();
        let id3 = r.add_entity(&e3).await.unwrap();

        r.add_relationship(
            &Relationship::new(id1.clone(), id2.clone(), RelationshipType::RelatedTo, 1.0).unwrap(),
        )
        .await
        .unwrap();
        r.add_relationship(
            &Relationship::new(id1.clone(), id3.clone(), RelationshipType::RelatedTo, 1.0).unwrap(),
        )
        .await
        .unwrap();
        r.add_relationship(
            &Relationship::new(id2.clone(), id3.clone(), RelationshipType::RelatedTo, 1.0).unwrap(),
        )
        .await
        .unwrap();

        let density = r.get_knowledge_density(&id1).await.unwrap();
        assert!(density > 0.0);
    }

    #[tokio::test]
    async fn query_by_type() {
        let r = repo();
        r.add_entity(&sample_entity("Alice")).await.unwrap();
        r.add_entity(&sample_entity("Bob")).await.unwrap();
        let proj = Entity::new(EntityType::Project, "Proj".to_string(), "d".to_string());
        r.add_entity(&proj).await.unwrap();

        let req = GraphQueryRequest {
            entity_type: Some(EntityType::Person),
            ..Default::default()
        };
        let result = r.query(&req).await.unwrap();
        assert_eq!(result.entities.len(), 2);
    }

    #[tokio::test]
    async fn query_by_relationship_type() {
        let r = repo();
        let e1 = sample_entity("A");
        let e2 = sample_entity("B");
        let id1 = r.add_entity(&e1).await.unwrap();
        let id2 = r.add_entity(&e2).await.unwrap();
        r.add_relationship(&Relationship::new(id1, id2, RelationshipType::Owns, 1.0).unwrap())
            .await
            .unwrap();

        let req = GraphQueryRequest {
            relationship_type: Some(RelationshipType::Owns),
            ..Default::default()
        };
        let result = r.query(&req).await.unwrap();
        assert_eq!(result.relationships.len(), 1);
    }

    #[tokio::test]
    async fn query_by_min_weight() {
        let r = repo();
        let e1 = sample_entity("A");
        let e2 = sample_entity("B");
        let id1 = r.add_entity(&e1).await.unwrap();
        let id2 = r.add_entity(&e2).await.unwrap();
        r.add_relationship(&Relationship::new(id1, id2, RelationshipType::RelatedTo, 0.3).unwrap())
            .await
            .unwrap();

        let req = GraphQueryRequest {
            min_weight: Some(0.5),
            ..Default::default()
        };
        let result = r.query(&req).await.unwrap();
        assert_eq!(result.relationships.len(), 0);
    }

    #[tokio::test]
    async fn timeline() {
        let r = repo();
        let e1 = sample_entity("A");
        let e2 = sample_entity("B");
        let id1 = r.add_entity(&e1).await.unwrap();
        let id2 = r.add_entity(&e2).await.unwrap();
        r.add_relationship(
            &Relationship::new(id1.clone(), id2.clone(), RelationshipType::RelatedTo, 1.0).unwrap(),
        )
        .await
        .unwrap();

        let timeline = r.get_timeline(&id1).await.unwrap();
        assert_eq!(timeline.len(), 1);
    }

    #[tokio::test]
    async fn merge_entities() {
        let r = repo();
        let e1 = sample_entity("Alice");
        let e2 = sample_entity("Alice Alt");
        let id1 = r.add_entity(&e1).await.unwrap();
        let id2 = r.add_entity(&e2).await.unwrap();

        let merged = r
            .merge_entities(&id1, std::slice::from_ref(&id2))
            .await
            .unwrap();
        assert_eq!(merged.id, id1);
        let dup = r.get_entity(&id2).await.unwrap().unwrap();
        assert_eq!(dup.status, EntityStatus::Merged);
    }

    #[tokio::test]
    async fn resolve_alias() {
        let r = repo();
        r.add_entity(&sample_entity("Alice Smith")).await.unwrap();
        let found = r.resolve_alias("Alice").await.unwrap();
        assert!(found.is_some());
    }
}
