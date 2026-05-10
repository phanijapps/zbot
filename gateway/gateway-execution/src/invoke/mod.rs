//! # Invoke Module
//!
//! Contains setup and executor building logic for agent invocation.
//!
//! This module extracts common setup patterns used by both root agent
//! invocation and delegated subagent spawning.

mod batch_writer;
mod event_logging;
mod executor;
pub mod goal_adapter;
pub mod ingest_adapter;
pub mod kg_store_adapter;
pub mod micro_recall;
mod response_accumulator;
pub mod setup;
pub mod stream;
mod stream_context;
mod tool_call_accumulator;
pub mod working_memory;
pub mod working_memory_middleware;

pub use batch_writer::{spawn_batch_writer, spawn_batch_writer_with_repo, BatchWriterHandle};
pub use executor::{
    collect_agents_summary, collect_skills_summary, resolve_thinking_flag, ExecutorBuilder,
};
pub use micro_recall::{
    detect_triggers, execute_micro_recall, extract_new_entities, MicroRecallContext,
    MicroRecallTrigger,
};
pub use setup::{
    append_system_context, detect_subagent_role, subagent_rules, AgentLoader, SubagentRole,
};
pub use response_accumulator::ResponseAccumulator;
pub use stream::{broadcast_event, process_stream_event};
pub use tool_call_accumulator::{ToolCallAccumulator, ToolCallRecord};
pub use stream_context::StreamContext;
pub use working_memory::WorkingMemory;
