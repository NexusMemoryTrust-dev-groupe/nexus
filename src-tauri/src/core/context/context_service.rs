use crate::core::context::context_builder::ContextBuilder;
use crate::core::context::context_cache::ContextCache;
use crate::core::context::context_package::ContextPackage;
use crate::core::context::context_request::ContextRequest;
use crate::core::context::context_snapshot::ContextSnapshot;
use crate::core::context::context_store::ContextStore;
use crate::core::entity_id::EntityId;
use crate::core::result::Result;

/// Orchestrator for context operations — build, cache, snapshot, restore.
pub struct ContextService<B: ContextBuilder, C: ContextCache, S: ContextStore> {
    builder: B,
    cache: C,
    store: S,
}

impl<B: ContextBuilder, C: ContextCache, S: ContextStore> ContextService<B, C, S> {
    pub fn new(builder: B, cache: C, store: S) -> Self {
        Self {
            builder,
            cache,
            store,
        }
    }

    /// Cache key for a request.
    ///
    /// Must cover every field that changes the produced package. Keying on
    /// `query` + `project_id` alone meant two requests differing only in
    /// `max_tokens`/`max_entities`/`max_depth`/`min_relevance` collided, so the
    /// second one silently got the first one's package.
    fn cache_key(request: &ContextRequest) -> String {
        format!(
            "{}|{}|t={}|e={}|d={}|r={:.4}",
            request.query,
            request
                .project_id
                .as_ref()
                .map(|p| p.as_str())
                .unwrap_or(""),
            request.max_tokens,
            request.max_entities,
            request.max_depth,
            request.min_relevance,
        )
    }

    /// Build a context, using cache if available.
    pub async fn build_context(&self, request: &ContextRequest) -> Result<ContextPackage> {
        let cache_key = Self::cache_key(request);

        if let Some(cached) = self.cache.get(&cache_key).await? {
            return Ok(cached);
        }

        let package = self.builder.build(request).await?;
        self.cache.set(&cache_key, &package).await?;

        Ok(package)
    }

    /// Get a cached context by query.
    pub async fn get_cached_context(&self, query: &str) -> Result<Option<ContextPackage>> {
        self.cache.get(query).await
    }

    /// Save the current context for an entity as a snapshot.
    pub async fn save_snapshot(&self, entity_id: &EntityId, label: Option<&str>) -> Result<String> {
        let request = ContextRequest {
            query: String::new(),
            project_id: Some(entity_id.clone()),
            ..Default::default()
        };

        let package = self.builder.build(&request).await?;

        let snapshot =
            ContextSnapshot::new(entity_id.clone(), package, label.map(|s| s.to_string()));

        self.store.save_snapshot(&snapshot).await
    }

    /// Restore a context package from a snapshot.
    pub async fn restore_snapshot(&self, snapshot_id: &str) -> Result<ContextPackage> {
        self.store.restore_snapshot(snapshot_id).await
    }

