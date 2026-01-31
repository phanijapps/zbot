// ============================================================================
// MCP MODULE
// Model Context Protocol support
// ============================================================================

//! # MCP Module
//!
//! Model Context Protocol client for external tool integration.
//!
//! Supports multiple transports (stdio, HTTP, SSE) for connecting
//! to MCP servers that provide additional tools and capabilities.
//!
//! ## Module Structure
//!
//! - [`config`]: Server configuration types
//! - [`manager`]: Connection manager for MCP servers
//! - [`client`]: Common trait for MCP clients
//! - [`http`]: HTTP transport implementation
//! - [`stdio`]: Stdio transport implementation (subprocess)
//! - [`sse`]: Server-Sent Events transport implementation
//! - [`error`]: Error types for MCP operations
//! - [`tool`]: Tool types provided by MCP servers

#![warn(missing_docs)]
#![warn(clippy::all)]

mod config;
mod manager;
mod client;
mod http;
mod stdio;
mod sse;
mod error;
mod tool;

// Public exports
pub use config::McpServerConfig;
pub use manager::McpManager;
pub use client::McpClient;
pub use error::McpError;
pub use tool::McpTool;
