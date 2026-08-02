// Public API re-exports — consumed by M2+ modules and main.rs
#![allow(dead_code)]
#![allow(unused_imports)]

pub mod config;
pub mod context;
pub mod domain_event;
pub mod entity_id;
pub mod event_bus;
pub mod graph;
pub mod interpreter;
pub mod mcp_register;
pub mod memory;
pub mod module_registry;
pub mod result;
pub mod sandbox;
pub mod security;
pub mod text;
pub mod tokenizer;
pub mod value_object;
pub mod versioning;

pub use config::{ConfigurationProvider, InMemoryConfig};
pub use domain_event::{DomainEvent, DomainEventType};
pub use entity_id::EntityId;
pub use module_registry::{Module, ModuleInfo, ModuleRegistry};
pub use result::{AppError, Result};
pub use security::RequestContext;
pub use value_object::ValueObject;
