// ============================================================================
// MCP TOOL
// ============================================================================

//! # MCP Tool
//!
//! A tool provided by an MCP server.

use serde_json::Value;

/// A tool provided by an MCP server
#[derive(Debug, Clone)]
pub struct McpTool {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Tool parameters schema (JSON Schema)
    pub parameters: Option<Value>,
}
