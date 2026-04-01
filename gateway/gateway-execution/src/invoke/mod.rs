//! # Invoke Module
//!
//! Contains setup and executor building logic for agent invocation.
//!
//! This module extracts common setup patterns used by both root agent
//! invocation and delegated subagent spawning.

mod batch_writer;
mod executor;
mod setup;
mod stream;

pub use batch_writer::{spawn_batch_writer, spawn_batch_writer_with_repo, BatchWriterHandle};
pub use executor::{collect_agents_summary, collect_skills_summary, new_workspace_cache, ExecutorBuilder, WorkspaceCache};
pub use setup::{AgentLoader, SubagentRole, append_system_context, detect_subagent_role, subagent_rules};
pub use stream::{
    broadcast_event, process_stream_event, ResponseAccumulator, StreamContext,
    ToolCallAccumulator,
};
