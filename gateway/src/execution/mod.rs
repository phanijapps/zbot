//! # Execution Module
//!
//! Agent execution integration for the gateway.
//!
//! This module provides the execution layer that:
//! - Creates and manages agent executors
//! - Converts execution events to gateway events
//! - Broadcasts events to connected clients
//! - Handles agent delegation with callbacks
//!
//! ## Module Structure
//!
//! - `config` - Execution configuration and file system context
//! - `handle` - Execution control handle (stop, pause, resume, cancel)
//! - `events` - Event conversion from runtime to gateway events
//! - `delegation` - Agent-to-agent delegation with callbacks
//! - `runner` - Main execution runner

mod config;
mod delegation;
mod events;
mod handle;
mod runner;

// Re-export public types
pub use config::{ExecutionConfig, GatewayFileSystem};
pub use delegation::{
    handle_subagent_completion, DelegationContext, DelegationRegistry, DelegationRequest,
};
pub use events::convert_stream_event;
pub use handle::ExecutionHandle;
pub use runner::ExecutionRunner;
