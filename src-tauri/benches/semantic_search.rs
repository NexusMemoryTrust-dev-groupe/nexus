use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nexus::core::entity_id::EntityId;
use nexus::core::context::semantic_search::SemanticSearch;

// ═══════════════════════════════════════════════════════════════
//  Benchmark: Fallback Embedding
// ═══════════════════════════════════════════════════════════════

fn bench_fallback_embed(c: &mut Criterion) {
    let mut group = c.benchmark_group("fallback_embed");

    let short = "Rust programming language";
    let medium = "Machine learning neural networks for natural language processing tasks";
    let long = "The quick brown fox jumps over the lazy dog. This is a longer text to benchmark \
                embedding performance with realistic content lengths that would appear in a \
                memory management system like Nexus. Adding more words to make it realistic.";

    group.bench_function("short_25chars", |b| {
        b.iter(|| SemanticSearch::bench_fallback_embed(black_box(short)))
    });

    group.bench_function("medium_70chars", |b| {
        b.iter(|| SemanticSearch::bench_fallback_embed(black_box(medium)))
    });

    group.bench_function("long_300chars", |b| {
        b.iter(|| SemanticSearch::bench_fallback_embed(black_box(long)))
    });

    group.finish();
}

// ═══════════════════════════════════════════════════════════════
//  Benchmark: Cosine Similarity
// ═══════════════════════════════════════════════════════════════

fn bench_cosine_similarity(c: &mut Criterion) {
    let mut group = c.benchmark_group("cosine_similarity");

    // Generate two random 384-dim unit vectors
    let a: Vec<f32> = (0..384).map(|i| (i as f32 * 0.01).sin()).collect();
    let b_vec: Vec<f32> = (0..384).map(|i| (i as f32 * 0.02).cos()).collect();

    group.bench_function("384dim", |bench| {
        let a_ref = &a;
        let b_ref = &b_vec;
        bench.iter(|| {
            SemanticSearch::cosine_similarity(a_ref, b_ref)
        })
    });

    // Orthogonal vectors
    let mut c_vec = vec![0.0f32; 384];
    c_vec[0] = 1.0;
    group.bench_function("orthogonal_384dim", |bench| {
        let a_ref = &a;
        let c_ref = &c_vec;
        bench.iter(|| {
            SemanticSearch::cosine_similarity(a_ref, c_ref)
        })
    });

    group.finish();
}

// ═══════════════════════════════════════════════════════════════
//  Benchmark: LRU Cache
// ═══════════════════════════════════════════════════════════════

fn bench_lru_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("lru_cache");

    group.bench_function("cache_hit_1024", |b| {
        let search = SemanticSearch::new_in_memory().unwrap();
        // Pre-populate cache
        for i in 0..1024 {
            let id = EntityId::new();
            search.store_fingerprint(&id, &format!("cached text number {}", i)).unwrap();
        }
        b.iter(|| {
            search.search(black_box("cached text number 500"), 10).unwrap()
        })
    });

    group.bench_function("cache_miss_new_text", |b| {
        let search = SemanticSearch::new_in_memory().unwrap();
        let id = EntityId::new();
        search.store_fingerprint(&id, "existing").unwrap();
        b.iter(|| {
            search.search(black_box("completely new unique query text"), 10).unwrap()
        })
    });

    group.finish();
}

// ═══════════════════════════════════════════════════════════════
//  Benchmark: Full Store+Search Pipeline
// ═══════════════════════════════════════════════════════════════

fn bench_store_search_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline");

    group.bench_function("store_100_memories", |b| {
        b.iter(|| {
            let search = SemanticSearch::new_in_memory().unwrap();
            for i in 0..100 {
                let id = EntityId::new();
                search.store_fingerprint(&id, &format!("Memory about topic number {} with some content", i)).unwrap();
            }
            black_box(&search);
        })
    });

    group.bench_function("search_100_memories", |b| {
        let search = SemanticSearch::new_in_memory().unwrap();
        for i in 0..100 {
            let id = EntityId::new();
            search.store_fingerprint(&id, &format!("Memory about topic number {} with some content", i)).unwrap();
        }
        b.iter(|| {
            search.search(black_box("topic number 50"), 10).unwrap()
        })
    });

    group.finish();
}

// ═══════════════════════════════════════════════════════════════
//  Benchmark: Text Validation
// ═══════════════════════════════════════════════════════════════

fn bench_text_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_validation");

    let short = "hello";
    let exact_8k = "x".repeat(8192);
    let over_8k = "x".repeat(16384);

    group.bench_function("short_5chars", |b| {
        b.iter(|| SemanticSearch::validate_text(black_box(short)))
    });

    group.bench_function("exact_8192chars", |b| {
        b.iter(|| SemanticSearch::validate_text(black_box(&exact_8k)))
    });

    group.bench_function("over_16384chars", |b| {
        b.iter(|| SemanticSearch::validate_text(black_box(&over_8k)))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_fallback_embed,
    bench_cosine_similarity,
    bench_lru_cache,
    bench_store_search_pipeline,
    bench_text_validation,
);
criterion_main!(benches);
