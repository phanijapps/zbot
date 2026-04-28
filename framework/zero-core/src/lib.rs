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
// HTTP USER-AGENT
// ============================================================================

/// User-Agent string attached to every outbound HTTP request made by the
/// z-bot system. Defined once here so all callers (LLM clients, MCP, gateway
/// connectors, CLI, tools) share the same value. Version tracks this crate's
/// Cargo.toml.
pub const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (compatible; Z-bot/",
    env!("CARGO_PKG_VERSION"),
    "; +https://github.com/phanijapps/zbot)"
);

// ============================================================================
// PUBLIC API RE-EXPORTS
// ============================================================================

pub mod agent;
pub mod callbacks;
pub mod capability;
pub mod connectors;
pub mod context;
pub mod error;
pub mod event;
pub mod filesystem;
pub mod multimodal;
pub mod policy;
pub mod registry;
pub mod tool;
pub mod types;

// ============================================================================
// CONVENIENCE RE-EXPORTS
// ============================================================================

pub use agent::{Agent, EventStream};
pub use callbacks::{AfterAgentCallback, BeforeAgentCallback};
pub use capability::{
    AgentCapabilities, Capability, CapabilityDescriptor, CapabilityKind, CapabilityProvider,
    CapabilityQuery,
};
pub use connectors::{CapabilityInfo, ConnectorResourceProvider};
pub use context::{
    CallbackContext, InvocationContext, ReadonlyContext, RunConfig, StreamingMode, ToolContext,
};
pub use error::{Result, ZeroError};
pub use event::{DelegateAction, Event, EventActions, RespondAction};
pub use filesystem::{FileSystemContext, NoFileSystemContext};
pub use policy::{CapabilityCategory, ResourceLimits, ToolPermissions, ToolRiskLevel};
pub use registry::{
    shared_unified_registry, CapabilityRegistry, CapabilityRouter, RoutingResult,
    SharedCapabilityRegistry, SharedUnifiedRegistry, UnifiedCapabilityRegistry,
};
pub use tool::{Tool, ToolPredicate, Toolset};
pub use types::{Content, Part};

// ============================================================================
// STRING UTILITIES
// ============================================================================

/// Truncate a string to at most `max_bytes` bytes at a valid UTF-8 char boundary.
/// Returns the original string if it fits, otherwise the largest valid slice.
#[inline]
pub fn truncate_str(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        s
    } else {
        &s[..s.floor_char_boundary(max_bytes)]
    }
}

// ============================================================================
// STATE PREFIX CONSTANTS
// ============================================================================

/// Key prefix for user preferences (persists across sessions)
pub const KEY_PREFIX_USER: &str = "user:";

/// Key prefix for application state (application-wide)
pub const KEY_PREFIX_APP: &str = "app:";

/// Key prefix for temporary data (cleared each turn)
pub const KEY_PREFIX_TEMP: &str = "temp:";
