// ============================================================================
// MCP SERVER CONFIG
// ============================================================================

//! # MCP Server Configuration
//!
//! Configuration for MCP servers with different transport types.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Configuration for an MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpServerConfig {
    /// Stdio-based MCP server (subprocess communication)
    #[serde(rename = "stdio")]
    Stdio {
        /// Optional server ID
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Server name
        name: String,
        /// Server description
        description: String,
        /// Command to execute
        command: String,
        /// Arguments for the command
        args: Vec<String>,
        /// Environment variables
        #[serde(skip_serializing_if = "Option::is_none")]
        env: Option<HashMap<String, String>>,
        /// Whether the server is enabled
        #[serde(default)]
        enabled: bool,
        /// Whether the server has been validated
        #[serde(default, skip_serializing_if = "Option::is_none")]
        validated: Option<bool>,
    },
    /// HTTP-based MCP server
    #[serde(rename = "http")]
    Http {
        /// Optional server ID
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Server name
        name: String,
        /// Server description
        description: String,
        /// Server URL
        url: String,
        /// HTTP headers
        #[serde(skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
        /// Whether the server is enabled
        #[serde(default)]
        enabled: bool,
        /// Whether the server has been validated
        #[serde(default, skip_serializing_if = "Option::is_none")]
        validated: Option<bool>,
    },
    /// SSE-based MCP server (Server-Sent Events)
    #[serde(rename = "sse")]
    Sse {
        /// Optional server ID
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Server name
        name: String,
        /// Server description
        description: String,
        /// Server URL
        url: String,
        /// HTTP headers
        #[serde(skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
        /// Whether the server is enabled
        #[serde(default)]
        enabled: bool,
        /// Whether the server has been validated
        #[serde(default, skip_serializing_if = "Option::is_none")]
        validated: Option<bool>,
    },
    /// Streamable HTTP MCP server
    #[serde(rename = "streamable-http")]
    StreamableHttp {
        /// Optional server ID
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Server name
        name: String,
        /// Server description
        description: String,
        /// Server URL
        url: String,
        /// HTTP headers
        #[serde(skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
        /// Whether the server is enabled
        #[serde(default)]
        enabled: bool,
        /// Whether the server has been validated
        #[serde(default, skip_serializing_if = "Option::is_none")]
        validated: Option<bool>,
    },
}

impl McpServerConfig {
    /// Get the server ID
    #[must_use]
    pub fn id(&self) -> String {
        match self {
            Self::Stdio { id, name, .. } => id.clone().unwrap_or_else(|| name.clone()),
            Self::Http { id, name, .. } => id.clone().unwrap_or_else(|| name.clone()),
            Self::Sse { id, name, .. } => id.clone().unwrap_or_else(|| name.clone()),
            Self::StreamableHttp { id, name, .. } => id.clone().unwrap_or_else(|| name.clone()),
        }
    }

    /// Get the server name
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Stdio { name, .. } => name,
            Self::Http { name, .. } => name,
            Self::Sse { name, .. } => name,
            Self::StreamableHttp { name, .. } => name,
        }
    }

    /// Check if the server is enabled
    #[must_use]
    pub fn enabled(&self) -> bool {
        match self {
            Self::Stdio { enabled, .. } => *enabled,
            Self::Http { enabled, .. } => *enabled,
            Self::Sse { enabled, .. } => *enabled,
            Self::StreamableHttp { enabled, .. } => *enabled,
        }
    }
}
