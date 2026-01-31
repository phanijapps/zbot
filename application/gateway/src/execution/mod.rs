//! # Execution Module
//!
//! Agent execution integration for the gateway.
//!
//! This module provides the execution layer that:
//! - Creates and manages agent executors
//! - Converts execution events to gateway events
//! - Broadcasts events to connected clients
//! - Handles agent delegation with callbacks

mod delegation;
mod runner;

pub use delegation::{DelegationContext, DelegationRegistry, handle_subagent_completion};
pub use runner::{ExecutionRunner, ExecutionConfig, ExecutionHandle};
