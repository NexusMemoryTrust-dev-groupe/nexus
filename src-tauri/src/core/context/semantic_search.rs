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

/// Maximum text length embedded as a single vector (8 KB).
///
/// Longer texts are split into overlapping [`CHUNK_CHARS`] chunks and every
/// chunk is embedded separately; retrieval takes the best (max) cosine over a
/// document's chunks. This closes the index truncation gap: a `pub struct`
/// at character 30,000 of a 68 KB file is now embedded, whereas a single
/// head-truncated vector could never represent it.
const MAX_EMBEDDING_TEXT_LEN: usize = 8192;

/// Chunk size for embedding long texts, in bytes. 1024 chars is roughly the
/// 256-token window of AllMiniLML6V2: with [`CHUNK_OVERLAP`] the stride
/// (1024 − 128 = 896) stays below the model window, so every character of a
/// long document lies within the model window of *some* chunk.
const CHUNK_CHARS: usize = 1024;

/// Overlap between adjacent chunks, so symbols near a boundary are captured
/// by both neighbours instead of falling into a seam.
const CHUNK_OVERLAP: usize = 128;

/// Full source text retained per fingerprint for the lexical (substring)
/// channel. The embedding is chunked, but keyword search needs the whole
/// document; 64 KB covers essentially every real source file while bounding
/// database size.
const MAX_SOURCE_TEXT_LEN: usize = 65536;

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

