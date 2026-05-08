//! # Invoke Module
//!
//! Contains setup and executor building logic for agent invocation.
//!
//! This module extracts common setup patterns used by both root agent
//! invocation and delegated subagent spawning.

mod batch_writer;
mod executor;
pub mod goal_adapter;
pub mod ingest_adapter;
pub mod kg_store_adapter;
pub mod micro_recall;
pub mod setup;
pub mod stream;
pub mod working_memory;
pub mod working_memory_middleware;

pub use batch_writer::{BatchWriterHandle, spawn_batch_writer, spawn_batch_writer_with_repo};
pub use executor::{
    ExecutorBuilder, collect_agents_summary, collect_skills_summary, resolve_thinking_flag,
};
pub use micro_recall::{
    MicroRecallContext, MicroRecallTrigger, detect_triggers, execute_micro_recall,
    extract_new_entities,
};
pub use setup::{
    AgentLoader, SubagentRole, append_system_context, detect_subagent_role, subagent_rules,
};
pub use stream::{
    ResponseAccumulator, StreamContext, ToolCallAccumulator, broadcast_event, process_stream_event,
};
pub use working_memory::WorkingMemory;
