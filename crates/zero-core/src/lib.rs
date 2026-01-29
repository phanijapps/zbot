//! # Zero Core
//!
//! Core traits, types, and errors for the Zero agent framework.
//!
//! ## Overview
//!
//! This crate provides the foundational abstractions:
//!
//! - [`Agent`] - Core agent interface
//! - [`Tool`] - Tool execution interface
//! - [`Toolset`] - Collection of tools
//! - [`ToolContext`] - Context provided to tools
//! - [`Event`] - Immutable conversation event
//! - [`InvocationContext`] - Context during agent invocation
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use zero_core::{Agent, Tool, Event, Result};
//! use std::sync::Arc;
//!
//! // All agents implement the Agent trait
//! // All tools implement the Tool trait
//! // Events are streamed as the agent executes
//! ```

// ============================================================================
// PUBLIC API RE-EXPORTS
// ============================================================================

pub mod agent;
pub mod tool;
pub mod context;
pub mod event;
pub mod types;
pub mod error;
pub mod callbacks;
pub mod filesystem;
pub mod policy;

// ============================================================================
// CONVENIENCE RE-EXPORTS
// ============================================================================

pub use agent::{Agent, EventStream};
pub use tool::{Tool, Toolset, ToolPredicate};
pub use context::{
    InvocationContext,
    ReadonlyContext,
    CallbackContext,
    ToolContext,
    RunConfig,
    StreamingMode,
};
pub use event::{Event, EventActions};
pub use types::{Content, Part};
pub use error::{ZeroError, Result};
pub use callbacks::{BeforeAgentCallback, AfterAgentCallback};
pub use filesystem::{FileSystemContext, NoFileSystemContext};
pub use policy::{ToolPermissions, ToolRiskLevel, ResourceLimits, CapabilityCategory};

// ============================================================================
// STATE PREFIX CONSTANTS
// ============================================================================

/// Key prefix for user preferences (persists across sessions)
pub const KEY_PREFIX_USER: &str = "user:";

/// Key prefix for application state (application-wide)
pub const KEY_PREFIX_APP: &str = "app:";

/// Key prefix for temporary data (cleared each turn)
pub const KEY_PREFIX_TEMP: &str = "temp:";
