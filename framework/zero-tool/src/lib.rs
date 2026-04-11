//! # Zero Tool
//!
//! Tool system and registry for the Zero framework.

pub mod context_impl;
pub mod function;
pub mod registry;

// Re-export from zero-core
pub use zero_core::{Tool, ToolContext, Toolset};

// Re-export from our modules
pub use context_impl::ToolContextImpl;
pub use function::FunctionTool;
pub use registry::ToolRegistry;
