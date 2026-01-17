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
pub use builtin::{builtin_tools_with_fs, FileSystemContext, NoFileSystemContext};
pub use context::ToolContext;
pub use error::{ToolError as ToolExecError, ToolResult as ToolExecResult};

// Re-export from types
pub use crate::types::ToolCall;

// ============================================================================
// TOOL TRAIT
// ============================================================================

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

/// Tool trait that all tools must implement
#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the name of the tool
    fn name(&self) -> &str;

    /// Returns a description of what the tool does
    fn description(&self) -> &str;

    /// Returns the JSON schema for the tool's parameters (optional)
    fn parameters_schema(&self) -> Option<Value> {
        None
    }

    /// Executes the tool with the given arguments
    async fn execute(
        &self,
        ctx: Arc<ToolContext>,
        args: Value,
    ) -> ToolExecResult<Value>;

    /// Returns whether this is a long-running operation (default: false)
    fn is_long_running(&self) -> bool {
        false
    }
}
