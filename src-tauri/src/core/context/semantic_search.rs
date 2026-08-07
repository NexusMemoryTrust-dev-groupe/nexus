use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::core::entity_id::EntityId;
use crate::core::result::{AppError, Result};
use crate::storage::sqlite::schema;

// ═══════════════════════════════════════════════════════════════
//  Constants
// ═══════════════════════════════════════════════════════════════

/// Maximum text length for embedding (8 KB).
/// Prevents OOM from oversized inputs. Text is truncated to this limit.
const MAX_EMBEDDING_TEXT_LEN: usize = 8192;

/// Maximum number of entries in the embedding cache.
const EMBEDDING_CACHE_CAPACITY: usize = 1024;

/// Minimum interval between search requests per instance (rate limiting).
const SEARCH_RATE_LIMIT: Duration = Duration::from_millis(50);

/// Maximum number of search results allowed.
const MAX_SEARCH_LIMIT: u32 = 1000;

// ═══════════════════════════════════════════════════════════════
//  Types
// ═══════════════════════════════════════════════════════════════

/// A semantic fingerprint for a memory — stores embedding vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFingerprint {
    pub memory_id: EntityId,
    pub embedding: Vec<f32>,
    pub created_at: String,
}

/// LRU cache for embedding vectors. Key = text, Value = embedding.
struct EmbeddingCache {
    capacity: usize,
    order: VecDeque<String>,
    map: HashMap<String, Vec<f32>>,
}

impl EmbeddingCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            order: VecDeque::new(),
            map: HashMap::new(),
        }
    }

    fn get(&mut self, key: &str) -> Option<&Vec<f32>> {
        if self.map.contains_key(key) {
            // Move to back (most recently used)
            self.order.retain(|k| k != key);
            self.order.push_back(key.to_string());
            self.map.get(key)
        } else {
            None
        }
    }

    fn put(&mut self, key: String, value: Vec<f32>) {
        if self.map.contains_key(&key) {
            // Update existing
            self.order.retain(|k| k != &key);
            self.order.push_back(key.clone());
            self.map.insert(key, value);
        } else {
            // Evict LRU if at capacity
            if self.map.len() >= self.capacity
                && let Some(oldest) = self.order.pop_front()
            {
                self.map.remove(&oldest);
            }
            self.order.push_back(key.clone());
            self.map.insert(key, value);
        }
    }

    fn clear(&mut self) {
        self.order.clear();
        self.map.clear();
    }

    fn len(&self) -> usize {
        self.map.len()
    }
}

/// Embedding backend — real ONNX model or deterministic fallback.
///
/// `TextEmbedding` is over a kilobyte on the stack, so it is boxed: otherwise
/// every `Fallback` value would carry that dead weight too.
enum EmbeddingBackend {
    Real(Box<TextEmbedding>),
    /// Fallback when ONNX model fails to load (no AVX2, network error, etc.).
    /// Uses deterministic hash-based vectors — not semantically meaningful,
    /// but allows the system to function without crashing.
    Fallback,
}

impl EmbeddingBackend {
    fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        match self {
            EmbeddingBackend::Real(model) => {
                let embeddings = model
                    .embed(vec![text], None)
                    .map_err(|e| AppError::Internal(format!("Embedding failed: {}", e)))?;
                embeddings
                    .into_iter()
                    .next()
                    .ok_or_else(|| AppError::Internal("No embedding returned".to_string()))
            }
            EmbeddingBackend::Fallback => Ok(Self::fallback_embed(text)),
        }
    }

    fn is_real(&self) -> bool {
        matches!(self, EmbeddingBackend::Real(_))
    }

    /// Deterministic fallback embedding based on hash.
    /// NOT semantically meaningful — only produces consistent vectors
    /// for identical inputs. Used when ONNX model is unavailable.
    fn fallback_embed(text: &str) -> Vec<f32> {
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let hash = hasher.finish();

        let mut embedding = Vec::with_capacity(384);
        for i in 0..384 {
            let mut h = DefaultHasher::new();
            hash.hash(&mut h);
            (i as u64).hash(&mut h);
            let val = (h.finish() as f32 / u64::MAX as f32) * 2.0 - 1.0;
            embedding.push(val);
        }

        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }
        embedding
    }
}

