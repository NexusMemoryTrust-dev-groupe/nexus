//! Entity resolution — поиск и объединение дубликатов в knowledge graph.
//!
//! Проблема, которую решает модуль: модель может создать пять сущностей
//! «Nexus», «Nexus MCP», «Nexus Server», «MCP Server», «Memory Server»,
//! хотя это один продукт или связанные компоненты. Без entity resolution
//! граф быстро накапливает дубликаты, ложные связи и паразитные узлы.
//!
//! Три уровня сравнения:
//! 1. Exact — точное совпадение нормализованного имени (безопасно, авто).
//! 2. Normalized — substring-совпадение: «Nexus» ⊂ «Nexus Server» (0.9).
//! 3. Fuzzy — Dice-коэффициент по уникальным словам для похожих имён.
//!
//! Группировка транзитивная (union-find): если «Nexus» похож и на
//! «Nexus MCP», и на «Nexus Server», все три попадают в одну группу,
//! даже если «Nexus MCP» и «Nexus Server» напрямую слабо похожи.

use serde::Serialize;

use crate::core::graph::entity::Entity;
use crate::core::graph::entity_types::EntityType;
use crate::core::graph::graph_store::GraphStore;
use crate::core::result::Result;
use crate::storage::sqlite::SqliteGraphRepository;

/// Порог similarity, выше которого два имени считаются дубликатами.
pub const DUPLICATE_DICE: f64 = 0.78;

/// Грамматические стоп-слова, которые не несут смысла при сравнении имён.
/// Технические слова («server», «mcp», «api») НЕ удаляются — это ключевые
/// маркеры, по которым «Nexus MCP» отличают от «Nexus Server».
const STOP: &[&str] = &[
    "the", "a", "an", "and", "or", "of", "for", "to", "in", "on", "at", "with", "from", "into",
    "via", "by",
];

/// Результат поиска дубликатов для одной сущности.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateCandidate {
    pub entity_id: String,
    pub title: String,
    pub entity_type: String,
    pub score: f64,
    pub match_kind: String, // "exact" | "normalized" | "fuzzy"
}

/// Результат полного сканирования графа на дубликаты.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub entities: Vec<DuplicateCandidate>,
    pub best_id: String,
}

