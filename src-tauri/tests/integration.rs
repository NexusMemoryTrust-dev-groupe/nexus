//! Integration tests: context pipeline, memory + semantic search, graph operations.

use nexus::core::context::compressor::ContextCompressor;
use nexus::core::context::context_package::{ContextPackage, IntentType, UserIntent};
use nexus::core::context::intent_detector::IntentDetector;
use nexus::core::context::ranker::ContextRanker;
use nexus::core::context::semantic_search::SemanticSearch;
use nexus::core::entity_id::EntityId;
use nexus::core::graph::GraphQuery;
use nexus::core::graph::GraphStore;
use nexus::core::graph::GraphTraversal;
use nexus::core::graph::entity::Entity;
use nexus::core::graph::entity_types::EntityType;
use nexus::core::graph::relationship::Relationship;
use nexus::core::graph::relationship_types::RelationshipType;
use nexus::core::memory::memory_record::MemoryRecord;
use nexus::core::memory::types::MemorySource;
use nexus::storage::sqlite::SqliteGraphRepository;
use nexus::storage::sqlite::schema;
use rusqlite::Connection;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    schema::apply_migrations(&conn).unwrap();
    conn
}

// ── 1. Context Pipeline Integration ──

#[test]
fn integration_intent_detection_to_ranking() {
    let detector = IntentDetector::new();
    let intent = detector.detect("Find my notes about Rust programming");
    assert!(intent.keywords.iter().any(|k| k.contains("rust")));
    assert!(intent.keywords.iter().any(|k| k.contains("programming")));

    let mut package = ContextPackage::new(UserIntent {
        query: "Find my notes about Rust programming".to_string(),
        intent_type: IntentType::Search,
        confidence: 0.8,
        keywords: intent.keywords.clone(),
        temporal: None,
    });

    for i in 0..5 {
        let mut rec = MemoryRecord::new(
            format!("Rust note {}", i),
            format!("Content about Rust programming topic {}", i),
            "test".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        rec.confidence_score = 0.5 + (i as f64 * 0.1);
        rec.importance_score = 0.3 + (i as f64 * 0.1);
        package.memory_records.push(rec);
    }

    let ranker = ContextRanker::new();
    let ranked = ranker.rank(&package);

    assert!(
        !ranked.relevance_scores.is_empty(),
        "Should have scored entities/memories"
    );

    for i in 1..ranked.memory_records.len() {
        let score_a = ranked
            .relevance_scores
            .get(&ranked.memory_records[i - 1].id.to_string())
            .unwrap_or(&0.0);
        let score_b = ranked
            .relevance_scores
            .get(&ranked.memory_records[i].id.to_string())
            .unwrap_or(&0.0);
        assert!(
            score_a >= score_b,
            "Ranking not sorted: {} < {}",
            score_a,
            score_b
        );
    }
}

#[test]
fn integration_compressor_limits_tokens() {
    let compressor = ContextCompressor::new();

    let mut package = ContextPackage::new(UserIntent {
        query: "test".to_string(),
        intent_type: IntentType::Search,
        confidence: 0.8,
        keywords: vec!["test".to_string()],
        temporal: None,
    });

    for i in 0..20 {
        let rec = MemoryRecord::new(
            format!("Memory {}", i),
            format!(
                "Content for memory number {} with extra text to fill tokens",
                i
            ),
            "test".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        package.memory_records.push(rec);
    }

    let compressed = compressor
        .compress(&package, 200, ContextCompressor::DEFAULT_MIN_RELEVANCE)
        .unwrap();
    assert!(
        compressed.memory_records.len() <= 20,
        "Should compress: got {} records",
        compressed.memory_records.len()
    );
}

// ── 2. Memory + Semantic Search Integration ──

#[test]
fn integration_memory_store_and_semantic_search() {
    let conn = setup_db();
    let search = SemanticSearch::new(conn).unwrap();

    let id1 = EntityId::new();
    let id2 = EntityId::new();
    let id3 = EntityId::new();

    search
        .store_fingerprint(&id1, "Rust programming language fundamentals")
        .unwrap();
    search
        .store_fingerprint(&id2, "Python web development with Django")
        .unwrap();
    search
        .store_fingerprint(&id3, "Rust ownership and borrowing")
        .unwrap();

    let results = search.search("rust ownership borrowing", 10).unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].0, id3, "Most similar should be Rust ownership");
}

#[test]
fn integration_search_after_delete() {
    let conn = setup_db();
    let search = SemanticSearch::new(conn).unwrap();

    let id1 = EntityId::new();
    let id2 = EntityId::new();
    search
        .store_fingerprint(&id1, "Machine learning algorithms")
        .unwrap();
    search
        .store_fingerprint(&id2, "Deep learning neural networks")
        .unwrap();
    assert_eq!(search.count().unwrap(), 2);

    search.delete_fingerprint(&id1).unwrap();
    assert_eq!(search.count().unwrap(), 1);

    let results = search.search("machine learning", 10).unwrap();
    assert!(!results.iter().any(|(id, _)| *id == id1));
}

