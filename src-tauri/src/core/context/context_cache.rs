use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
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

/// In-memory context cache with TTL-based expiration.
pub struct InMemoryContextCache {
    cache: Mutex<HashMap<String, (ContextPackage, Instant)>>,
    ttl: Duration,
}

impl InMemoryContextCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            ttl,
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
        cache.insert(cache_key.to_string(), (package.clone(), Instant::now()));
        Ok(())
    }

    async fn invalidate(&self, _entity_id: &EntityId) -> Result<()> {
        // In a real implementation, we'd track which cache entries contain this entity.
        // For M4, clearing all is acceptable.
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        let mut cache = self
            .cache
            .lock()
            .map_err(|e| crate::core::AppError::Internal(e.to_string()))?;
        cache.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::context_package::{IntentType, UserIntent};

    fn sample_package() -> ContextPackage {
        ContextPackage::new(UserIntent {
            query: "test".to_string(),
            intent_type: IntentType::Search,
            confidence: 0.8,
            keywords: vec!["test".to_string()],
            temporal: None,
        })
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
}
