//! # Gateway Configuration
//!
//! Configuration for the gateway server.

use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// Gateway server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Host address to bind to.
    #[serde(default = "default_host")]
    pub host: IpAddr,

    /// WebSocket port.
    #[serde(default = "default_ws_port")]
    pub websocket_port: u16,

    /// HTTP port.
    #[serde(default = "default_http_port")]
    pub http_port: u16,

    /// Enable CORS for development.
    #[serde(default)]
    pub cors_enabled: bool,

    /// Allowed CORS origins (if cors_enabled).
    #[serde(default)]
    pub cors_origins: Vec<String>,

    /// Path to static files for web dashboard (optional).
    #[serde(default)]
    pub static_dir: Option<String>,

    /// Enable serving the web dashboard.
    #[serde(default = "default_serve_dashboard")]
    pub serve_dashboard: bool,
}

fn default_serve_dashboard() -> bool {
    true
}

fn default_host() -> IpAddr {
    "127.0.0.1".parse().unwrap()
}

fn default_ws_port() -> u16 {
    crate::DEFAULT_WS_PORT
}

fn default_http_port() -> u16 {
    crate::DEFAULT_HTTP_PORT
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            websocket_port: default_ws_port(),
            http_port: default_http_port(),
            cors_enabled: true,
            cors_origins: vec![
                "http://localhost:1420".to_string(), // Tauri dev
                "http://localhost:3000".to_string(), // Web dev
            ],
            static_dir: None,
            serve_dashboard: true,
        }
    }
}

impl GatewayConfig {
    /// Create a new config with custom ports.
    pub fn with_ports(ws_port: u16, http_port: u16) -> Self {
        Self {
            websocket_port: ws_port,
            http_port,
            ..Default::default()
        }
    }

    /// Get the WebSocket bind address.
    pub fn ws_addr(&self) -> String {
        format!("{}:{}", self.host, self.websocket_port)
    }

    /// Get the HTTP bind address.
    pub fn http_addr(&self) -> String {
        format!("{}:{}", self.host, self.http_port)
    }
}