// ── 3. Graph Operations Integration ──

#[test]
fn integration_graph_entity_relationship_search() {
    let conn = setup_db();
    let repo = SqliteGraphRepository::new(conn).unwrap();

    let e1 = Entity::new(
        EntityType::Technology,
        "Rust".to_string(),
        "Systems programming language".to_string(),
    );
    let e2 = Entity::new(
        EntityType::Task,
        "Systems Programming".to_string(),
        "Low-level programming".to_string(),
    );
    let e3 = Entity::new(
        EntityType::Task,
        "Memory Safety".to_string(),
        "Prevent memory bugs".to_string(),
    );

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        repo.add_entity(&e1).await.unwrap();
        repo.add_entity(&e2).await.unwrap();
        repo.add_entity(&e3).await.unwrap();

        let r1 = Relationship::new(
            e1.id.clone(),
            e2.id.clone(),
            RelationshipType::RelatedTo,
            0.9,
        )
        .unwrap();
        let r2 = Relationship::new(
            e1.id.clone(),
            e3.id.clone(),
            RelationshipType::RelatedTo,
            0.8,
        )
        .unwrap();
        repo.add_relationship(&r1).await.unwrap();
        repo.add_relationship(&r2).await.unwrap();

        let found = repo.search_entities("Rust").await.unwrap();
        assert!(!found.is_empty());
        assert!(found.iter().any(|e| e.title == "Rust"));

        let rels = repo.get_entity_relationships(&e1.id).await.unwrap();
        assert_eq!(rels.len(), 2, "Rust should have 2 relationships");

        let density = repo.get_knowledge_density(&e1.id).await.unwrap();
        assert!(density >= 0.0, "Density should be >= 0");
    });
}

#[test]
fn integration_graph_traversal() {
    let conn = setup_db();
    let repo = SqliteGraphRepository::new(conn).unwrap();

    let e1 = Entity::new(EntityType::Task, "A".to_string(), "Node A".to_string());
    let e2 = Entity::new(EntityType::Task, "B".to_string(), "Node B".to_string());
    let e3 = Entity::new(EntityType::Task, "C".to_string(), "Node C".to_string());

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        repo.add_entity(&e1).await.unwrap();
        repo.add_entity(&e2).await.unwrap();
        repo.add_entity(&e3).await.unwrap();

        let r1 = Relationship::new(
            e1.id.clone(),
            e2.id.clone(),
            RelationshipType::RelatedTo,
            1.0,
        )
        .unwrap();
        let r2 = Relationship::new(
            e2.id.clone(),
            e3.id.clone(),
            RelationshipType::RelatedTo,
            1.0,
        )
        .unwrap();
        repo.add_relationship(&r1).await.unwrap();
        repo.add_relationship(&r2).await.unwrap();

        let path = repo.find_path(&e1.id, &e3.id, 5).await.unwrap();
        assert!(path.is_some(), "Path A→C should exist");
        let path = path.unwrap();
        assert_eq!(path.len(), 3, "Path should have 3 nodes: A, B, C");
        assert_eq!(path[0], e1.id);
        assert_eq!(path[1], e2.id);
        assert_eq!(path[2], e3.id);

        let dist = repo.get_distance(&e1.id, &e3.id).await.unwrap();
        assert_eq!(dist, Some(2));
    });
}

// ── 4. Full Pipeline Integration ──

#[test]
fn integration_full_memory_lifecycle() {
    let conn = setup_db();
    let search = SemanticSearch::new(conn).unwrap();

    let memories: Vec<(EntityId, &str)> = vec![
        (EntityId::new(), "Тайм-менеджмент и продуктивность"),
        (EntityId::new(), "Программирование на Rust для начинающих"),
        (EntityId::new(), "Машинное обучение и нейросети"),
        (EntityId::new(), "Управление проектами в Agile"),
    ];

    for (id, text) in &memories {
        search.store_fingerprint(id, text).unwrap();
    }

    assert_eq!(search.count().unwrap(), 4);

    let results = search.search("нейросети машинное обучение", 10).unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].0, memories[2].0, "Should find ML entry first");

    let results = search.search("Rust программирование", 10).unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].0, memories[1].0, "Should find Rust entry first");

    search.delete_fingerprint(&memories[0].0).unwrap();
    assert_eq!(search.count().unwrap(), 3);

    let results = search.search("тайм-менеджмент", 10).unwrap();
    assert!(
        !results.iter().any(|(id, _)| *id == memories[0].0),
        "Deleted memory should not appear in search"
    );
}
