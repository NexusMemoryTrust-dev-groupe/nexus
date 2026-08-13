pub mod auto_graph_builder;
pub mod compressor;
pub mod context_builder;
pub mod context_cache;
pub mod context_lab;
pub mod context_package;
pub mod context_request;
pub mod context_service;
pub mod context_snapshot;
pub mod context_store;
pub mod export;
pub mod graph_seeder;
pub mod indexer;
pub mod intent_detector;
pub mod memory_injector;
pub mod predictive;
pub mod provenance;
pub mod ranker;
pub mod semantic_search;

pub use auto_graph_builder::AutoGraphBuilder;
pub use compressor::ContextCompressor;
pub use context_builder::ContextBuilder;
pub use context_cache::{ContextCache, InMemoryContextCache};
pub use context_lab::{
    ContextStrategy, LabExperiment, LabMetrics, LabResult, predict_accuracy, render_comparison,
};
pub use context_package::{ContextPackage, IntentType, TemporalSlice, UserIntent};
pub use context_request::ContextRequest;
pub use context_service::ContextService;
pub use context_snapshot::ContextSnapshot;
pub use context_store::ContextStore;
pub use graph_seeder::GraphSeeder;
pub use intent_detector::IntentDetector;
pub use memory_injector::MemoryInjector;
pub use predictive::{
    Prediction, QueryLogEntry, jaccard, normalize, predict_next, prewarm_entities,
};
pub use provenance::{DropCause, ItemKind, Provenance, Reason, ScorePart, Trace};
pub use ranker::ContextRanker;
pub use semantic_search::SemanticSearch;
