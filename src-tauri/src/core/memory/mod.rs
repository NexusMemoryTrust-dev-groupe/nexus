pub mod agent_permissions;
pub mod canonical_consolidation;
pub mod conflict;
pub mod layer;
pub mod memory_compression;
pub mod memory_firewall;
pub mod memory_lifecycle;
pub mod memory_radar;
pub mod memory_recall;
pub mod memory_record;
pub mod memory_rehearsal;
pub mod memory_repository;
pub mod memory_score;
pub mod memory_service;
pub mod types;

pub use agent_permissions::{
    AccessVerdict, AgentAccessAssessment, AgentPolicy, Sensitivity, assess_agent_access,
    classify_categories, classify_sensitivity, render_policy,
};
pub use canonical_consolidation::{
    Cluster, ConsolidationResult, build_canonical, find_clusters, render_clusters, similarity,
};
pub use layer::{LayerClassification, LayerClassifier};
pub use memory_compression::{
    CompressedMemory, MemoryCompressionService, SimpleCompressionService,
};
pub use memory_firewall::{
    FirewallAction, FirewallAssessment, FirewallRepository, FirewallRule, FirewallScores,
    FirewallVerdict, QuarantineEntry, QuarantineStatus, assess_content, assess_with_rules,
};
pub use memory_radar::{RadarAction, RadarItem, RadarSnapshot, build_snapshot};
pub use memory_recall::{MemoryRecallService, RecallContext, RecallResult};
pub use memory_record::MemoryRecord;
pub use memory_rehearsal::{
    RehearsalCounts, RehearsalItem, RehearsalPlan, apply_rehearsal, build_rehearsal_plan, is_due,
    next_rehearsal_at, schedule_first_rehearsal, sleep_cycle,
};
pub use memory_repository::MemoryRepository;
pub use memory_score::{MemoryScore, ScoreMetric, ScoreOptions, compute_score, render_score};
pub use memory_service::MemoryService;
pub use types::{
    MemoryCaptureMode, MemoryFeedback, MemoryLayer, MemorySource, MemoryState, MemoryStatus,
    MemoryVisibility,
};
