//! # Invoke Module
//!
//! Contains setup and executor building logic for agent invocation.
//!
//! This module extracts common setup patterns used by both root agent
//! invocation and delegated subagent spawning.

mod batch_writer;
mod executor;
pub mod goal_adapter;
pub mod graph_adapter;
pub mod ingest_adapter;
pub mod micro_recall;
pub mod setup;
pub mod stream;
pub mod working_memory;
pub mod working_memory_middleware;

pub use batch_writer::{spawn_batch_writer, spawn_batch_writer_with_repo, BatchWriterHandle};
pub use executor::{
    collect_agents_summary, collect_skills_summary, new_workspace_cache, ExecutorBuilder,
    WorkspaceCache,
};
pub use micro_recall::{
    detect_triggers, execute_micro_recall, extract_new_entities, MicroRecallContext,
    MicroRecallTrigger,
};
pub use setup::{
    append_system_context, detect_subagent_role, subagent_rules, AgentLoader, SubagentRole,
};
pub use stream::{
    broadcast_event, process_stream_event, ResponseAccumulator, StreamContext, ToolCallAccumulator,
};
pub use working_memory::WorkingMemory;
