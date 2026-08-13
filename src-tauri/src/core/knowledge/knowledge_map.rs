//! Knowledge Map (Knowledge Navigation 2.0) — карта интеллектуального
//! пространства вокруг сущности.
//!
//! Пользователь не смотрит на граф «сверху» — он пилотирует. Вокруг текущей
//! цели Nexus раскладывает знание концентрическими кольцами:
//!
//! ```text
//!        HISTORICAL         ← старые версии, superseded, прошлые решения
//!      SUPPORTING           ← стратегия, принципы, скиллы, смежные кольца
//!    RELEVANT               ← факты, процедуры, решения по теме
//!  MISSION                  ← что в работе прямо сейчас (Working/Episodic)
//!    ● CURRENT ENTITY
//! ```
//!
//! Чистые функции здесь: распределение по кольцам, сортировка внутри кольца,
//! рендер. Сборка карты из репозиториев — в командном слое.

use serde::{Deserialize, Serialize};

/// Кольцо карты: насколько близко знание к текущей миссии.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MapRing {
    /// Что в работе прямо сейчас (Working / Episodic).
    Mission,
    /// Факты и процедуры по теме (Semantic / Procedural).
    Relevant,
    /// Принципы, решения, скиллы, смежное (Decision / Strategic / skills).
    Supporting,
    /// Устаревшее: superseded, прошлые версии, отменённые решения.
    Historical,
}

impl MapRing {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mission => "mission",
            Self::Relevant => "relevant",
            Self::Supporting => "supporting",
            Self::Historical => "historical",
        }
    }

    /// Порядок колец: от центра к периферии.
    pub fn order(&self) -> u8 {
        match self {
            Self::Mission => 0,
            Self::Relevant => 1,
            Self::Supporting => 2,
            Self::Historical => 3,
        }
    }
}

/// Один элемент карты.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MapItem {
    pub ring: MapRing,
    /// entity | memory | decision | conflict | skill | version
    pub kind: String,
    pub id: String,
    pub title: String,
    /// Слой памяти / тип сущности (для фильтрации в UI).
    pub layer: String,
    /// Важность/вес для сортировки внутри кольца.
    pub weight: f64,
    /// Кому принадлежит элемент (агент/пользователь) — если известно.
    pub owner: String,
}

/// Карта знания вокруг одной сущности.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KnowledgeMap {
    pub entity_id: String,
    pub entity_title: String,
    pub items: Vec<MapItem>,
}

