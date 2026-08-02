use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::core::context::context_package::ContextPackage;
use crate::core::entity_id::EntityId;
use crate::core::result::Result;

/// Cache for context packages — reuse computed contexts for similar queries.
#[async_trait]
pub trait ContextCache: Send + Sync {
    async fn get(&self, cache_key: &str) -> Result<Option<ContextPackage>>;
    async fn set(&self, cache_key: &str, package: &ContextPackage) -> Result<()>;
    async fn invalidate(&self, entity_id: &EntityId) -> Result<()>;
    async fn clear(&self) -> Result<()>;
}

/// Global shared cache instance — persists across all build_context calls.
pub fn global_cache() -> &'static InMemoryContextCache {
    static INSTANCE: OnceLock<InMemoryContextCache> = OnceLock::new();
    INSTANCE.get_or_init(|| InMemoryContextCache::new(Duration::from_secs(300)))
}

/// In-memory context cache with TTL-based expiration and entity-based invalidation.
pub struct InMemoryContextCache {
    cache: Mutex<HashMap<String, (ContextPackage, Instant)>>,
    /// Reverse index: entity_id → set of cache keys containing that entity
    entity_index: Mutex<HashMap<String, HashSet<String>>>,
    ttl: Duration,
}

impl InMemoryContextCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            entity_index: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    /// Extract all entity IDs from a context package.
    fn extract_entity_ids(package: &ContextPackage) -> Vec<String> {
        let mut ids: Vec<String> = package.entities.iter().map(|e| e.id.to_string()).collect();
        // Also include memory record IDs for completeness
        ids.extend(package.memory_records.iter().map(|r| r.id.to_string()));
        ids
    }

    /// Update the reverse index when adding a cache entry.
    fn index_entry(
        cache_key: &str,
        package: &ContextPackage,
        entity_index: &mut HashMap<String, HashSet<String>>,
    ) {
        for entity_id in Self::extract_entity_ids(package) {
            entity_index
                .entry(entity_id)
                .or_default()
                .insert(cache_key.to_string());
        }
    }

    /// Remove a cache entry and clean up the reverse index.
    fn remove_entry(
        cache_key: &str,
        cache: &mut HashMap<String, (ContextPackage, Instant)>,
        entity_index: &mut HashMap<String, HashSet<String>>,
    ) {
        if let Some((package, _)) = cache.remove(cache_key) {
            // Clean up reverse index
            for entity_id in Self::extract_entity_ids(&package) {
                if let Some(keys) = entity_index.get_mut(&entity_id) {
                    keys.remove(cache_key);
                    if keys.is_empty() {
                        entity_index.remove(&entity_id);
                    }
                }
            }
        }
    }
}

#[async_trait]
impl ContextCache for InMemoryContextCache {
    async fn get(&self, cache_key: &str) -> Result<Option<ContextPackage>> {
        let cache = self
            .cache
            .lock()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        if let Some((package, timestamp)) = cache.get(cache_key)
            && timestamp.elapsed() < self.ttl
        {
            return Ok(Some(package.clone()));
        }
        Ok(None)
    }
    async fn set(&self, cache_key: &str, package: &ContextPackage) -> Result<()> {
        let mut cache = self
            .cache
            .lock()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        let mut entity_index = self
            .entity_index
            .lock()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;

        // Remove old entry if exists (clean up reverse index)
        Self::remove_entry(cache_key, &mut cache, &mut entity_index);

        // Insert new entry
        cache.insert(cache_key.to_string(), (package.clone(), Instant::now()));
        Self::index_entry(cache_key, package, &mut entity_index);

        Ok(())
    }

    async fn invalidate(&self, entity_id: &EntityId) -> Result<()> {
        let mut cache = self
            .cache
            .lock()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        let mut entity_index = self
            .entity_index
            .lock()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;

        let id_str = entity_id.to_string();
        if let Some(keys) = entity_index.remove(&id_str) {
            for key in keys {
                Self::remove_entry(&key, &mut cache, &mut entity_index);
            }
        }

        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        let mut cache = self
            .cache
            .lock()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        let mut entity_index = self
            .entity_index
            .lock()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        cache.clear();
        entity_index.clear();
        Ok(())
    }
}

