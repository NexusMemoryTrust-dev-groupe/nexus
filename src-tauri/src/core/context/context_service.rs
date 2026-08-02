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
    // ContextService tests require mock implementations of all 3 traits.
    // Tested via integration with concrete implementations.
}
