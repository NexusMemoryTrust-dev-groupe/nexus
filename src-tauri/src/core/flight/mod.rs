//! Flight Recorder — бортовой самописец операций (Система 5).
//!
//! Хроника всех значимых шагов экосистемы: создания памяти, конфликтов,
//! карантина, rehearsal, вызовов скиллов и MCP-инструментов. Журнал можно
//! воспроизвести по цепочке (replay) и получить сводную статистику.

pub mod context_chain;
pub mod flight_listener;
pub mod flight_recorder;

pub use context_chain::{
    ChainStage, ContextChain, ContextKind, ContextSeed, KindShare, StageRecord, context_breakdown,
    render_stages, render_why,
};
pub use flight_listener::create_flight_listener;
pub use flight_recorder::*;