/// Blanket impl: allows passing `&InMemoryContextCache` (e.g. `&'static` from
/// `global_cache()`) anywhere a `ContextCache` is required.
#[async_trait]
impl<T: ContextCache> ContextCache for &T {
    async fn get(&self, cache_key: &str) -> Result<Option<ContextPackage>> {
        (**self).get(cache_key).await
    }

    async fn set(&self, cache_key: &str, package: &ContextPackage) -> Result<()> {
        (**self).set(cache_key, package).await
    }

    async fn invalidate(&self, entity_id: &EntityId) -> Result<()> {
        (**self).invalidate(entity_id).await
    }

    async fn clear(&self) -> Result<()> {
        (**self).clear().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::context_package::{IntentType, UserIntent};
    use crate::core::graph::entity::Entity;
    use crate::core::graph::entity_types::EntityType;

    fn sample_package() -> ContextPackage {
        ContextPackage::new(UserIntent {
            query: "test".to_string(),
            intent_type: IntentType::Search,
            confidence: 0.8,
            keywords: vec!["test".to_string()],
            temporal: None,
        })
    }

    fn package_with_entity(entity_id: &EntityId) -> ContextPackage {
        let mut pkg = sample_package();
        let entity = Entity::new(
            EntityType::Person,
            "Alice".to_string(),
            "Engineer".to_string(),
        );
        // Override the entity ID to match what we want
        let mut entity = entity;
        entity.id = entity_id.clone();
        pkg.entities = vec![entity];
        pkg
    }

    #[tokio::test]
    async fn cache_miss() {
        let cache = InMemoryContextCache::new(Duration::from_secs(60));
        assert!(cache.get("key1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn cache_hit() {
        let cache = InMemoryContextCache::new(Duration::from_secs(60));
        let pkg = sample_package();
        cache.set("key1", &pkg).await.unwrap();
        let cached = cache.get("key1").await.unwrap().unwrap();
        assert_eq!(cached.id, pkg.id);
    }

    #[tokio::test]
    async fn cache_expired() {
        let cache = InMemoryContextCache::new(Duration::from_millis(1));
        let pkg = sample_package();
        cache.set("key1", &pkg).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(cache.get("key1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn cache_clear() {
        let cache = InMemoryContextCache::new(Duration::from_secs(60));
        cache.set("key1", &sample_package()).await.unwrap();
        cache.clear().await.unwrap();
        assert!(cache.get("key1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn invalidate_removes_entries_with_entity() {
        let cache = InMemoryContextCache::new(Duration::from_secs(60));
        let eid = EntityId::new();

        // Cache two entries: one with the entity, one without
        let pkg_with = package_with_entity(&eid);
        let pkg_without = sample_package();

        cache.set("key_with_entity", &pkg_with).await.unwrap();
        cache.set("key_without_entity", &pkg_without).await.unwrap();

        // Invalidate the entity
        cache.invalidate(&eid).await.unwrap();

        // Entry with entity should be gone
        assert!(cache.get("key_with_entity").await.unwrap().is_none());
        // Entry without entity should still be there
        assert!(cache.get("key_without_entity").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn invalidate_only_affects_related_entries() {
        let cache = InMemoryContextCache::new(Duration::from_secs(60));
        let eid1 = EntityId::new();
        let eid2 = EntityId::new();

        // Create entries with different entities
        let mut pkg1 = sample_package();
        pkg1.entities = vec![Entity::new(
            EntityType::Person,
            "Alice".to_string(),
            "".to_string(),
        )];
        pkg1.entities[0].id = eid1.clone();

        let mut pkg2 = sample_package();
        pkg2.entities = vec![Entity::new(
            EntityType::Person,
            "Bob".to_string(),
            "".to_string(),
        )];
        pkg2.entities[0].id = eid2.clone();

        cache.set("key1", &pkg1).await.unwrap();
        cache.set("key2", &pkg2).await.unwrap();

        // Invalidate only eid1
        cache.invalidate(&eid1).await.unwrap();

        assert!(cache.get("key1").await.unwrap().is_none());
        assert!(cache.get("key2").await.unwrap().is_some());
    }
}
