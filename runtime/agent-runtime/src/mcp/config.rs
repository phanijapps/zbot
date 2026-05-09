// ============================================================================
// MCP SERVER CONFIG
// ============================================================================

//! # MCP Server Configuration
//!
//! Configuration for MCP servers with different transport types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn stdio(name: &str, id: Option<&str>, enabled: bool) -> McpServerConfig {
        McpServerConfig::Stdio {
            id: id.map(str::to_string),
            name: name.to_string(),
            description: "desc".to_string(),
            command: "node".to_string(),
            args: vec!["server.js".to_string()],
            env: None,
            enabled,
            validated: None,
        }
    }

    #[test]
    fn id_falls_back_to_name_when_missing() {
        let c = stdio("my-server", None, false);
        assert_eq!(c.id(), "my-server");
        assert_eq!(c.name(), "my-server");
        assert!(!c.enabled());
    }

    #[test]
    fn id_uses_explicit_id_when_present() {
        let c = stdio("my-server", Some("server-id-1"), true);
        assert_eq!(c.id(), "server-id-1");
        assert_eq!(c.name(), "my-server");
        assert!(c.enabled());
    }

    #[test]
    fn http_id_name_enabled() {
        let c = McpServerConfig::Http {
            id: Some("h1".to_string()),
            name: "http-srv".to_string(),
            description: String::new(),
            url: "https://example.com".to_string(),
            headers: None,
            enabled: true,
            validated: None,
        };
        assert_eq!(c.id(), "h1");
        assert_eq!(c.name(), "http-srv");
        assert!(c.enabled());

        let c2 = McpServerConfig::Http {
            id: None,
            name: "http-srv".to_string(),
            description: String::new(),
            url: "https://example.com".to_string(),
            headers: None,
            enabled: false,
            validated: None,
        };
        assert_eq!(c2.id(), "http-srv");
        assert!(!c2.enabled());
    }

    #[test]
    fn sse_id_name_enabled() {
        let c = McpServerConfig::Sse {
            id: None,
            name: "sse-srv".to_string(),
            description: String::new(),
            url: "https://example.com/sse".to_string(),
            headers: None,
            enabled: true,
            validated: None,
        };
        assert_eq!(c.id(), "sse-srv");
        assert_eq!(c.name(), "sse-srv");
        assert!(c.enabled());
    }

    #[test]
    fn streamable_http_id_name_enabled() {
        let c = McpServerConfig::StreamableHttp {
            id: Some("sh1".to_string()),
            name: "stream-srv".to_string(),
            description: "d".to_string(),
            url: "https://x".to_string(),
            headers: None,
            enabled: false,
            validated: Some(true),
        };
        assert_eq!(c.id(), "sh1");
        assert_eq!(c.name(), "stream-srv");
        assert!(!c.enabled());
    }

    #[test]
    fn deserialize_stdio_yaml() {
        let yaml = r#"
type: stdio
name: fs-server
description: filesystem
command: npx
args: ["-y", "@some/server"]
env:
  KEY: value
enabled: true
"#;
        let c: McpServerConfig = serde_yaml::from_str(yaml).unwrap();
        match &c {
            McpServerConfig::Stdio {
                name,
                command,
                args,
                env,
                enabled,
                ..
            } => {
                assert_eq!(name, "fs-server");
                assert_eq!(command, "npx");
                assert_eq!(args.len(), 2);
                assert!(env.as_ref().unwrap().contains_key("KEY"));
                assert!(*enabled);
            }
            _ => panic!("expected stdio variant"),
        }
        assert_eq!(c.id(), "fs-server");
    }

    #[test]
    fn deserialize_http_minimal_yaml() {
        // No `enabled`, no `id`, no `headers` — defaults must apply.
        let yaml = "type: http\nname: h\ndescription: d\nurl: https://x.test\n";
        let c: McpServerConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(c, McpServerConfig::Http { .. }));
        assert!(!c.enabled());
        assert_eq!(c.id(), "h");
    }

    #[test]
    fn deserialize_sse_yaml() {
        let yaml = "type: sse\nname: s\ndescription: d\nurl: https://sse.test\nenabled: true\n";
        let c: McpServerConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(c, McpServerConfig::Sse { .. }));
        assert!(c.enabled());
    }

    #[test]
    fn deserialize_streamable_http_yaml() {
        let yaml =
            "type: streamable-http\nname: sh\ndescription: d\nurl: https://sh.test\nenabled: false\n";
        let c: McpServerConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(c, McpServerConfig::StreamableHttp { .. }));
        assert_eq!(c.id(), "sh");
        assert!(!c.enabled());
    }

    #[test]
    fn round_trip_serialize_then_deserialize() {
        let c = stdio("rt", Some("rt-id"), true);
        let s = serde_json::to_string(&c).unwrap();
        // Sanity: type tag present
        assert!(s.contains("\"type\":\"stdio\""));
        let back: McpServerConfig = serde_json::from_str(&s).unwrap();
        assert_eq!(back.id(), "rt-id");
        assert_eq!(back.name(), "rt");
        assert!(back.enabled());
    }
}
