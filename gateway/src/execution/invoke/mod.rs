//! # Invoke Module
//!
//! Contains setup and executor building logic for agent invocation.
//!
//! This module extracts common setup patterns used by both root agent
//! invocation and delegated subagent spawning.

mod executor;
mod setup;
mod stream;

pub use executor::{collect_agents_summary, collect_skills_summary, ExecutorBuilder};
pub use setup::AgentLoader;
pub use stream::{
    broadcast_event, process_stream_event, ResponseAccumulator, StreamContext,
    ToolCallAccumulator,
};