impl KnowledgeMap {
    /// Элементы конкретного кольца, отсортированные по весу (убывание).
    pub fn ring(&self, ring: MapRing) -> Vec<&MapItem> {
        let mut v: Vec<&MapItem> = self.items.iter().filter(|i| i.ring == ring).collect();
        v.sort_by(|a, b| {
            b.weight
                .partial_cmp(&a.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        v
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Общее число элементов на карте.
    pub fn len(&self) -> usize {
        self.items.len()
    }
}

/// Распределить память по кольцу на основе когнитивного слоя.
///
/// Working/Episodic — текущая миссия; Semantic/Procedural — релевантное знание;
/// Decision/Strategic — поддерживающее. Superseded-записи — в историю.
pub fn ring_for_layer(layer: &str, superseded: bool) -> MapRing {
    if superseded {
        return MapRing::Historical;
    }
    match layer {
        "Working" | "Episodic" => MapRing::Mission,
        "Semantic" | "Procedural" => MapRing::Relevant,
        "Decision" | "Strategic" => MapRing::Supporting,
        _ => MapRing::Relevant,
    }
}

/// Название кольца для UI (RU-подписи подпирают навигацию по кругам).
pub fn ring_label(ring: MapRing) -> &'static str {
    match ring {
        MapRing::Mission => "Current Mission",
        MapRing::Relevant => "Relevant Knowledge",
        MapRing::Supporting => "Supporting Knowledge",
        MapRing::Historical => "Historical Knowledge",
    }
}

/// Человекочитаемый рендер карты (круги от центра к периферии).
pub fn render_map(map: &KnowledgeMap) -> String {
    if map.is_empty() {
        return format!(
            "Knowledge Map: \"{}\" has no connected knowledge yet.",
            map.entity_title
        );
    }
    let mut out = String::with_capacity(512);
    out.push_str(&format!(
        "Knowledge Map: {} ({})\n",
        map.entity_title, map.entity_id
    ));
    for ring in [
        MapRing::Mission,
        MapRing::Relevant,
        MapRing::Supporting,
        MapRing::Historical,
    ] {
        let items = map.ring(ring);
        if items.is_empty() {
            continue;
        }
        out.push_str(&format!("  {}:\n", ring_label(ring)));
        for item in items.iter().take(8) {
            out.push_str(&format!(
                "    {} — {} ({})",
                item.title, item.kind, item.layer
            ));
            if !item.owner.is_empty() {
                out.push_str(&format!(" [{}]", item.owner));
            }
            out.push('\n');
        }
        if items.len() > 8 {
            out.push_str(&format!("    … +{} more\n", items.len() - 8));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(ring: MapRing, kind: &str, id: &str, title: &str, layer: &str, weight: f64) -> MapItem {
        MapItem {
            ring,
            kind: kind.to_string(),
            id: id.to_string(),
            title: title.to_string(),
            layer: layer.to_string(),
            weight,
            owner: String::new(),
        }
    }

    #[test]
    fn ring_for_layer_mapping() {
        assert_eq!(ring_for_layer("Working", false), MapRing::Mission);
        assert_eq!(ring_for_layer("Episodic", false), MapRing::Mission);
        assert_eq!(ring_for_layer("Semantic", false), MapRing::Relevant);
        assert_eq!(ring_for_layer("Procedural", false), MapRing::Relevant);
        assert_eq!(ring_for_layer("Decision", false), MapRing::Supporting);
        assert_eq!(ring_for_layer("Strategic", false), MapRing::Supporting);
        // Superseded всегда уходит в историю, независимо от слоя.
        assert_eq!(ring_for_layer("Working", true), MapRing::Historical);
        assert_eq!(ring_for_layer("Semantic", true), MapRing::Historical);
    }

    #[test]
    fn ring_filters_and_sorts_by_weight() {
        let map = KnowledgeMap {
            entity_id: "e1".to_string(),
            entity_title: "Auth".to_string(),
            items: vec![
                item(
                    MapRing::Relevant,
                    "memory",
                    "m1",
                    "JWT flow",
                    "Semantic",
                    0.3,
                ),
                item(
                    MapRing::Relevant,
                    "memory",
                    "m2",
                    "Refresh tokens",
                    "Procedural",
                    0.9,
                ),
                item(
                    MapRing::Mission,
                    "memory",
                    "m3",
                    "Fixing bug",
                    "Working",
                    1.0,
                ),
            ],
        };
        let relevant = map.ring(MapRing::Relevant);
        assert_eq!(relevant.len(), 2);
        assert_eq!(relevant[0].title, "Refresh tokens");
        assert_eq!(map.len(), 3);
        assert!(!map.is_empty());
    }

    #[test]
    fn ring_order_is_center_to_edge() {
        assert!(MapRing::Mission.order() < MapRing::Relevant.order());
        assert!(MapRing::Relevant.order() < MapRing::Supporting.order());
        assert!(MapRing::Supporting.order() < MapRing::Historical.order());
    }

    #[test]
    fn render_contains_rings_and_title() {
        let map = KnowledgeMap {
            entity_id: "e1".to_string(),
            entity_title: "Auth".to_string(),
            items: vec![
                item(
                    MapRing::Mission,
                    "memory",
                    "m1",
                    "Fixing bug",
                    "Working",
                    1.0,
                ),
                item(
                    MapRing::Historical,
                    "version",
                    "v1",
                    "Old approach",
                    "Superseded",
                    0.1,
                ),
            ],
        };
        let text = render_map(&map);
        assert!(text.contains("Auth"));
        assert!(text.contains("Current Mission"));
        assert!(text.contains("Historical Knowledge"));
        assert!(text.contains("Fixing bug"));
        assert!(text.contains("Old approach"));
    }

    #[test]
    fn render_empty_map() {
        let map = KnowledgeMap {
            entity_id: "e1".to_string(),
            entity_title: "Auth".to_string(),
            items: vec![],
        };
        assert!(render_map(&map).contains("no connected knowledge"));
    }

    #[test]
    fn render_caps_long_rings() {
        let mut items = Vec::new();
        for i in 0..12 {
            items.push(item(
                MapRing::Relevant,
                "memory",
                &format!("m{i}"),
                &format!("Item {i}"),
                "Semantic",
                0.5,
            ));
        }
        let map = KnowledgeMap {
            entity_id: "e1".to_string(),
            entity_title: "Auth".to_string(),
            items,
        };
        let text = render_map(&map);
        assert!(text.contains("+4 more"));
    }

    #[test]
    fn as_str_and_labels_are_stable() {
        assert_eq!(MapRing::Mission.as_str(), "mission");
        assert_eq!(MapRing::Relevant.as_str(), "relevant");
        assert_eq!(MapRing::Supporting.as_str(), "supporting");
        assert_eq!(MapRing::Historical.as_str(), "historical");
        assert_eq!(ring_label(MapRing::Mission), "Current Mission");
        assert_eq!(ring_label(MapRing::Historical), "Historical Knowledge");
    }
}