/// One hybrid hit with its per-channel breakdown
/// `(entity_id, cosine, lexical, filename, total)`. Factored into a type alias
/// so public signatures stay readable and clippy's `type_complexity` stays
/// quiet.
pub type HybridBreakdownHit = (EntityId, f64, f64, f64, f64);

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

    /// Split text into overlapping embedding chunks.
    ///
    /// Short texts (≤ [`MAX_EMBEDDING_TEXT_LEN`]) are returned whole as a
    /// single chunk — chunking only kicks in when a single vector could no
    /// longer represent the document. Long texts are cut at [`CHUNK_CHARS`]
    /// byte windows with [`CHUNK_OVERLAP`] overlap, preferring line breaks as
    /// boundaries so identifiers and declarations stay intact and no symbol
    /// falls into a seam between chunks. All boundaries are UTF-8 safe.
    fn chunk_text(text: &str) -> Vec<&str> {
        if text.len() <= MAX_EMBEDDING_TEXT_LEN {
            return vec![text];
        }
        let mut chunks = Vec::new();
        let mut start = 0usize;
        while start < text.len() {
            let mut end = (start + CHUNK_CHARS).min(text.len());
            // Align to a char boundary *first*: a later window slice must
            // never panic on a multibyte character straddling the cut.
            while end > start && !text.is_char_boundary(end) {
                end -= 1;
            }
            // Back off to a line boundary when one is nearby, so a chunk
            // starts and ends on whole lines (keeps `pub struct Record` and
            // friends intact instead of splitting mid-token).
            if end < text.len() {
                let window = &text[start..end];
                let last_nl = window.rfind('\n');
                if let Some(nl) = last_nl {
                    let after_nl = nl + 1;
                    // Only use the line boundary if it doesn't gut the chunk.
                    if after_nl > CHUNK_CHARS - CHUNK_OVERLAP {
                        end = start + after_nl;
                    }
                }
            }
            chunks.push(&text[start..end]);
            if end >= text.len() {
                break;
            }
            start = end.saturating_sub(CHUNK_OVERLAP);
            // The overlap cut can land inside a multibyte char; walk forward
            // to the next char boundary so the next slice never panics.
            while start < text.len() && !text.is_char_boundary(start) {
                start += 1;
            }
            // Guard against a non-advancing loop on pathological input.
            if start >= end {
                start = end;
            }
        }
        chunks
    }

    /// Parse a stored `keywords_json` into the chunk-embedding list.
    ///
    /// New fingerprints store `Vec<Vec<f32>>` (one vector per chunk); legacy
    /// rows stored a single `Vec<f32>`. Both decode to the chunked form, so
    /// old databases keep working after the format change.
    fn parse_embeddings(json: &str) -> Option<Vec<Vec<f32>>> {
        if let Ok(chunks) = serde_json::from_str::<Vec<Vec<f32>>>(json) {
            return Some(chunks);
        }
        serde_json::from_str::<Vec<f32>>(json)
            .ok()
            .map(|single| vec![single])
    }

    /// Best cosine over a document's chunk embeddings: the document is as
    /// similar as its most relevant chunk (the one containing the matching
    /// symbol), not an average diluted across unrelated sections.
    fn best_chunk_cosine(query: &[f32], chunks: &[Vec<f32>]) -> f64 {
        chunks
            .iter()
            .map(|c| Self::cosine_similarity(query, c))
            .fold(0.0f64, f64::max)
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
    ///
    /// Long texts are chunked (see [`SemanticSearch::chunk_text`]): each chunk
    /// is embedded and all chunk vectors are stored, so symbols beyond the
    /// single-vector window remain searchable. `source_text` keeps the full
    /// text (up to [`MAX_SOURCE_TEXT_LEN`]) for the lexical channel.
    pub fn store_fingerprint(&self, memory_id: &EntityId, text: &str) -> Result<()> {
        let chunks = Self::chunk_text(text);
        let mut embeddings = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            embeddings.push(self.get_embedding(chunk)?);
        }

        let embedding_json =
            serde_json::to_string(&embeddings).map_err(|e| AppError::Internal(e.to_string()))?;
        let source_text = crate::core::text::truncate_chars(text, MAX_SOURCE_TEXT_LEN);
        let now = chrono::Utc::now().to_rfc3339();

        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        conn.execute(
            "INSERT OR REPLACE INTO memory_semantic_fingerprints
                (memory_id, keywords_json, created_at, source_text)
             VALUES (?1, ?2, ?3, ?4)",
            params![memory_id.as_str(), embedding_json, now, source_text],
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
                && let Some(chunks) = Self::parse_embeddings(&embedding_json)
            {
                let similarity = Self::best_chunk_cosine(&query_embedding, &chunks);
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
                && let Some(chunks) = Self::parse_embeddings(&embedding_json)
                && let Some(embedding) = Self::mean_embedding(&chunks)
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

    /// Mean-pool chunk embeddings into a single document vector.
    ///
    /// `SemanticFingerprint` exposes one `Vec<f32>` (it predates chunking);
    /// listing is used for rebuild/migration paths where a per-document
    /// representative is enough. Empty chunk lists yield `None`.
    fn mean_embedding(chunks: &[Vec<f32>]) -> Option<Vec<f32>> {
        let first = chunks.first()?;
        let dim = first.len();
        if dim == 0 {
            return None;
        }
        let mut sum = vec![0.0f32; dim];
        for chunk in chunks {
            if chunk.len() != dim {
                return None;
            }
            for (acc, v) in sum.iter_mut().zip(chunk) {
                *acc += v;
            }
        }
        let n = chunks.len() as f32;
        Some(sum.into_iter().map(|v| v / n).collect())
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
    /// Long texts are chunked exactly like memory fingerprints, so symbols in
    /// the tail of a large file stay searchable.
    pub fn store_document_fingerprint(&self, document_id: &EntityId, text: &str) -> Result<()> {
        let chunks = Self::chunk_text(text);
        let mut embeddings = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            embeddings.push(self.get_embedding(chunk)?);
        }
        let embedding_json =
            serde_json::to_string(&embeddings).map_err(|e| AppError::Internal(e.to_string()))?;
        let source_text = crate::core::text::truncate_chars(text, MAX_SOURCE_TEXT_LEN);
        let now = chrono::Utc::now().to_rfc3339();

        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        conn.execute(
            "INSERT OR REPLACE INTO document_fingerprints
                (document_id, keywords_json, created_at, source_text)
             VALUES (?1, ?2, ?3, ?4)",
            params![document_id.as_str(), embedding_json, now, source_text],
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
                && let Some(chunks) = Self::parse_embeddings(&embedding_json)
            {
                let similarity = Self::best_chunk_cosine(&query_embedding, &chunks);
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

// ═══════════════════════════════════════════════════════════════
//  Hybrid retrieval
// ═══════════════════════════════════════════════════════════════

/// Stop words ignored when extracting query terms for the lexical channel.
const HYBRID_STOP: &[&str] = &[
    "how", "what", "where", "when", "why", "who", "which", "the", "and", "for", "are", "was",
    "were", "does", "did", "is", "in", "on", "of", "to", "with", "that", "this", "from", "not",
    "its", "has", "had", "have", "can", "will", "would", "should", "all", "our", "your", "their",
];

/// Hybrid score weights. Chosen for code retrieval on homogeneous corpora:
/// pure cosine collapses when hundreds of files score ≈0.6 ("MUI Button" →
/// styled.spec.tsx / Link.js instead of Button.js), so the exact-path evidence
/// carries the decisive weight. The lexical fraction breaks content ties and
/// cosine stays as a secondary signal: the path is what identifies the file.
const HYBRID_W_COSINE: f64 = 0.30;
const HYBRID_W_LEXICAL: f64 = 0.20;
const HYBRID_W_FILENAME: f64 = 0.50;

/// Lowercased, punctuation-stripped query terms (3+ chars, no stop words).
fn hybrid_query_terms(query: &str) -> Vec<String> {
    query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| w.len() > 2 && !HYBRID_STOP.contains(w))
        .map(|w| w.to_string())
        .collect()
}

/// Path segments of a source text's *heading line*: splits on separators so
/// `Button.js`, `mui-material`, `src/Button` all yield comparable tokens.
///
/// Only the first line is used because that is where the identity of a record
/// lives — `index_text` puts the title (for indexed files: the relative path)
/// first, and the body is code/content where a term like "button" appears in
/// hundreds of files. Segmenting the body would make the filename channel
/// identical to the lexical one; segmenting the heading keeps it a true
/// path channel.
fn hybrid_path_segments(source_text: &str) -> Vec<String> {
    let heading = source_text.lines().next().unwrap_or(source_text);
    heading
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect()
}

/// Weighted fraction of query terms found anywhere in the source text
/// (0.0–1.0). `weights` are IDF-normalised so a rare, discriminating term
/// ("button") counts far more than corpus-wide noise ("styled", "component").
fn hybrid_lexical_score(terms: &[String], weights: &[f64], source_text: &str) -> f64 {
    if terms.is_empty() {
        return 0.0;
    }
    let total: f64 = weights.iter().sum();
    if total <= 0.0 {
        return 0.0;
    }
    let hay = source_text.to_lowercase();
    let hits: f64 = terms
        .iter()
        .zip(weights)
        .filter(|(t, _)| hay.contains(t.as_str()))
        .map(|(_, w)| w)
        .sum();
    hits / total
}

/// Stem of the heading line's basename: the file name with its last extension
/// removed (`Button.test.js` → `button.test`, `Button.js` → `button`). The
/// filename channel uses it to reward the *exact* file a query names, not just
/// any file sharing a directory segment — `Dialog.js` beats `Dialog.test.js`
/// because the latter's stem is `dialog.test`.
fn hybrid_basename_stem(source_text: &str) -> String {
    let heading = source_text.lines().next().unwrap_or(source_text);
    let basename = heading.rsplit(['/', '\\']).next().unwrap_or(heading).trim();
    basename
        .rsplit_once('.')
        .map(|(stem, _)| stem.to_lowercase())
        .unwrap_or_else(|| basename.to_lowercase())
}

/// True when the heading's basename marks a non-implementation twin: a test
/// (`Button.test.js`), a spec (`Button.spec.tsx`), or a type declaration
/// (`Button.d.ts`). Code-retrieval queries ask *how something is implemented*
/// — the implementation file is the target, so the filename channel halves
/// the evidence of its test/spec/declaration twins instead of letting them
/// outrank it.
fn hybrid_is_non_implementation_basename(source_text: &str) -> bool {
    let heading = source_text.lines().next().unwrap_or(source_text);
    let basename = heading
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(heading)
        .trim()
        .to_lowercase();
    basename.ends_with(".d.ts") || basename.contains(".test.") || basename.contains(".spec.")
}

/// Weighted fraction of query terms with filename evidence (0.0–1.0).
///
/// Two evidence levels, strongest first:
/// - the term *is* the file's stem (`Button.js` ← "button"): the query names
///   this very file — that term's weight is tripled (one segment hit plus a
///   double stem bonus);
/// - the term appears inside a path segment (`IconButton.js` ← "button",
///   `auth.py` ← "authentication"): substring matches surface related
///   variants alongside the exact file.
///
/// `Button.js` therefore outranks `Button.test.js` (stem `button.test` ≠
/// "button") and `Select.js` (no "button" evidence at all) — exactly the
/// discrimination the code-retrieval queries need.
///
/// Both substring directions require the *segment* to be at least 3 chars:
/// extension noise segments like `d` (from `.d.ts`) or `ts` must never match
/// a term by pure containment ("implemented" contains "d"), or every type
/// declaration would outrank its implementation file.
///
/// Non-implementation twins (`.test.*`, `.spec.*`, `.d.ts`) get half the
/// score: for an "how is X implemented" query the implementation must beat
/// its test/spec/declaration look-alikes even when their paths carry the
/// same evidence.
fn hybrid_filename_score(terms: &[String], weights: &[f64], source_text: &str) -> f64 {
    if terms.is_empty() {
        return 0.0;
    }
    let total: f64 = weights.iter().sum();
    if total <= 0.0 {
        return 0.0;
    }
    let segments = hybrid_path_segments(source_text);
    let stem = hybrid_basename_stem(source_text);
    let mut hits: f64 = 0.0;
    let mut stem_bonus: f64 = 0.0;
    for (t, w) in terms.iter().zip(weights) {
        let segment_match = segments.iter().any(|s| {
            s == t.as_str()
                || (s.len() >= 3 && s.contains(t.as_str()))
                || (s.len() >= 3 && t.len() >= 3 && t.contains(s.as_str()))
        });
        if segment_match {
            hits += w;
        }
        if *t == stem {
            stem_bonus += 2.0 * w;
        }
    }
    let score = ((hits + stem_bonus) / total).min(1.0);
    if hybrid_is_non_implementation_basename(source_text) {
        score * 0.5
    } else {
        score
    }
}

/// Combined hybrid score: cosine + weighted lexical coverage + weighted
/// exact-filename boost.
///
/// Two separate IDF weight vectors feed the two text channels: `weights`
/// (content IDF) for the lexical channel and `path_weights` (heading-line IDF)
/// for the filename channel. This matters on code corpora: "button" is common
/// in file bodies (lexical channel must not over-weight it) yet rare in paths
/// (the filename channel must over-weight it to lift `Button.js` above content
/// look-alikes like `Select.js`).
pub fn hybrid_combine(
    cosine: f64,
    terms: &[String],
    weights: &[f64],
    path_weights: &[f64],
    source_text: Option<&str>,
) -> f64 {
    let (lex, file) = match source_text {
        Some(text) => (
            hybrid_lexical_score(terms, weights, text),
            hybrid_filename_score(terms, path_weights, text),
        ),
        None => (0.0, 0.0),
    };
    HYBRID_W_COSINE * cosine + HYBRID_W_LEXICAL * lex + HYBRID_W_FILENAME * file
}

/// IDF weights for the query terms: `ln((1+N)/(1+df)) + 1`, normalised so the
/// weights sum to 1.0. Computed from the corpus actually scanned by
/// [`SemanticSearch::search_hybrid`], so a term present in every file ("mui",
/// "component") is nearly weightless while a rare one ("button", "dialog")
/// dominates the lexical and filename channels.
fn hybrid_idf_weights(terms: &[String], sources: &[Option<String>]) -> Vec<f64> {
    if terms.is_empty() {
        return Vec::new();
    }
    let n = sources.len() as f64;
    let mut df: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for text in sources.iter().flatten() {
        let lower = text.to_lowercase();
        for t in terms {
            if lower.contains(t.as_str()) {
                *df.entry(t.as_str()).or_insert(0) += 1;
            }
        }
    }
    let raw: Vec<f64> = terms
        .iter()
        .map(|t| {
            let d = *df.get(t.as_str()).unwrap_or(&0) as f64;
            ((1.0 + n) / (1.0 + d)).ln() + 1.0
        })
        .collect();
    let sum: f64 = raw.iter().sum();
    if sum <= 0.0 {
        vec![0.0; terms.len()]
    } else {
        raw.iter().map(|w| w / sum).collect()
    }
}

/// IDF weights for the *filename* channel, computed over heading lines only
/// (the first line of each stored `source_text` — for indexed files that is
/// the relative path). A term counts as present only when it matches an exact
/// path segment, not when it merely appears somewhere in the body: "button"
/// occurs in hundreds of MUI file bodies but only in `Button/*` paths, which
/// is precisely the rarity the filename channel must reward.
fn hybrid_path_idf_weights(terms: &[String], sources: &[Option<String>]) -> Vec<f64> {
    if terms.is_empty() {
        return Vec::new();
    }
    let n = sources.len() as f64;
    let mut df: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for text in sources.iter().flatten() {
        let segments = hybrid_path_segments(text);
        for t in terms {
            if segments.iter().any(|s| s == t.as_str()) {
                *df.entry(t.as_str()).or_insert(0) += 1;
            }
        }
    }
    let raw: Vec<f64> = terms
        .iter()
        .map(|t| {
            let d = *df.get(t.as_str()).unwrap_or(&0) as f64;
            ((1.0 + n) / (1.0 + d)).ln() + 1.0
        })
        .collect();
    let sum: f64 = raw.iter().sum();
    if sum <= 0.0 {
        vec![0.0; terms.len()]
    } else {
        raw.iter().map(|w| w / sum).collect()
    }
}

impl SemanticSearch {
    /// Hybrid retrieval over memory fingerprints.
    ///
    /// Combines the embedding cosine (semantic channel) with keyword coverage
    /// and exact-filename evidence read from the stored `source_text`. A query
    /// like "MUI Button implementation" no longer has to rely on the vector
    /// alone: the file whose path actually contains `Button` gets lifted above
    /// thematically close but wrong neighbours (StepButton, ButtonBase).
    ///
    /// Falls back to pure semantic scoring when no source text is stored
    /// (legacy fingerprints from before V30).
    pub fn search_hybrid(&self, query: &str, limit: u32) -> Result<Vec<(EntityId, f64)>> {
        Ok(self
            .hybrid_retrieve(
                query,
                limit,
                "SELECT memory_id, keywords_json, source_text FROM memory_semantic_fingerprints",
            )?
            .into_iter()
            .map(|(id, _, _, _, total)| (id, total))
            .collect())
    }

    /// Diagnostic variant of [`SemanticSearch::search_hybrid`]: same scoring,
    /// but each hit carries its per-channel breakdown
    /// `(cosine, lexical, filename, total)` so a benchmark can explain *why* a
    /// document ranks where it does.
    pub fn search_hybrid_breakdown(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<HybridBreakdownHit>> {
        self.hybrid_retrieve(
            query,
            limit,
            "SELECT memory_id, keywords_json, source_text FROM memory_semantic_fingerprints",
        )
    }

    /// Shared engine for the hybrid channels. Runs the rate-limit guard, tokenises
    /// the query, reads the whole fingerprint table (id, embedding, source_text),
    /// computes corpus-wide IDF weights for both text channels, then scores every
    /// row. Returns `(entity_id, cosine, lexical, filename, total)` sorted by
    /// `total` descending.
    fn hybrid_retrieve(
        &self,
        query: &str,
        limit: u32,
        sql: &str,
    ) -> Result<Vec<HybridBreakdownHit>> {
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
        let terms = hybrid_query_terms(query);
        let query_embedding = self.get_embedding(query)?;

        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let id_str: String = row.get(0)?;
                let keywords_json: String = row.get(1)?;
                let source_text: Option<String> = row.get(2)?;
                Ok((id_str, keywords_json, source_text))
            })
            .map_err(|e| AppError::Internal(e.to_string()))?;

        // Materialise the rows first: the lexical/filename channels need
        // corpus-wide IDF weights, which requires seeing every source_text
        // before scoring any single row.
        let mut corpus: Vec<(EntityId, Vec<Vec<f32>>, Option<String>)> = Vec::new();
        let mut sources: Vec<Option<String>> = Vec::new();
        for row in rows {
            let (id_str, embedding_json, source_text) =
                row.map_err(|e| AppError::Internal(e.to_string()))?;
            if let Ok(id) = EntityId::parse(&id_str)
                && let Some(chunks) = Self::parse_embeddings(&embedding_json)
            {
                sources.push(source_text.clone());
                corpus.push((id, chunks, source_text));
            }
        }
        let weights = hybrid_idf_weights(&terms, &sources);
        let path_weights = hybrid_path_idf_weights(&terms, &sources);

        let mut results: Vec<HybridBreakdownHit> = Vec::new();
        for (id, chunks, source_text) in corpus {
            let cosine = Self::best_chunk_cosine(&query_embedding, &chunks);
            let (lex, file) = match source_text.as_deref() {
                Some(text) => (
                    hybrid_lexical_score(&terms, &weights, text),
                    hybrid_filename_score(&terms, &path_weights, text),
                ),
                None => (0.0, 0.0),
            };
            let total =
                HYBRID_W_COSINE * cosine + HYBRID_W_LEXICAL * lex + HYBRID_W_FILENAME * file;
            if total > 0.0 {
                results.push((id, cosine, lex, file, total));
            }
        }
        results.sort_by(|a, b| b.4.partial_cmp(&a.4).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit as usize);
        Ok(results)
    }

    /// Hybrid retrieval over project document fingerprints (same mechanics as
    /// [`SemanticSearch::search_hybrid`], on the `document_fingerprints` table).
    pub fn search_documents_hybrid(&self, query: &str, limit: u32) -> Result<Vec<(EntityId, f64)>> {
        Ok(self
            .hybrid_retrieve(
                query,
                limit,
                "SELECT document_id, keywords_json, source_text FROM document_fingerprints",
            )?
            .into_iter()
            .map(|(id, _, _, _, total)| (id, total))
            .collect())
    }
}

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

    // ── Plan 3.4: resumable / incremental indexing ──

    /// Re-indexing one memory (a file changed) replaces only that memory's
    /// fingerprint: the row count stays put (INSERT OR REPLACE), no duplicates
    /// pile up and the other memories' fingerprints are left untouched.
    #[test]
    fn reindex_replaces_own_fingerprint_keeps_others() {
        let search = SemanticSearch::new_in_memory().unwrap();
        let changed = EntityId::new();
        let untouched = EntityId::new();

        search
            .store_fingerprint(&changed, "architecture v1: sqlite")
            .unwrap();
        search
            .store_fingerprint(&untouched, "architecture v1: sqlite")
            .unwrap();
        assert_eq!(search.count().unwrap(), 2);

        // The file changed: re-index it with new text.
        search
            .store_fingerprint(&changed, "architecture v2: postgres")
            .unwrap();

        assert_eq!(
            search.count().unwrap(),
            2,
            "re-indexing one memory must not grow the fingerprint table"
        );

        // The stored rows are exactly the two ids, and the untouched memory
        // keeps its *original* source text while the re-indexed one was
        // replaced. Checking the table directly is deterministic where
        // hash-based fallback similarity would be threshold-dependent.
        let conn = search.conn.lock().unwrap();
        let mut rows: Vec<(String, Option<String>)> = Vec::new();
        {
            let mut stmt = conn
                .prepare(
                    "SELECT memory_id, source_text
                     FROM memory_semantic_fingerprints
                     ORDER BY memory_id",
                )
                .unwrap();
            let mut query = stmt.query([]).unwrap();
            while let Some(row) = query.next().unwrap() {
                rows.push((row.get(0).unwrap(), row.get(1).unwrap()));
            }
        }
        drop(conn);
        assert_eq!(rows.len(), 2, "no duplicate rows may appear");

        let by_id: std::collections::HashMap<_, _> = rows.into_iter().collect();
        assert_eq!(
            by_id.get(changed.as_str()).and_then(|t| t.as_deref()),
            Some("architecture v2: postgres"),
            "re-indexed memory must carry the new text"
        );
        assert_eq!(
            by_id.get(untouched.as_str()).and_then(|t| t.as_deref()),
            Some("architecture v1: sqlite"),
            "untouched memory must keep its original fingerprint"
        );
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

    // ── Hybrid retrieval ──

    fn uniform_weights(terms: &[String]) -> Vec<f64> {
        if terms.is_empty() {
            return Vec::new();
        }
        vec![1.0 / terms.len() as f64; terms.len()]
    }

    #[test]
    fn hybrid_lexical_score_counts_covered_terms() {
        let terms = hybrid_query_terms("How is the MUI Button component implemented?");
        assert!(terms.contains(&"button".to_string()));
        assert!(!terms.contains(&"the".to_string()));

        let weights = uniform_weights(&terms);
        let score = hybrid_lexical_score(
            &terms,
            &weights,
            "material-ui: packages/mui-material/src/Button/Button.js",
        );
        assert!(score > 0.0, "query terms must be found in the source text");
        assert!(score <= 1.0);
    }

    #[test]
    fn hybrid_filename_score_prefers_exact_segment() {
        let terms = vec!["button".to_string()];
        let weights = uniform_weights(&terms);
        let exact = "material-ui: packages/mui-material/src/Button/Button.js";
        let lookalike = "material-ui: packages/mui-material/src/Select/Select.js";

        let score_exact = hybrid_filename_score(&terms, &weights, exact);
        let score_lookalike = hybrid_filename_score(&terms, &weights, lookalike);
        assert!(
            score_exact > score_lookalike,
            "exact path segment must beat a look-alike: {} vs {}",
            score_exact,
            score_lookalike
        );
        assert_eq!(score_exact, 1.0);
    }

    #[test]
    fn hybrid_filename_score_rewards_basename_stem_over_test_twin() {
        // The query names "dialog" (plus a second term that matches nothing):
        // the implementation file's stem is exactly "dialog" while its test
        // twin's stem is "dialog.test" — the stem bonus must lift Dialog.js
        // above Dialog.test.js even though both share the Dialog/ directory
        // segment and the "dialog" segment.
        let terms = vec!["dialog".to_string(), "behavior".to_string()];
        let weights = uniform_weights(&terms);
        let impl_file = "material-ui: packages/mui-material/src/Dialog/Dialog.js";
        let test_file = "material-ui: packages/mui-material/src/Dialog/Dialog.test.js";
        let sibling = "material-ui: packages/mui-material/src/Modal/Modal.d.ts";

        let s_impl = hybrid_filename_score(&terms, &weights, impl_file);
        let s_test = hybrid_filename_score(&terms, &weights, test_file);
        let s_sibling = hybrid_filename_score(&terms, &weights, sibling);
        assert!(
            s_impl > s_test,
            "implementation stem must beat its test twin: {} vs {}",
            s_impl,
            s_test
        );
        assert!(
            s_impl > s_sibling,
            "implementation must beat a sibling module: {} vs {}",
            s_impl,
            s_sibling
        );
        assert_eq!(s_impl, 1.0);
    }

    #[test]
    fn hybrid_filename_score_matches_substring_segments() {
        // "button" must surface IconButton/ButtonBase alongside Button, and
        // "authentication" must surface auth.py even though the segment is the
        // shorter "auth".
        let terms = vec!["button".to_string()];
        let weights = uniform_weights(&terms);
        let variant = "material-ui: packages/mui-material/src/IconButton/IconButton.js";
        let unrelated = "material-ui: packages/mui-material/src/Slider/Slider.js";
        let s_variant = hybrid_filename_score(&terms, &weights, variant);
        let s_unrelated = hybrid_filename_score(&terms, &weights, unrelated);
        assert!(s_variant > 0.0, "substring match must surface IconButton");
        assert_eq!(s_unrelated, 0.0, "unrelated file must stay at zero");

        let terms_auth = vec!["authentication".to_string()];
        let weights_auth = uniform_weights(&terms_auth);
        let auth_file = "requests: src/requests/auth.py";
        let s_auth = hybrid_filename_score(&terms_auth, &weights_auth, auth_file);
        assert!(s_auth > 0.0, "term containing the segment must match");
    }

    #[test]
    fn hybrid_filename_score_ignores_extension_noise_segments() {
        // Regression: `.d.ts` splits into the single-char segment "d" and the
        // two-char "ts". Before this guard, "implemented".contains("d") and
        // "modal".contains("d") matched those noise segments, so every type
        // declaration outranked its implementation twin. Neither direction of
        // the substring match may fire on segments shorter than 3 chars.
        let terms = vec!["implemented".to_string(), "modal".to_string()];
        let weights = uniform_weights(&terms);
        let impl_file = "material-ui: packages/mui-material/src/Dialog/Dialog.js";
        let decl_file = "material-ui: packages/mui-material/src/Dialog/Dialog.d.ts";

        let s_impl = hybrid_filename_score(&terms, &weights, impl_file);
        let s_decl = hybrid_filename_score(&terms, &weights, decl_file);
        assert_eq!(
            s_decl, 0.0,
            "d/ts extension segments must not match terms by containment: got {}",
            s_decl
        );
        assert_eq!(
            s_impl, 0.0,
            "js extension must not match either: got {}",
            s_impl
        );

        // Sanity: a real 3+ char segment still matches via term-containment.
        let auth_file = "requests: src/requests/auth.py";
        let terms_auth = vec!["authentication".to_string()];
        let weights_auth = uniform_weights(&terms_auth);
        assert!(hybrid_filename_score(&terms_auth, &weights_auth, auth_file) > 0.0);
    }

    #[test]
    fn hybrid_filename_score_does_not_boost_dts_over_impl() {
        // The exact implementation file ("Dialog.js", stem "dialog") must beat
        // its type-declaration twin ("Dialog.d.ts", stem "dialog.d") even
        // though both share the directory + filename segments — and the .d.ts
        // must not gain an unfair boost from its d/ts extension segments.
        let terms = vec!["dialog".to_string(), "behavior".to_string()];
        let weights = uniform_weights(&terms);
        let impl_file = "material-ui: packages/mui-material/src/Dialog/Dialog.js";
        let decl_file = "material-ui: packages/mui-material/src/Dialog/Dialog.d.ts";

        let s_impl = hybrid_filename_score(&terms, &weights, impl_file);
        let s_decl = hybrid_filename_score(&terms, &weights, decl_file);
        assert!(
            s_impl > s_decl,
            "implementation stem must beat its declaration twin: impl {} vs decl {}",
            s_impl,
            s_decl
        );
        assert_eq!(s_impl, 1.0);
    }

    #[test]
    fn hybrid_filename_score_discounts_test_and_declaration_twins() {
        // For an "how is X implemented" query the implementation file must beat
        // its test/spec/declaration look-alikes even when their paths carry
        // the same segment evidence: `styled.spec.tsx` and `styled.d.ts` share
        // the "styled" segment with the real implementation but must not
        // outrank it. This is the regression behind the Button query where
        // `styled.spec.tsx` (0.383) and `styled.d.ts` (0.369) crowded out
        // relevant `ToggleButton.js` / `StepButton.js` (0.367 / 0.365).
        let terms = vec![
            "mui".to_string(),
            "button".to_string(),
            "styled".to_string(),
        ];
        let weights = uniform_weights(&terms);
        let impl_file = "material-ui: packages/mui-material/src/Button/Button.js";
        let spec_file = "material-ui: packages/mui-material/src/styles/styled.spec.tsx";
        let decl_file = "material-ui: packages/mui-material/src/styles/styled.d.ts";
        let variant_impl = "material-ui: packages/mui-material/src/ToggleButton/ToggleButton.js";

        let s_impl = hybrid_filename_score(&terms, &weights, impl_file);
        let s_spec = hybrid_filename_score(&terms, &weights, spec_file);
        let s_decl = hybrid_filename_score(&terms, &weights, decl_file);
        let s_variant = hybrid_filename_score(&terms, &weights, variant_impl);
        assert!(
            s_impl > s_spec,
            "implementation must beat its spec twin: impl {} vs spec {}",
            s_impl,
            s_spec
        );
        assert!(
            s_impl > s_decl,
            "implementation must beat its declaration twin: impl {} vs decl {}",
            s_impl,
            s_decl
        );
        assert!(
            s_variant > s_spec,
            "a button-variant implementation must beat an unrelated spec twin: variant {} vs spec {}",
            s_variant,
            s_spec
        );
        assert!(
            s_variant > s_decl,
            "a button-variant implementation must beat an unrelated declaration: variant {} vs decl {}",
            s_variant,
            s_decl
        );
        assert_eq!(s_impl, 1.0);
    }

    #[test]
    fn hybrid_combine_bounds_and_orders() {
        // Fallback embeddings give near-zero cosine for different texts, so the
        // filename signal must dominate when it is present.
        let terms = vec!["button".to_string()];
        let weights = uniform_weights(&terms);
        let path_weights = uniform_weights(&terms);
        let exact = "material-ui: packages/mui-material/src/Button/Button.js";
        let unrelated = "log: src/lib.rs";
        let s_exact = hybrid_combine(0.0, &terms, &weights, &path_weights, Some(exact));
        let s_unrelated = hybrid_combine(0.0, &terms, &weights, &path_weights, Some(unrelated));
        assert!(s_exact > 0.0);
        assert!(s_unrelated < s_exact);
        assert!(s_exact <= 1.0);
    }

    #[test]
    fn hybrid_path_idf_weights_reward_rare_path_segments() {
        // "button" is common in *bodies* but rare as a path *segment*: only the
        // Button/ directory carries it. The path IDF must reward it, while a
        // path segment present everywhere ("mui" from mui-material) stays cheap.
        let terms = vec!["button".to_string(), "mui".to_string()];
        let sources = vec![
            Some("material-ui: packages/mui-material/src/Button/Button.js\n\nconst button = styled('button')".to_string()),
            Some("material-ui: packages/mui-material/src/Select/Select.js\n\nconst mui = styled('div')".to_string()),
            Some("material-ui: packages/mui-material/src/Dialog/Dialog.js\n\nconst button = styled('button')".to_string()),
            Some("material-ui: packages/mui-material/src/Modal/Modal.d.ts\n\nmui styled".to_string()),
            Some("log: src/lib.rs\n\nbutton handling".to_string()),
        ];
        let weights = hybrid_path_idf_weights(&terms, &sources);
        assert!(
            weights[0] > weights[1],
            "rare path segment must outweigh corpus-wide one: {:?}",
            weights
        );
    }

    #[test]
    fn hybrid_idf_weights_prefer_rare_terms() {
        // A term present in every corpus row must weigh less than a rare one.
        let terms = vec!["common".to_string(), "rare".to_string()];
        let sources = vec![
            Some("common stuff here".to_string()),
            Some("common and more common".to_string()),
        ];
        let weights = hybrid_idf_weights(&terms, &sources);
        assert!(
            weights[1] > weights[0],
            "rare term must outweigh corpus-wide term: {:?}",
            weights
        );
    }

    #[test]
    fn search_hybrid_ranks_exact_filename_first() {
        let search = SemanticSearch::new_in_memory().unwrap();
        let exact_id = EntityId::new();
        let lookalike_id = EntityId::new();
        search
            .store_fingerprint(
                &exact_id,
                "material-ui: packages/mui-material/src/Button/Button.js\n\nimport React from 'react';\nexport default function Button() { return <button /> }",
            )
            .unwrap();
        search
            .store_fingerprint(
                &lookalike_id,
                "material-ui: packages/mui-material/src/StepButton/StepButton.js\n\nimport Button from '../Button/Button';\nexport default function StepButton() { return <Button /> }",
            )
            .unwrap();

        let results = search
            .search_hybrid("MUI Button component implementation", 5)
            .unwrap();
        assert_eq!(
            results[0].0, exact_id,
            "exact Button.js must rank above StepButton.js in hybrid search"
        );
    }

    #[test]
    fn search_hybrid_falls_back_without_source_text() {
        // Simulate a pre-V30 fingerprint row: embedding present, source_text NULL.
        let search = SemanticSearch::new_in_memory().unwrap();
        let id = EntityId::new();
        let embedding = EmbeddingBackend::fallback_embed("some content here");
        let embedding_json = serde_json::to_string(&embedding).unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        {
            let conn = search.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO memory_semantic_fingerprints (memory_id, keywords_json, created_at)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![id.as_str(), embedding_json, now],
            )
            .unwrap();
        }

        let results = search.search_hybrid("some query", 5).unwrap();
        assert!(
            results.iter().all(|(_, s)| *s <= 1.0),
            "hybrid scores must stay bounded even on legacy rows"
        );
    }

    #[test]
    fn search_documents_hybrid_ranks_by_path() {
        let search = SemanticSearch::new_in_memory().unwrap();
        let doc_id = EntityId::new();
        let other_id = EntityId::new();
        search
            .store_document_fingerprint(
                &doc_id,
                "architecture.md\n\nthe system uses a repository pattern for data access",
            )
            .unwrap();
        search
            .store_document_fingerprint(
                &other_id,
                "onboarding.md\n\nwelcome to the team and read the handbook",
            )
            .unwrap();

        let results = search
            .search_documents_hybrid("repository pattern architecture", 5)
            .unwrap();
        assert!(
            results.iter().any(|(id, _)| *id == doc_id),
            "hybrid document search must surface the matching document"
        );
    }

    // ── Chunking (index truncation gap fix) ──

    #[test]
    fn chunk_text_keeps_short_text_whole() {
        let text = "short text under the window";
        let chunks = SemanticSearch::chunk_text(text);
        assert_eq!(chunks.len(), 1, "short text must stay a single chunk");
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn chunk_text_splits_long_text() {
        let text = "line of code with distinctive symbol\n".repeat(300); // ~13 KB
        let chunks = SemanticSearch::chunk_text(&text);
        assert!(
            chunks.len() > 1,
            "text beyond the window must be chunked (got {})",
            chunks.len()
        );
        // Every character must appear in at least one chunk (no seams).
        for c in text.chars().take(50) {
            assert!(
                chunks.iter().any(|ch| ch.contains(c)),
                "char {c:?} must be covered by some chunk"
            );
        }
        // Chunks are ordered: the first chunk starts the text and the last
        // chunk reaches the end (overlap means boundaries repeat, never skip).
        assert!(text.starts_with(chunks[0]));
        assert!(text.ends_with(chunks[chunks.len() - 1]));
    }

    #[test]
    fn chunk_text_utf8_safe_boundaries() {
        // Cyrillic is 2 bytes per char; a byte-indexed cut would panic.
        let text = "Пользователь ".repeat(1500); // ~26 KB
        let chunks = SemanticSearch::chunk_text(&text);
        assert!(chunks.len() > 1);
        for ch in &chunks {
            assert!(std::str::from_utf8(ch.as_bytes()).is_ok());
        }
        // Coverage invariant: every character of the text appears in at least
        // one chunk — the UTF-8-safe chunker must not drop any tail content.
        for c in text.chars().take(50) {
            assert!(
                chunks.iter().any(|ch| ch.contains(c)),
                "char {c:?} must be covered by some chunk"
            );
        }
        // Ordering invariant: the first chunk starts the text, the last chunk
        // reaches its end (overlap means boundaries repeat, never skip).
        assert!(text.starts_with(chunks[0]));
        assert!(text.ends_with(chunks[chunks.len() - 1]));
    }

    #[test]
    fn chunk_text_prefers_line_boundaries() {
        // A long text whose natural lines fit: chunks should end on '\n'.
        // 400 × 35 bytes ≈ 14 KB — safely past the single-vector window.
        let line = "pub struct Record { field: u32 }\n";
        let text = line.repeat(400); // ~14 KB
        let chunks = SemanticSearch::chunk_text(&text);
        assert!(chunks.len() > 1);
        for ch in chunks.iter().take(chunks.len() - 1) {
            assert!(
                ch.ends_with('\n'),
                "full chunks must end on a line break: {ch:?}"
            );
        }
    }

    #[test]
    fn parse_embeddings_accepts_legacy_single_vector() {
        let legacy = serde_json::to_string(&vec![0.1f32, 0.2, 0.3]).unwrap();
        let parsed = SemanticSearch::parse_embeddings(&legacy).unwrap();
        assert_eq!(parsed.len(), 1, "legacy Vec<f32> wraps into one chunk");
        assert_eq!(parsed[0], vec![0.1f32, 0.2, 0.3]);
    }

    #[test]
    fn parse_embeddings_accepts_chunked_format() {
        let chunked = serde_json::to_string(&vec![vec![0.1f32], vec![0.2f32]]).unwrap();
        let parsed = SemanticSearch::parse_embeddings(&chunked).unwrap();
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn parse_embeddings_rejects_garbage() {
        assert!(SemanticSearch::parse_embeddings("not json").is_none());
    }

    #[test]
    fn best_chunk_cosine_takes_maximum() {
        let query = vec![1.0f32, 0.0, 0.0];
        let chunks = vec![
            vec![0.0f32, 1.0, 0.0], // cos = 0
            vec![1.0f32, 0.0, 0.0], // cos = 1
        ];
        let best = SemanticSearch::best_chunk_cosine(&query, &chunks);
        assert!((best - 1.0).abs() < 1e-6);
    }

    #[test]
    fn best_chunk_cosine_zero_on_empty_chunks() {
        let query = vec![1.0f32, 0.0, 0.0];
        assert_eq!(SemanticSearch::best_chunk_cosine(&query, &[]), 0.0);
    }

    #[test]
    fn mean_embedding_averages_chunks() {
        let chunks = vec![vec![2.0f32, 4.0], vec![4.0f32, 8.0]];
        let mean = SemanticSearch::mean_embedding(&chunks).unwrap();
        assert_eq!(mean, vec![3.0f32, 6.0]);
    }

    #[test]
    fn mean_embedding_none_for_empty() {
        assert!(SemanticSearch::mean_embedding(&[]).is_none());
    }

    /// Regression for the index truncation gap: a symbol living *beyond* the
    /// old single-vector window (8192 bytes) must still be embedded and
    /// searchable. The text mimics the rust-log case (Record at char 30840 of
    /// a 68 KB lib.rs); the chunker must produce a chunk containing it, and
    /// the stored source_text must retain it for the lexical channel.
    #[test]
    fn tail_symbol_beyond_old_window_is_indexed() {
        let search = SemanticSearch::new_in_memory().unwrap();
        let id = EntityId::new();

        // ~10 KB of noise, then the distinctive declaration, then more
        // noise — the declaration sits far inside the body, beyond the old
        // single-vector window.
        let filler = "let variable = compute_value(arg);\n".repeat(300); // ~11 KB
        let tail = "pub struct Record { field: u32 }\n".to_string();
        let after = "fn helper() {}\n".repeat(300); // ~4.5 KB
        let text = format!("{filler}{tail}{after}");

        assert!(
            text.len() > MAX_EMBEDDING_TEXT_LEN,
            "test text must exceed the old single-vector window"
        );

        let chunks = SemanticSearch::chunk_text(&text);
        assert!(
            chunks.iter().any(|c| c.contains("pub struct Record")),
            "the tail symbol must fall into some chunk (got {} chunks)",
            chunks.len()
        );

        search.store_fingerprint(&id, &text).unwrap();

        // The lexical channel must still see the full tail: source_text is
        // stored up to MAX_SOURCE_TEXT_LEN, not truncated at the embed window.
        let conn = search.conn.lock().unwrap();
        let stored: Option<String> = conn
            .query_row(
                "SELECT source_text FROM memory_semantic_fingerprints WHERE memory_id = ?1",
                [id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        drop(conn);
        assert!(
            stored
                .as_deref()
                .unwrap_or("")
                .contains("pub struct Record"),
            "source_text must retain tail symbols for the lexical channel"
        );
    }

    /// Long-text fingerprints are stored chunked: the row's keywords_json is a
    /// Vec<Vec<f32>> with one vector per chunk, and the count matches the
    /// chunker's output.
    #[test]
    fn long_text_stores_multiple_embeddings() {
        let search = SemanticSearch::new_in_memory().unwrap();
        let id = EntityId::new();
        let text = "content ".repeat(3000); // ~24 KB
        let expected_chunks = SemanticSearch::chunk_text(&text).len();

        search.store_fingerprint(&id, &text).unwrap();

        let conn = search.conn.lock().unwrap();
        let json: String = conn
            .query_row(
                "SELECT keywords_json FROM memory_semantic_fingerprints WHERE memory_id = ?1",
                [id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        drop(conn);

        let parsed = SemanticSearch::parse_embeddings(&json).unwrap();
        assert_eq!(
            parsed.len(),
            expected_chunks,
            "stored embedding count must match chunker output"
        );
        assert!(parsed.len() > 1, "long text must produce several chunks");
        assert_eq!(parsed[0].len(), 384, "each chunk is a full-dim vector");
    }

    /// A long document fingerprint follows the same chunking contract.
    #[test]
    fn long_document_stores_multiple_embeddings() {
        let search = SemanticSearch::new_in_memory().unwrap();
        let doc_id = EntityId::new();
        let text = "doc content ".repeat(3000); // ~24 KB

        search.store_document_fingerprint(&doc_id, &text).unwrap();

        let conn = search.conn.lock().unwrap();
        let json: String = conn
            .query_row(
                "SELECT keywords_json FROM document_fingerprints WHERE document_id = ?1",
                [doc_id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        drop(conn);

        let parsed = SemanticSearch::parse_embeddings(&json).unwrap();
        assert!(parsed.len() > 1);
    }
}
