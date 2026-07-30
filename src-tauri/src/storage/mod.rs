// Storage layer — concrete implementations of core/ traits.
// Will be consumed by main.rs and commands/ when wiring DI.
#![allow(unused_imports)]

pub mod sqlite;

pub use sqlite::{SqliteMemoryRepository, InMemoryRecallService, SqliteVersioningRepository, SqliteGraphRepository, SqliteContextRepository};