    /// Replay a context from a snapshot (alias for restore).
    pub async fn replay_context(&self, snapshot_id: &str) -> Result<ContextPackage> {
        self.store.restore_snapshot(snapshot_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::context_builder::ContextBuilderImpl;
    use crate::core::context::context_cache::InMemoryContextCache;
    use crate::core::memory::memory_record::MemoryRecord;
    use crate::core::memory::memory_repository::MemoryRepository;
    use crate::core::memory::types::MemorySource;
    use crate::storage::sqlite::context_repository::SqliteContextRepository;
    use crate::storage::sqlite::graph_repository::SqliteGraphRepository;
    use crate::storage::sqlite::memory_repository_sqlite::SqliteMemoryRepository;
    use std::time::Duration;

    /// Three independent in-memory connections (graph / memory / snapshots),
    /// each migrated — the same isolation pattern the rest of the suite uses.
    fn test_repos() -> (
        SqliteGraphRepository,
        SqliteMemoryRepository,
        SqliteContextRepository,
    ) {
        let graph_conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::schema::apply_migrations(&graph_conn).unwrap();
        let graph = SqliteGraphRepository::new(graph_conn).unwrap();

        let mem_conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::schema::apply_migrations(&mem_conn).unwrap();
        let memory = SqliteMemoryRepository::new(mem_conn).unwrap();

        let ctx_conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::schema::apply_migrations(&ctx_conn).unwrap();
        let store = SqliteContextRepository::new(ctx_conn).unwrap();

        (graph, memory, store)
    }

    /// A throwaway SQLite *file* (not in-memory): the integrity test needs a
    /// second independent connection to the same database so memory can be
    /// mutated *after* the builder has consumed its own connection.
    fn fresh_db(prefix: &str) -> std::path::PathBuf {
        let tmp = std::env::temp_dir().join(format!("nexus-{prefix}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        let db = tmp.join("nexus.db");
        let conn = crate::db::open_connection_at(&db).expect("open test db");
        crate::storage::sqlite::schema::apply_migrations(&conn).expect("migrate test db");
        drop(conn);
        db
    }

    async fn seed_memory(memory: &SqliteMemoryRepository, content: &str) -> MemoryRecord {
        let record = MemoryRecord::new(
            "DB choice".to_string(),
            content.to_string(),
            "alice".to_string(),
            MemorySource::Manual,
        )
        .unwrap();
        memory.save(&record).await.unwrap();
        record
    }

    /// 9.4 Context integrity — «memory изменили после сборки».
    ///
    /// A context package is built while the memory says X, then the memory is
    /// changed to Y. The snapshot recorded at build time must still carry X:
    /// the package is a fixed record of the *actual* context, not a live view.
    #[tokio::test]
    async fn snapshot_frozen_after_memory_change() {
        let db = fresh_db("ctx-integrity");

        // Build: graph + memory + snapshot store, each with its own connection
        // to the same file.
        let graph =
            SqliteGraphRepository::new(crate::db::open_connection_at(&db).unwrap()).unwrap();
        let store =
            SqliteContextRepository::new(crate::db::open_connection_at(&db).unwrap()).unwrap();
        let memory =
            SqliteMemoryRepository::new(crate::db::open_connection_at(&db).unwrap()).unwrap();

        let record = seed_memory(&memory, "Use PostgreSQL").await;
        let memory_id = record.id.clone();

        let builder = ContextBuilderImpl::new(graph, memory);
        let request = ContextRequest {
            query: "PostgreSQL".to_string(),
            // Zero floor so the freshly seeded memory is guaranteed to survive
            // the compression step — the test wants to pin *content*, not
            // relevance-filter behaviour.
            min_relevance: 0.0,
            ..Default::default()
        };
        let pkg = builder.build(&request).await.unwrap();

        // The built package reflects the memory as it was at build time.
        let built = pkg
            .memory_records
            .iter()
            .find(|m| m.id == memory_id)
            .expect("memory must be in the built package");
        assert_eq!(built.content, "Use PostgreSQL");

        // Pin the package: snapshot it before touching anything else.
        let snap = ContextSnapshot::new(EntityId::new(), pkg, Some("pin".to_string()));
        let snap_id = store.save_snapshot(&snap).await.unwrap();

        // The memory is now changed — but the recorded package must not move.
        // A fresh connection is required: the builder consumed the first one.
        let memory2 =
            SqliteMemoryRepository::new(crate::db::open_connection_at(&db).unwrap()).unwrap();
        let mut changed = record;
        changed.content = "Use MySQL now".to_string();
        changed.updated_at = chrono::Utc::now();
        changed.version += 1;
        memory2.update(&changed).await.unwrap();

        // Sanity: the mutation is visible through the fresh connection.
        let live = memory2.get_by_id(&memory_id).await.unwrap().unwrap();
        assert_eq!(live.content, "Use MySQL now");

        // The snapshot, however, is frozen at build time.
        let restored = store.restore_snapshot(&snap_id).await.unwrap();
        let pinned = restored
            .memory_records
            .iter()
            .find(|m| m.id == memory_id)
            .expect("memory still present in restored package");
        assert_eq!(
            pinned.content, "Use PostgreSQL",
            "snapshot must carry the actual context: the content as it was at build time"
        );
    }

    /// 9.5 Deterministic replay — the same snapshot replayed twice yields the
    /// same package (IDs, scores, provenance, tokens), and it matches the
    /// package that was pinned at build time.
    #[tokio::test]
    async fn replay_context_is_deterministic() {
        let (graph, memory, store) = test_repos();
        seed_memory(&memory, "Use PostgreSQL").await;

        let builder = ContextBuilderImpl::new(graph, memory);
        let request = ContextRequest {
            query: "PostgreSQL".to_string(),
            max_tokens: 4000,
            min_relevance: 0.0,
            ..Default::default()
        };
        let pkg = builder.build(&request).await.unwrap();

        // Persist the snapshot BEFORE the service takes ownership of the store.
        let snap = ContextSnapshot::new(EntityId::new(), pkg.clone(), Some("replay".to_string()));
        let snap_id = store.save_snapshot(&snap).await.unwrap();

        let cache = InMemoryContextCache::new(Duration::from_secs(60));
        let service = ContextService::new(builder, cache, store);

        let replay_1 = service.replay_context(&snap_id).await.unwrap();
        let replay_2 = service.replay_context(&snap_id).await.unwrap();
        let replay_3 = service.replay_context(&snap_id).await.unwrap();

        // Full JSON equality: IDs, scores, provenance, tokens — everything.
        let j1 = serde_json::to_value(&replay_1).unwrap();
        let j2 = serde_json::to_value(&replay_2).unwrap();
        let j3 = serde_json::to_value(&replay_3).unwrap();
        assert_eq!(j1, j2, "replay must be stable across calls");
        assert_eq!(j2, j3, "replay must be stable across calls");
        assert_eq!(
            j1,
            serde_json::to_value(&pkg).unwrap(),
            "replay must reproduce the pinned package"
        );
    }
}