/// Where the ONNX embedding model is cached on disk.
///
/// fastembed's default is `.fastembed_cache` resolved against the *current
/// working directory*. For an installed build that directory is inside
/// `Program Files`, which is not writable by a normal user, so the model
/// download fails and the engine silently degrades to hash-based vectors.
///
/// Anchoring the cache next to the database keeps model and data together in a
/// per-user writable location, and matches where `core::tokenizer` already
/// looks for `tokenizer.json` — so exact token counting and semantic search
/// share one download instead of two.
///
/// An *already populated* cache always wins over the per-user default, so a
/// checkout or portable install that has the model next to the working
/// directory keeps using it instead of re-downloading. `FASTEMBED_CACHE_DIR`
/// overrides everything, for CI.
fn model_cache_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("FASTEMBED_CACHE_DIR")
        && !dir.trim().is_empty()
    {
        return std::path::PathBuf::from(dir);
    }

    let per_user = crate::db::db_path()
        .parent()
        .map(|p| p.join(".fastembed_cache"));

    // Order matters only for *existing* caches; the download target is always
    // the per-user directory, which is guaranteed writable.
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Some(dir) = per_user.clone() {
        candidates.push(dir);
    }
    candidates.push(std::path::PathBuf::from(".fastembed_cache"));
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join(".fastembed_cache"));
    }

    if let Some(populated) = candidates.iter().find(|dir| has_downloaded_model(dir)) {
        return populated.clone();
    }

    per_user.unwrap_or_else(|| std::path::PathBuf::from(".fastembed_cache"))
}

/// True when `root` holds a HuggingFace-style model download
/// (`models--<org>--<name>/…`). Cheap: one directory listing, no recursion.
fn has_downloaded_model(root: &std::path::Path) -> bool {
    let Ok(entries) = std::fs::read_dir(root) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry.file_name().to_string_lossy().starts_with("models--") && entry.path().is_dir()
    })
}

// ═══════════════════════════════════════════════════════════════
//  SemanticSearch
// ═══════════════════════════════════════════════════════════════

/// Semantic search engine using embedding vectors.
///
/// Production path: ONNX model (AllMiniLML6V2, 384-dim) with LRU caching.
/// Fallback path: deterministic hash-based vectors if model unavailable.
pub struct SemanticSearch {
    conn: Mutex<Connection>,
    backend: Mutex<EmbeddingBackend>,
    cache: Mutex<EmbeddingCache>,
    last_search: Mutex<Instant>,
}

impl SemanticSearch {
    /// Create a new semantic search instance.
    ///
    /// Attempts to load the ONNX embedding model. If it fails (no AVX2,
    /// network error, model not downloaded), falls back to deterministic
    /// hash-based vectors with a warning log.
    pub fn new(conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        schema::apply_migrations(&conn)?;

        let cache_dir = model_cache_dir();
        let _ = std::fs::create_dir_all(&cache_dir);
        let backend = match TextEmbedding::try_new(
            TextInitOptions::new(EmbeddingModel::AllMiniLML6V2)
                .with_cache_dir(cache_dir)
                .with_show_download_progress(false),
        ) {
            Ok(model) => {
                tracing::info!("ONNX embedding model loaded successfully");
                EmbeddingBackend::Real(Box::new(model))
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to load ONNX embedding model ({}), using fallback. \
                     Semantic search will work but with reduced quality.",
                    e
                );
                EmbeddingBackend::Fallback
            }
        };

