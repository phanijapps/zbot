// ============================================================================
// APP-TOOLS - Built-in Tools for agentzero Application
// ============================================================================

//! # App Tools
//!
//! Built-in tool implementations for the agentzero application.
//!
//! This crate provides concrete tool implementations that use
//! the abstractions defined in zero-core.

mod tools;

pub use tools::{builtin_tools_with_fs, core_tools, optional_tools, ListAgentsTool, ToolSettings};

// Re-export from zero-core
pub use zero_core::{Tool, FileSystemContext};
