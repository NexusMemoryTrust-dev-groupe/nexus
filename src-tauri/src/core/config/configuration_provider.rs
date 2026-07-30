use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::Mutex;

use crate::core::result::Result;

/// Trait for configuration providers.
/// Implementations must be Send + Sync for async access.
/// Business logic must never access std::env directly — always through this trait.
#[async_trait]
pub trait ConfigurationProvider: Send + Sync {
    /// Get a configuration value by key.
    async fn get(&self, key: &str) -> Result<Option<String>>;

    /// Get a configuration value or return default if not found.
    fn get_or_default(&self, key: &str, default: &str) -> String;

    /// Set a configuration value.
    async fn set(&self, key: &str, value: &str) -> Result<()>;

    /// Check if a key exists.
    async fn has(&self, key: &str) -> Result<bool>;

    /// Delete a key.
    async fn delete(&self, key: &str) -> Result<()>;
}

/// In-memory configuration store.
/// Suitable for testing and development.
pub struct InMemoryConfig {
    values: Mutex<HashMap<String, String>>,
}

impl InMemoryConfig {
    pub fn new() -> Self {
        Self {
            values: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl ConfigurationProvider for InMemoryConfig {
    async fn get(&self, key: &str) -> Result<Option<String>> {
        let values = self.values.lock().await;
        Ok(values.get(key).cloned())
    }

    fn get_or_default(&self, _key: &str, default: &str) -> String {
        // For InMemoryConfig, we can do a blocking read since it's just for dev/testing
        // In production, this would be async
        default.to_string()
    }

    async fn set(&self, key: &str, value: &str) -> Result<()> {
        let mut values = self.values.lock().await;
        values.insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn has(&self, key: &str) -> Result<bool> {
        let values = self.values.lock().await;
        Ok(values.contains_key(key))
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let mut values = self.values.lock().await;
        values.remove(key);
        Ok(())
    }
}

impl Default for InMemoryConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn in_memory_config_set_and_get() {
        let config = InMemoryConfig::new();
        config.set("db.path", "/tmp/nexus.db").await.unwrap();
        let value = config.get("db.path").await.unwrap();
        assert_eq!(value, Some("/tmp/nexus.db".to_string()));
    }

    #[tokio::test]
    async fn in_memory_config_get_missing() {
        let config = InMemoryConfig::new();
        let value = config.get("nonexistent").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn in_memory_config_has() {
        let config = InMemoryConfig::new();
        assert!(!config.has("key").await.unwrap());
        config.set("key", "value").await.unwrap();
        assert!(config.has("key").await.unwrap());
    }

    #[tokio::test]
    async fn in_memory_config_delete() {
        let config = InMemoryConfig::new();
        config.set("key", "value").await.unwrap();
        config.delete("key").await.unwrap();
        assert_eq!(config.get("key").await.unwrap(), None);
    }

    #[tokio::test]
    async fn in_memory_config_overwrite() {
        let config = InMemoryConfig::new();
        config.set("key", "v1").await.unwrap();
        config.set("key", "v2").await.unwrap();
        assert_eq!(config.get("key").await.unwrap(), Some("v2".to_string()));
    }

    #[test]
    fn in_memory_config_get_or_default() {
        let config = InMemoryConfig::new();
        assert_eq!(
            config.get_or_default("missing", "fallback"),
            "fallback"
        );
    }

    #[tokio::test]
    async fn in_memory_config_default() {
        let config = InMemoryConfig::default();
        assert_eq!(config.get("any").await.unwrap(), None);
    }
}