/// Нормализация имени сущности для сравнения:
/// lowercase, разделение по пробелам и пунктуации, отбрасывание стоп-слов.
pub fn normalize_name(name: &str) -> String {
    name.split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .filter(|w| !STOP.contains(&w.as_str()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Dice-коэффициент схожести двух имён (по уникальным словам).
pub fn name_dice(a: &str, b: &str) -> f64 {
    let wa: Vec<&str> = a.split_whitespace().collect();
    let wb: Vec<&str> = b.split_whitespace().collect();
    if wa.is_empty() || wb.is_empty() {
        return 0.0;
    }
    let set_a: std::collections::HashSet<&&str> = wa.iter().collect();
    let set_b: std::collections::HashSet<&&str> = wb.iter().collect();
    let common = set_a.intersection(&set_b).count();
    2.0 * common as f64 / (set_a.len() + set_b.len()) as f64
}

/// Насколько два имени похожи — максимум из exact/normalized/fuzzy.
pub fn similarity(a: &str, b: &str) -> f64 {
    let na = normalize_name(a);
    let nb = normalize_name(b);
    if na == nb && !na.is_empty() {
        return 1.0; // exact после нормализации
    }
    // Одно имя целиком содержится в другом («Nexus» ⊂ «Nexus Server»,
    // «auth» ⊂ «auth service») — общий корень, вероятный дубликат.
    if na.len() >= 3 && nb.len() >= 3 && (na.contains(&nb) || nb.contains(&na)) {
        return 0.9;
    }
    name_dice(&na, &nb)
}

/// Чистая функция группировки дубликатов (транзитивная, union-find).
///
/// Возвращает группы по 2+ сущности, у которых similarity ≥ порога,
/// с транзитивным замыканием: A~B и A~C объединяют B и C в одну группу.
/// Первый кандидат группы — сущность с самым специфичным (длинным) именем
/// (best_id, кандидат на роль canonical); score каждого члена — максимальная
/// схожесть с любым другим членом группы.
pub fn build_groups(entities: &[Entity], min_score: f64) -> Vec<DuplicateGroup> {
    let n = entities.len();
    if n < 2 {
        return Vec::new();
    }

    fn find(parent: &mut [usize], x: usize) -> usize {
        if parent[x] != x {
            let root = find(parent, parent[x]);
            parent[x] = root;
        }
        parent[x]
    }
    fn union(parent: &mut [usize], a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent[rb] = ra;
        }
    }

    let mut parent: Vec<usize> = (0..n).collect();
    for i in 0..n {
        for j in (i + 1)..n {
            if similarity(&entities[i].title, &entities[j].title) >= min_score {
                union(&mut parent, i, j);
            }
        }
    }

    let mut by_root: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();
    for i in 0..n {
        let root = find(&mut parent, i);
        by_root.entry(root).or_default().push(i);
    }

    let mut groups = Vec::new();
    for (_, members) in by_root {
        if members.len() < 2 {
            continue;
        }
        let best_idx = *members
            .iter()
            .max_by_key(|&&i| entities[i].title.len())
            .unwrap();
        let best = &entities[best_idx];
        let mut candidates: Vec<DuplicateCandidate> = members
            .iter()
            .filter(|&&i| i != best_idx)
            .map(|&i| {
                let e = &entities[i];
                let score = members
                    .iter()
                    .filter(|&&j| j != i)
                    .map(|&j| similarity(&e.title, &entities[j].title))
                    .fold(0.0f64, f64::max);
                let kind = if score >= 0.999 {
                    "exact"
                } else if score >= 0.9 {
                    "normalized"
                } else {
                    "fuzzy"
                };
                DuplicateCandidate {
                    entity_id: e.id.as_str().to_string(),
                    title: e.title.clone(),
                    entity_type: format!("{:?}", e.entity_type),
                    score,
                    match_kind: kind.to_string(),
                }
            })
            .collect();
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.insert(
            0,
            DuplicateCandidate {
                entity_id: best.id.as_str().to_string(),
                title: best.title.clone(),
                entity_type: format!("{:?}", best.entity_type),
                score: 1.0,
                match_kind: "exact".to_string(),
            },
        );
        groups.push(DuplicateGroup {
            entities: candidates,
            best_id: best.id.as_str().to_string(),
        });
    }
    groups
}

/// Найти все группы дубликатов в графе.
///
/// Собирает сущности всех известных типов через публичный API трейта
/// GraphStore и группирует их транзитивно по similarity ≥ порога.
pub async fn find_duplicates(
    repo: &SqliteGraphRepository,
    min_score: f64,
) -> Result<Vec<DuplicateGroup>> {
    let known_types = [
        EntityType::Person,
        EntityType::Organization,
        EntityType::Project,
        EntityType::Document,
        EntityType::Meeting,
        EntityType::Decision,
        EntityType::Task,
        EntityType::Technology,
        EntityType::Incident,
        EntityType::Repository,
        EntityType::Service,
        EntityType::Model,
        EntityType::Conversation,
        EntityType::Memory,
    ];
    let mut entities: Vec<Entity> = Vec::new();
    for t in known_types {
        if let Ok(mut list) = repo.get_entities_by_type(&t).await {
            entities.append(&mut list);
        }
    }
    Ok(build_groups(&entities, min_score))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(title: &str) -> Entity {
        Entity::new(EntityType::Project, title.to_string(), String::new())
    }

    #[test]
    fn normalize_strips_grammatical_stop_words() {
        assert_eq!(normalize_name("The Nexus Server"), "nexus server");
        assert_eq!(normalize_name("Auth-Service"), "auth service");
        assert_eq!(normalize_name("Nexus, MCP"), "nexus mcp");
    }

    #[test]
    fn exact_match_is_one() {
        assert_eq!(similarity("Nexus Server", "nexus server"), 1.0);
        assert_eq!(similarity("Auth", "auth"), 1.0);
    }

    #[test]
    fn substring_catches_prefix_variants() {
        // «Nexus» ⊂ «Nexus Server» — общий корень.
        assert!(similarity("Nexus", "Nexus Server") >= 0.78);
        assert!(similarity("auth", "auth service") >= 0.78);
    }

    #[test]
    fn transitive_grouping_joins_product_family() {
        // «Nexus», «Nexus MCP», «Nexus Server» — один продукт из задания.
        let ents = vec![
            entity("Nexus"),
            entity("Nexus MCP"),
            entity("Nexus Server"),
            entity("Parser"),
        ];
        let groups = build_groups(&ents, DUPLICATE_DICE);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].entities.len(), 3);
        // Лучший кандидат — самое специфичное имя.
        assert_eq!(groups[0].entities[0].title, "Nexus Server");
        assert_eq!(groups[0].best_id, groups[0].entities[0].entity_id);
        // «Parser» не попал в группу.
        assert!(groups[0].entities.iter().all(|c| c.title != "Parser"));
    }

    #[test]
    fn unrelated_names_low_score() {
        assert!(similarity("Database", "Chat") < 0.5);
        assert!(similarity("Parser", "Tokenomics") < 0.5);
    }

    #[test]
    fn no_groups_for_distinct_entities() {
        let ents = vec![entity("Database"), entity("Chat"), entity("Parser")];
        assert!(build_groups(&ents, DUPLICATE_DICE).is_empty());
    }
}
