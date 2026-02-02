//! # Execution Module
//!
//! Agent execution integration for the gateway.
//!
//! This module provides the execution layer that:
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

mod config;
mod continuation;
mod delegation;
mod events;
mod handle;
mod invoke;
mod lifecycle;
mod runner;

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
    emit_delegation_started, get_or_create_session, save_messages, start_execution,
    stop_execution, SessionSetup,
};
pub use runner::ExecutionRunner;
