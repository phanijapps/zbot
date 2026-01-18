//! # Zero MCP
//!
//! Model Context Protocol (MCP) client and tool integration for the Zero framework.
//!
//! This crate provides:
//! - MCP server configuration types
//! - Core MCP client trait and RMCP-based implementations
//! - Tool wrapper that integrates MCP tools with the Zero framework
//! - Schema sanitization for LLM compatibility
//! - Connection lifecycle management with pooling
//! - Tool filtering with predicates
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐
//! │   McpToolset    │ Implements Toolset trait
//! │  (filters)      │
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │   McpConnection │ Lifecycle tracking
//! │  (managed)      │
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │   McpClient     │ RMCP-based protocol
//! │  (rmcp SDK)     │
//! └─────────────────┘
//! ```

pub mod config;
pub mod client;
pub mod connection;
pub mod error;
pub mod filter;
pub mod schema;
pub mod tool;
pub mod toolset;

// Re-export commonly used types
pub use config::{McpCommand, McpServerConfig, McpTransport};
pub use client::{McpClient, McpServerInfo, McpToolDefinition, MockMcpClient};
pub use connection::{ConnectionState, McpConnection, McpConnectionPool};
pub use error::{McpError, Result};
pub use filter::{ToolFilter, ToolPredicate, accept_all, accept_none, by_names, by_prefix, exclude_names, with_property};
pub use schema::{extract_input_schema, sanitize_tool_schema};
pub use tool::{McpTool, McpToolBuilder};
pub use toolset::{McpToolset, McpToolsetBuilder};

/// Re-export Tool and Toolset from zero-core for convenience
pub use zero_core::{Tool, Toolset};

// Prelude module for convenient imports
pub mod prelude {
    pub use crate::config::*;
    pub use crate::client::*;
    pub use crate::connection::*;
    pub use crate::error::*;
    pub use crate::filter::*;
    pub use crate::schema::*;
    pub use crate::tool::*;
    pub use crate::toolset::*;
    pub use zero_core::{Tool, Toolset};
}
