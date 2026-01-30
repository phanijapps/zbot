// ============================================================================
// TOOLS MODULE
// Tool execution system
// ============================================================================

//! # Tools Module
//!
//! Extensible tool registry and execution framework.
//!
//! ## Submodules
//!
//! - [`registry`]: Tool registry for managing available tools
//! - [`builtin`]: Built-in tools provided by the framework
//! - [`context`]: Execution context for tool operations
//! - [`error`]: Error types for tool operations

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod registry;
pub mod builtin;
pub mod context;
pub mod error;

pub use registry::ToolRegistry;
pub use builtin::{FileSystemContext, NoFileSystemContext};
pub use context::ToolContext;
pub use error::{ToolError as ToolExecError, ToolResult as ToolExecResult};

// Re-export from types
pub use crate::types::ToolCall;

// Re-export zero_core::Tool as the standard tool trait
pub use zero_core::Tool;