        Ok(Self {
            conn: Mutex::new(conn),
            backend: Mutex::new(backend),
            cache: Mutex::new(EmbeddingCache::new(EMBEDDING_CACHE_CAPACITY)),
            last_search: Mutex::new(Instant::now()),
        })
    }

    /// Create a new in-memory instance with fallback embeddings (for testing).
    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        schema::apply_migrations(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
            backend: Mutex::new(EmbeddingBackend::Fallback),
            cache: Mutex::new(EmbeddingCache::new(EMBEDDING_CACHE_CAPACITY)),
            last_search: Mutex::new(Instant::now()),
        })
    }

    /// Returns true if the real ONNX model is loaded (not fallback).
    pub fn is_model_loaded(&self) -> bool {
        self.backend.lock().map(|b| b.is_real()).unwrap_or(false)
    }

    /// Validate and truncate input text.
    /// Returns truncated text and whether it was truncated.
    pub fn validate_text(text: &str) -> (&str, bool) {
        if text.len() > MAX_EMBEDDING_TEXT_LEN {
            // Find a safe char boundary (don't split mid-UTF8)
            let mut end = MAX_EMBEDDING_TEXT_LEN;
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            (&text[..end], true)
        } else {
            (text, false)
        }
    }

    /// Get embedding with LRU cache.
    fn get_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // Check cache first
        {
            let mut cache = self
                .cache
                .lock()
                .map_err(|e| AppError::Internal(e.to_string()))?;
            if let Some(cached) = cache.get(text) {
                return Ok(cached.clone());
            }
        }

        // Compute embedding
        let embedding = {
            let mut backend = self
                .backend
                .lock()
                .map_err(|e| AppError::Internal(e.to_string()))?;
            backend.embed(text)?
        };

        // Store in cache
        {
            let mut cache = self
                .cache
                .lock()
                .map_err(|e| AppError::Internal(e.to_string()))?;
            cache.put(text.to_string(), embedding.clone());
        }

        Ok(embedding)
    }

    /// Public wrapper for fallback embedding (benchmarks only).
    pub fn bench_fallback_embed(text: &str) -> Vec<f32> {
        EmbeddingBackend::fallback_embed(text)
    }

    /// Compute cosine similarity between two embedding vectors.
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let mut dot_product = 0.0f64;
        let mut norm_a = 0.0f64;
        let mut norm_b = 0.0f64;

        for i in 0..a.len() {
            dot_product += a[i] as f64 * b[i] as f64;
            norm_a += a[i] as f64 * a[i] as f64;
            norm_b += b[i] as f64 * b[i] as f64;
        }

        let norm_a = norm_a.sqrt();
        let norm_b = norm_b.sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
    }

    /// Store a semantic fingerprint (embedding) for a memory.
    pub fn store_fingerprint(&self, memory_id: &EntityId, text: &str) -> Result<()> {
        let (text, _truncated) = Self::validate_text(text);
        let embedding = self.get_embedding(text)?;

        let embedding_json =
            serde_json::to_string(&embedding).map_err(|e| AppError::Internal(e.to_string()))?;
        let now = chrono::Utc::now().to_rfc3339();

        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        conn.execute(
            "INSERT OR REPLACE INTO memory_semantic_fingerprints (memory_id, keywords_json, created_at)
             VALUES (?1, ?2, ?3)",
            params![memory_id.as_str(), embedding_json, now],
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(())
    }

    /// Delete a semantic fingerprint.
    pub fn delete_fingerprint(&self, memory_id: &EntityId) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        conn.execute(
            "DELETE FROM memory_semantic_fingerprints WHERE memory_id = ?1",
            params![memory_id.as_str()],
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(())
    }

    /// Search memories by semantic similarity to a query using embeddings.
    ///
    /// Rate-limited: minimum 50ms between searches.
    /// Returns (memory_id, similarity_score) pairs sorted by similarity descending.
    pub fn search(&self, query: &str, limit: u32) -> Result<Vec<(EntityId, f64)>> {
        // Rate limiting
        {
            let mut last = self
                .last_search
                .lock()
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let elapsed = last.elapsed();
            if elapsed < SEARCH_RATE_LIMIT {
                std::thread::sleep(SEARCH_RATE_LIMIT - elapsed);
            }
            *last = Instant::now();
        }

        // Validate and clamp limit
        let limit = limit.min(MAX_SEARCH_LIMIT);
        let (query, _truncated) = Self::validate_text(query);

        let query_embedding = self.get_embedding(query)?;

        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut stmt = conn
            .prepare("SELECT memory_id, keywords_json FROM memory_semantic_fingerprints")
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let memory_id_str: String = row.get(0)?;
                let keywords_json: String = row.get(1)?;
                Ok((memory_id_str, keywords_json))
            })
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut results: Vec<(EntityId, f64)> = Vec::new();

        for row in rows {
            let (memory_id_str, embedding_json) =
                row.map_err(|e| AppError::Internal(e.to_string()))?;

            if let Ok(memory_id) = EntityId::parse(&memory_id_str)
                && let Ok(embedding) = serde_json::from_str::<Vec<f32>>(&embedding_json)
            {
                let similarity = Self::cosine_similarity(&query_embedding, &embedding);
                if similarity > 0.0 {
                    results.push((memory_id, similarity));
                }
            }
        }

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit as usize);

        Ok(results)
    }

    /// Get all fingerprints (for rebuilding).
    pub fn list_fingerprints(&self) -> Result<Vec<SemanticFingerprint>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT memory_id, keywords_json, created_at FROM memory_semantic_fingerprints",
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let memory_id_str: String = row.get(0)?;
                let keywords_json: String = row.get(1)?;
                let created_at: String = row.get(2)?;
                Ok((memory_id_str, keywords_json, created_at))
            })
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut fingerprints = Vec::new();

        for row in rows {
            let (memory_id_str, embedding_json, created_at) =
                row.map_err(|e| AppError::Internal(e.to_string()))?;

            if let Ok(memory_id) = EntityId::parse(&memory_id_str)
                && let Ok(embedding) = serde_json::from_str::<Vec<f32>>(&embedding_json)
            {
                fingerprints.push(SemanticFingerprint {
                    memory_id,
                    embedding,
                    created_at,
                });
            }
        }

        Ok(fingerprints)
    }

    /// Count total fingerprints.
    pub fn count(&self) -> Result<u64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM memory_semantic_fingerprints",
                [],
                |row| row.get(0),
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(count as u64)
    }

    /// Clear the embedding cache.
    pub fn clear_cache(&self) -> Result<()> {
        let mut cache = self
            .cache
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        cache.clear();
        Ok(())
    }

    /// Get cache statistics.
    pub fn cache_stats(&self) -> Result<(usize, usize)> {
        let cache = self
            .cache
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok((cache.len(), cache.capacity))
    }

    // ═══════════════════════════════════════════════════════════
    //  Document fingerprints (RAG corpus)
    // ═══════════════════════════════════════════════════════════

    /// Store a semantic fingerprint for a project document.
    /// Documents live in `document_fingerprints`, separate from memories.
    pub fn store_document_fingerprint(&self, document_id: &EntityId, text: &str) -> Result<()> {
        let (text, _truncated) = Self::validate_text(text);
        let embedding = self.get_embedding(text)?;
        let embedding_json =
            serde_json::to_string(&embedding).map_err(|e| AppError::Internal(e.to_string()))?;
        let now = chrono::Utc::now().to_rfc3339();

        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        conn.execute(
            "INSERT OR REPLACE INTO document_fingerprints (document_id, keywords_json, created_at)
             VALUES (?1, ?2, ?3)",
            params![document_id.as_str(), embedding_json, now],
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(())
    }

    /// Delete a project document's fingerprint.
    pub fn delete_document_fingerprint(&self, document_id: &EntityId) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        conn.execute(
            "DELETE FROM document_fingerprints WHERE document_id = ?1",
            params![document_id.as_str()],
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    /// Search project documents by semantic similarity to a query.
    /// Returns (document_id, similarity) pairs sorted descending.
    pub fn search_documents(&self, query: &str, limit: u32) -> Result<Vec<(EntityId, f64)>> {
        // Rate limiting
        {
            let mut last = self
                .last_search
                .lock()
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let elapsed = last.elapsed();
            if elapsed < SEARCH_RATE_LIMIT {
                std::thread::sleep(SEARCH_RATE_LIMIT - elapsed);
            }
            *last = Instant::now();
        }

        let limit = limit.min(MAX_SEARCH_LIMIT);
        let (query, _truncated) = Self::validate_text(query);
        let query_embedding = self.get_embedding(query)?;

        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut stmt = conn
            .prepare("SELECT document_id, keywords_json FROM document_fingerprints")
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let id_str: String = row.get(0)?;
                let keywords_json: String = row.get(1)?;
                Ok((id_str, keywords_json))
            })
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut results: Vec<(EntityId, f64)> = Vec::new();
        for row in rows {
            let (id_str, embedding_json) = row.map_err(|e| AppError::Internal(e.to_string()))?;
            if let Ok(doc_id) = EntityId::parse(&id_str)
                && let Ok(embedding) = serde_json::from_str::<Vec<f32>>(&embedding_json)
            {
                let similarity = Self::cosine_similarity(&query_embedding, &embedding);
                if similarity > 0.0 {
                    results.push((doc_id, similarity));
                }
            }
        }

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit as usize);
        Ok(results)
    }

    /// Count document fingerprints.
    pub fn count_documents(&self) -> Result<u64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM document_fingerprints", [], |row| {
                row.get(0)
            })
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(count.max(0) as u64)
    }
}

