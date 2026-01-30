//! # Execution Module
//!
//! Agent execution integration for the gateway.
//!
//! This module provides the execution layer that:
//! - Creates and manages agent executors
//! - Converts execution events to gateway events
//! - Broadcasts events to connected clients

mod runner;

pub use runner::{ExecutionRunner, ExecutionConfig, ExecutionHandle};
