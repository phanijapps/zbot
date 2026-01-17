// ============================================================================
// ZEROTOOLS - Built-in Tools for Agent Runtime
// ============================================================================

//! # ZeroTools
//!
//! Built-in tool implementations for the agent-runtime framework.
//!
//! This crate provides concrete tool implementations that depend on
//! the abstractions defined in agent-runtime. This keeps agent-runtime
//! generic and reusable while providing useful tools out of the box.

mod tools;

pub use tools::builtin_tools_with_fs;
pub use agent_runtime::tools::builtin::FileSystemContext;

// Re-export commonly used types from agent-runtime
pub use agent_runtime::tools::Tool;