// ═══════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Cosine Similarity ──

    #[test]
    fn cosine_similarity_identical() {
        let a = vec![0.5, 0.5, 0.0, 0.0];
        let b = vec![0.5, 0.5, 0.0, 0.0];
        let sim = SemanticSearch::cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-10, "Expected ~1.0, got {}", sim);
    }

    #[test]
    fn cosine_similarity_different() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = SemanticSearch::cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 1e-10, "Expected ~0.0, got {}", sim);
    }

    #[test]
    fn cosine_similarity_partial() {
        let a = vec![0.5, 0.5, 0.0];
        let b = vec![0.5, 0.0, 0.5];
        let sim = SemanticSearch::cosine_similarity(&a, &b);
        assert!(sim > 0.0 && sim < 1.0);
    }

    #[test]
    fn cosine_similarity_empty() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        let sim = SemanticSearch::cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cosine_similarity_different_lengths() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = SemanticSearch::cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < f64::EPSILON);
    }

    // ── Fallback Embedding ──

    #[test]
    fn fallback_embedding_deterministic() {
        let a = EmbeddingBackend::fallback_embed("hello world");
        let b = EmbeddingBackend::fallback_embed("hello world");
        assert_eq!(a, b, "Same text should produce same embedding");
        assert_eq!(a.len(), 384, "Embedding should be 384-dim");
    }

    #[test]
    fn fallback_embedding_unit_norm() {
        let emb = EmbeddingBackend::fallback_embed("test text");
        let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-5,
            "Embedding should be unit-normalized, got norm={}",
            norm
        );
    }

    // ── LRU Cache ──

    #[test]
    fn lru_cache_basic() {
        let mut cache = EmbeddingCache::new(3);
        cache.put("a".into(), vec![1.0]);
        cache.put("b".into(), vec![2.0]);
        cache.put("c".into(), vec![3.0]);

        assert!(cache.get("a").is_some());
        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn lru_cache_eviction() {
        let mut cache = EmbeddingCache::new(2);
        cache.put("a".into(), vec![1.0]);
        cache.put("b".into(), vec![2.0]);
        // "a" is LRU now, adding "c" should evict "a"
        cache.put("c".into(), vec![3.0]);

        assert!(cache.get("a").is_none(), "a should be evicted");
        assert!(cache.get("b").is_some());
        assert!(cache.get("c").is_some());
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn lru_cache_access_refreshes() {
        let mut cache = EmbeddingCache::new(2);
        cache.put("a".into(), vec![1.0]);
        cache.put("b".into(), vec![2.0]);
        // Access "a" to make it most recently used
        cache.get("a");
        // Now "b" is LRU, adding "c" should evict "b"
        cache.put("c".into(), vec![3.0]);

        assert!(cache.get("a").is_some(), "a should survive (was accessed)");
        assert!(cache.get("b").is_none(), "b should be evicted");
        assert!(cache.get("c").is_some());
    }

    #[test]
    fn lru_cache_clear() {
        let mut cache = EmbeddingCache::new(3);
        cache.put("a".into(), vec![1.0]);
        cache.put("b".into(), vec![2.0]);
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.get("a").is_none());
    }

    #[test]
    fn lru_cache_update_existing() {
        let mut cache = EmbeddingCache::new(2);
        cache.put("a".into(), vec![1.0]);
        cache.put("a".into(), vec![99.0]);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get("a").unwrap(), &vec![99.0]);
    }

    // ── Text Validation ──

    #[test]
    fn validate_text_short() {
        let (text, truncated) = SemanticSearch::validate_text("hello");
        assert_eq!(text, "hello");
        assert!(!truncated);
    }

    #[test]
    fn validate_text_exact_limit() {
        let text = "x".repeat(MAX_EMBEDDING_TEXT_LEN);
        let (result, truncated) = SemanticSearch::validate_text(&text);
        assert_eq!(result.len(), MAX_EMBEDDING_TEXT_LEN);
        assert!(!truncated);
    }

    #[test]
    fn validate_text_over_limit() {
        let text = "x".repeat(MAX_EMBEDDING_TEXT_LEN + 100);
        let (result, truncated) = SemanticSearch::validate_text(&text);
        assert_eq!(result.len(), MAX_EMBEDDING_TEXT_LEN);
        assert!(truncated);
    }

    #[test]
    fn validate_text_utf8_safe_truncation() {
        // 4-byte UTF-8 char (emoji) right at the boundary
        let text = "a".repeat(MAX_EMBEDDING_TEXT_LEN - 2) + "🚀";
        let (result, _truncated) = SemanticSearch::validate_text(&text);
        // Should not panic and should end at a valid char boundary
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }

    // ── Store/Search Integration ──

    #[test]
    fn store_and_search() {
        let search = SemanticSearch::new_in_memory().unwrap();
        let id1 = EntityId::new();
        let id2 = EntityId::new();

        search
            .store_fingerprint(&id1, "rust programming language")
            .unwrap();
        search
            .store_fingerprint(&id2, "python web development")
            .unwrap();

        let results = search.search("rust programming", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].0, id1, "Rust entry should rank first");
    }

    #[test]
    fn search_respects_limit() {
        let search = SemanticSearch::new_in_memory().unwrap();
        for i in 0..10 {
            let id = EntityId::new();
            search
                .store_fingerprint(&id, &format!("document about topic number {}", i))
                .unwrap();
        }

        let results = search.search("document topic", 5).unwrap();
        assert!(results.len() <= 5);
    }

    #[test]
    fn search_clamps_limit() {
        let search = SemanticSearch::new_in_memory().unwrap();
        let id = EntityId::new();
        search.store_fingerprint(&id, "test").unwrap();

        // Request 10000 but should be clamped to MAX_SEARCH_LIMIT
        let results = search.search("test", 10000).unwrap();
        assert!(results.len() <= MAX_SEARCH_LIMIT as usize);
    }

    #[test]
    fn delete_fingerprint() {
        let search = SemanticSearch::new_in_memory().unwrap();
        let id = EntityId::new();
        search.store_fingerprint(&id, "test content").unwrap();
        assert_eq!(search.count().unwrap(), 1);

        search.delete_fingerprint(&id).unwrap();
        assert_eq!(search.count().unwrap(), 0);
    }

    #[test]
    fn count_fingerprints() {
        let search = SemanticSearch::new_in_memory().unwrap();
        assert_eq!(search.count().unwrap(), 0);

        let id1 = EntityId::new();
        let id2 = EntityId::new();
        search.store_fingerprint(&id1, "content one").unwrap();
        search.store_fingerprint(&id2, "content two").unwrap();
        assert_eq!(search.count().unwrap(), 2);
    }

    #[test]
    fn list_fingerprints() {
        let search = SemanticSearch::new_in_memory().unwrap();
        let id = EntityId::new();
        search.store_fingerprint(&id, "test content").unwrap();

        let fingerprints = search.list_fingerprints().unwrap();
        assert_eq!(fingerprints.len(), 1);
        assert_eq!(fingerprints[0].memory_id, id);
        assert_eq!(fingerprints[0].embedding.len(), 384);
    }

    // ── Caching ──

    #[test]
    fn cache_reuses_embedding() {
        let search = SemanticSearch::new_in_memory().unwrap();
        let (stats_before, _) = search.cache_stats().unwrap();

        // Store same text twice — second should hit cache
        let id1 = EntityId::new();
        let id2 = EntityId::new();
        search.store_fingerprint(&id1, "cached text").unwrap();
        search.store_fingerprint(&id2, "cached text").unwrap();

        let (stats_after, _) = search.cache_stats().unwrap();
        assert_eq!(
            stats_after,
            stats_before + 1,
            "Cache should have exactly 1 entry (same text reused)"
        );
    }

    #[test]
    fn clear_cache() {
        let search = SemanticSearch::new_in_memory().unwrap();
        let id = EntityId::new();
        search.store_fingerprint(&id, "some text").unwrap();
        assert!((search.cache_stats().unwrap().0) > 0);

        search.clear_cache().unwrap();
        let (count, _) = search.cache_stats().unwrap();
        assert_eq!(count, 0);
    }

    // ── Model Status ──

    #[test]
    fn in_memory_is_not_real_model() {
        let search = SemanticSearch::new_in_memory().unwrap();
        assert!(
            !search.is_model_loaded(),
            "In-memory instance should use fallback"
        );
    }

    // ── Rate Limiting (basic check — can't test timing precisely) ──

    #[test]
    fn search_does_not_panic_with_zero_limit() {
        let search = SemanticSearch::new_in_memory().unwrap();
        let id = EntityId::new();
        search.store_fingerprint(&id, "test").unwrap();

        let results = search.search("test", 0).unwrap();
        assert!(results.is_empty());
    }
}
