//! # Zero Tool
//!
//! Tool system and registry for the Zero framework.

pub mod registry;
pub mod function;
pub mod context_impl;

// Re-export from zero-core
pub use zero_core::{Tool, ToolContext, Toolset};

// Re-export from our modules
pub use registry::ToolRegistry;
pub use function::FunctionTool;
pub use context_impl::ToolContextImpl;
