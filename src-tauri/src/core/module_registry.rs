use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::core::result::{AppError, Result};

/// Information about a registered module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub name: String,
    pub version: String,
    pub dependencies: Vec<String>,
}

/// Trait for all Nexus modules.
/// Modules are the building blocks of the system.
/// They must be self-contained and declare their dependencies.
#[async_trait]
pub trait Module: Send + Sync {
    /// Returns the module name.
    fn name(&self) -> &str;

    /// Returns the module version.
    fn version(&self) -> &str;

    /// Returns the names of modules this module depends on.
    fn dependencies(&self) -> Vec<&str>;

    /// Initialize the module. Called during system startup.
    async fn initialize(&self) -> Result<()>;

    /// Shutdown the module. Called during system shutdown.
    async fn shutdown(&self) -> Result<()>;
}

/// Registry for tracking loaded modules and their dependencies.
pub struct ModuleRegistry {
    modules: std::collections::HashMap<String, Box<dyn Module>>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self {
            modules: std::collections::HashMap::new(),
        }
    }

    /// Register a module.
    pub fn register(&mut self, module: Box<dyn Module>) {
        self.modules.insert(module.name().to_string(), module);
    }

    /// Get a module by name.
    pub fn get(&self, name: &str) -> Option<&dyn Module> {
        self.modules.get(name).map(|m| m.as_ref())
    }

    /// List all registered module names.
    pub fn list(&self) -> Vec<&str> {
        self.modules.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a module is registered.
    pub fn is_registered(&self, name: &str) -> bool {
        self.modules.contains_key(name)
    }

    /// Validate that all declared dependencies exist in the registry.
    pub fn validate_dependencies(&self) -> Result<()> {
        for module in self.modules.values() {
            for dep in module.dependencies() {
                if !self.modules.contains_key(dep) {
                    return Err(AppError::Configuration(format!(
                        "Module '{}' depends on '{}' which is not registered",
                        module.name(),
                        dep
                    )));
                }
            }
        }
        Ok(())
    }

    /// Initialize all registered modules.
    pub async fn initialize_all(&self) -> Result<()> {
        for module in self.modules.values() {
            module.initialize().await?;
        }
        Ok(())
    }

    /// Shutdown all registered modules.
    pub async fn shutdown_all(&self) -> Result<()> {
        for module in self.modules.values() {
            module.shutdown().await?;
        }
        Ok(())
    }
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestModule {
        name: String,
        version: String,
        deps: Vec<String>,
    }

    #[async_trait]
    impl Module for TestModule {
        fn name(&self) -> &str {
            &self.name
        }
        fn version(&self) -> &str {
            &self.version
        }
        fn dependencies(&self) -> Vec<&str> {
            self.deps.iter().map(|s| s.as_str()).collect()
        }
        async fn initialize(&self) -> Result<()> {
            Ok(())
        }
        async fn shutdown(&self) -> Result<()> {
            Ok(())
        }
    }

    fn make_module(name: &str, deps: Vec<&str>) -> Box<dyn Module> {
        Box::new(TestModule {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            deps: deps.into_iter().map(String::from).collect(),
        })
    }

    #[test]
    fn register_and_get() {
        let mut reg = ModuleRegistry::new();
        reg.register(make_module("core", vec![]));
        assert!(reg.is_registered("core"));
        assert_eq!(reg.get("core").unwrap().name(), "core");
    }

    #[test]
    fn list_modules() {
        let mut reg = ModuleRegistry::new();
        reg.register(make_module("a", vec![]));
        reg.register(make_module("b", vec![]));
        assert_eq!(reg.list().len(), 2);
    }

    #[test]
    fn validate_deps_pass() {
        let mut reg = ModuleRegistry::new();
        reg.register(make_module("core", vec![]));
        reg.register(make_module("m1", vec!["core"]));
        assert!(reg.validate_dependencies().is_ok());
    }

    #[test]
    fn validate_deps_fail() {
        let mut reg = ModuleRegistry::new();
        reg.register(make_module("m1", vec!["nonexistent"]));
        assert!(reg.validate_dependencies().is_err());
    }

    #[tokio::test]
    async fn initialize_all() {
        let mut reg = ModuleRegistry::new();
        reg.register(make_module("a", vec![]));
        reg.register(make_module("b", vec![]));
        assert!(reg.initialize_all().await.is_ok());
    }

    #[tokio::test]
    async fn shutdown_all() {
        let mut reg = ModuleRegistry::new();
        reg.register(make_module("a", vec![]));
        assert!(reg.shutdown_all().await.is_ok());
    }

    #[test]
    fn default_registry() {
        let reg = ModuleRegistry::default();
        assert!(reg.list().is_empty());
    }
}
