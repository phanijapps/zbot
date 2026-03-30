//! # Gateway Execution
//!
//! Agent execution engine for the AgentZero gateway.
//!
//! This crate provides the execution layer that:
//! - Creates and manages agent executors
//! - Converts execution events to gateway events
//! - Broadcasts events to connected clients
//! - Handles agent delegation with callbacks
//! - Spawns continuation turns after delegations complete
//!
//! ## Module Structure
//!
//! - `config` - Execution configuration and file system context
//! - `continuation` - Continuation spawning after delegation completion
//! - `handle` - Execution control handle (stop, pause, resume, cancel)
//! - `events` - Event conversion from runtime to gateway events
//! - `delegation` - Agent-to-agent delegation with callbacks
//! - `invoke` - Setup and executor building utilities
//! - `lifecycle` - Session and execution state management
//! - `runner` - Main execution runner

pub mod archiver;
pub mod composite_provider;
pub mod config;
pub mod continuation;
pub mod delegation;
pub mod distillation;
pub mod events;
pub mod handle;
pub mod invoke;
pub mod lifecycle;
pub mod pruning;
pub mod recall;
pub mod resource_provider;
pub mod runner;
pub mod ward_sync;
pub mod middleware;

// Re-export public types
pub use config::{ExecutionConfig, GatewayFileSystem};
pub use delegation::{
    handle_delegation_failure, handle_delegation_success, handle_subagent_completion,
    spawn_delegated_agent, DelegationContext, DelegationRegistry, DelegationRequest,
};
pub use events::convert_stream_event;
pub use handle::ExecutionHandle;
pub use continuation::{check_and_spawn_continuation, spawn_continuation_turn};
pub use lifecycle::{
    complete_execution, crash_execution, emit_agent_started, emit_delegation_completed,
    emit_delegation_started, get_or_create_session, start_execution,
    stop_execution, SessionSetup,
};
pub use invoke::{new_workspace_cache, WorkspaceCache};
pub use runner::ExecutionRunner;
pub use distillation::SessionDistiller;
pub use recall::{MemoryRecall, RecallResult, GraphContext, format_recalled_facts, format_combined_recall, format_prioritized_recall};
pub use archiver::SessionArchiver;
pub use composite_provider::CompositeResourceProvider;
pub use resource_provider::GatewayResourceProvider;
