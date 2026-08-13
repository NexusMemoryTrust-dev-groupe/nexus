//! Knowledge Navigation 2.0 commands — «AI Universe» вокруг сущности.
//!
//! `knowledge_map` собирает концентрическую карту знания: что в работе
//! (Mission), что релевантно (Relevant), что поддерживает (Supporting),
//! что устарело (Historical). Пользователь пилотирует внутри карты, а не
//! смотрит на сырой граф сверху.

use serde::Serialize;

use crate::core::graph::{GraphNeighborhood, GraphStore, GraphTraversal};
use crate::core::knowledge::knowledge_map::{KnowledgeMap, MapItem, MapRing, ring_for_layer};
use crate::core::memory::conflict::ConflictRepository;
use crate::core::memory::memory_repository::MemoryRepository;
use crate::storage::sqlite::memory_entity_links_repository::MemoryEntityLinkRepository;
use crate::storage::sqlite::{
    SqliteConflictRepository, SqliteGraphRepository, SqliteMemoryEntityLinkRepository,
    SqliteMemoryRepository,
};

/// Serializable map item for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapItemDto {
    pub ring: String,
    pub kind: String,
    pub id: String,
    pub title: String,
    pub layer: String,
    pub weight: f64,
    pub owner: String,
}

impl From<&MapItem> for MapItemDto {
    fn from(i: &MapItem) -> Self {
        Self {
            ring: i.ring.as_str().to_string(),
            kind: i.kind.clone(),
            id: i.id.clone(),
            title: i.title.clone(),
            layer: i.layer.clone(),
            weight: i.weight,
            owner: i.owner.clone(),
        }
    }
}

/// Serializable knowledge map for Tauri IPC.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeMapDto {
    pub entity_id: String,
    pub entity_title: String,
    pub mission: Vec<MapItemDto>,
    pub relevant: Vec<MapItemDto>,
    pub supporting: Vec<MapItemDto>,
    pub historical: Vec<MapItemDto>,
    pub total: usize,
    pub rendered: String,
}

/// Собрать карту знания вокруг сущности.
#[tauri::command]
pub async fn knowledge_map(
    entity_id: String,
    depth: Option<u32>,
) -> Result<KnowledgeMapDto, String> {
    let eid = crate::core::entity_id::EntityId::parse(&entity_id).map_err(|e| e.to_string())?;
    let depth = depth.unwrap_or(1).max(1);

    let graph_conn = crate::db::open_connection().map_err(|e| e.to_string())?;
    let mem_conn = crate::db::open_connection().map_err(|e| e.to_string())?;
    let link_conn = crate::db::open_connection().map_err(|e| e.to_string())?;
    let conflict_conn = crate::db::open_connection().map_err(|e| e.to_string())?;

    let graph = SqliteGraphRepository::new(graph_conn).map_err(|e| e.to_string())?;
    let memory = SqliteMemoryRepository::new(mem_conn).map_err(|e| e.to_string())?;
    let links = SqliteMemoryEntityLinkRepository::new(link_conn).map_err(|e| e.to_string())?;
    let conflicts = SqliteConflictRepository::new(conflict_conn).map_err(|e| e.to_string())?;

    let entity = graph
        .get_entity(&eid)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Entity not found: {entity_id}"))?;
    let title = entity.title.clone();

    let mut items: Vec<MapItem> = Vec::new();

    // 1. Соседи по графу → Relevant (прямая связь, вес = сила связи).
    let neighborhood: GraphNeighborhood = graph
        .get_neighbors(&eid, depth)
        .await
        .map_err(|e| e.to_string())?;
    for n in &neighborhood.entities {
        if n.id != eid {
            items.push(MapItem {
                ring: MapRing::Relevant,
                kind: "entity".to_string(),
                id: n.id.as_str().to_string(),
                title: n.title.clone(),
                layer: format!("{:?}", n.entity_type),
                weight: 0.7,
                owner: String::new(),
            });
        }
    }

    // 2. Память, связанная с сущностью → распределение по кольцам.
    let linked = links
        .get_links_for_entity(&eid)
        .await
        .map_err(|e| e.to_string())?;
    for link in &linked {
        let mid = link.memory_id.clone();
        if let Ok(Some(rec)) = memory.get_by_id(&mid).await {
            let superseded = rec.memory_state.as_str() == "Superseded";
            let ring = ring_for_layer(rec.layer.as_str(), superseded);
            items.push(MapItem {
                ring,
                kind: "memory".to_string(),
                id: rec.id.as_str().to_string(),
                title: rec.title.clone(),
                layer: rec.layer.as_str().to_string(),
                weight: rec.importance_score,
                owner: rec.author.clone(),
            });

            // 3. Открытые конфликты вокруг этой памяти → Supporting.
            if let Ok(groups) = conflicts.open_groups_containing(&mid).await {
                for g in groups {
                    items.push(MapItem {
                        ring: MapRing::Supporting,
                        kind: "conflict".to_string(),
                        id: g.id.as_str().to_string(),
                        title: g.topic.clone(),
                        layer: "conflict".to_string(),
                        weight: 0.5,
                        owner: String::new(),
                    });
                }
            }
        }
    }

    // 4. Superseded-версии связанных записей → Historical.
    let mut seen_historical = std::collections::HashSet::new();
    let mut historical: Vec<MapItem> = items
        .iter()
        .filter(|i| i.ring == MapRing::Historical)
        .cloned()
        .collect();
    for item in &historical {
        seen_historical.insert(item.id.clone());
    }
    // Дополнительно ищем superseded-память по имени сущности.
    if let Ok(hits) = memory.search(&entity.title).await {
        for rec in hits {
            if rec.memory_state.as_str() != "Superseded" {
                continue;
            }
            if seen_historical.contains(rec.id.as_str()) {
                continue;
            }
            seen_historical.insert(rec.id.as_str().to_string());
            historical.push(MapItem {
                ring: MapRing::Historical,
                kind: "memory".to_string(),
                id: rec.id.as_str().to_string(),
                title: rec.title.clone(),
                layer: rec.layer.as_str().to_string(),
                weight: rec.importance_score,
                owner: rec.author.clone(),
            });
        }
    }

    let map = KnowledgeMap {
        entity_id: entity_id.clone(),
        entity_title: title.clone(),
        items,
    };
    let dto = KnowledgeMapDto {
        entity_id,
        entity_title: title,
        mission: map
            .ring(MapRing::Mission)
            .iter()
            .map(|i| MapItemDto::from(*i))
            .collect(),
        relevant: map
            .ring(MapRing::Relevant)
            .iter()
            .map(|i| MapItemDto::from(*i))
            .collect(),
        supporting: map
            .ring(MapRing::Supporting)
            .iter()
            .map(|i| MapItemDto::from(*i))
            .collect(),
        historical: historical.iter().map(MapItemDto::from).collect(),
        total: map.len() + historical.len(),
        rendered: crate::core::knowledge::knowledge_map::render_map(&map),
    };
    Ok(dto)
}
