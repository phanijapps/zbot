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

    /// Bind the legacy standalone WebSocket port (`websocket_port`).
    ///
    /// Off by default — the gateway now serves WebSocket traffic on the
    /// HTTP port at `/ws`, so mobile clients and reverse-proxy setups
    /// don't need a second firewall hole. Flip this on only if you have
    /// external integrations that hardcoded `ws://host:18790` and haven't
    /// migrated yet. Slated for removal in a future release.
    #[serde(default)]
    pub legacy_ws_port_enabled: bool,
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
            legacy_ws_port_enabled: false,
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

/// Resolve the effective bind host from a `DiscoveryConfig`.
///
/// Precedence:
/// 1. `advanced.bind_host` if present and parseable.
/// 2. `0.0.0.0` if `expose_to_lan = true`.
/// 3. `127.0.0.1` otherwise.
///
/// Garbage in `advanced.bind_host` falls back to loopback rather than
/// crashing — surfacing the misconfiguration via logs is better than
/// failing to start. Defensive: when an override is set but invalid,
/// we deliberately do NOT fall through to `expose_to_lan` — a typo
/// must never accidentally publish the daemon on 0.0.0.0.
pub fn resolve_bind_host(cfg: &discovery::DiscoveryConfig) -> std::net::IpAddr {
    use std::net::{IpAddr, Ipv4Addr};
    if let Some(s) = cfg.advanced.bind_host.as_deref() {
        if let Ok(parsed) = s.parse::<IpAddr>() {
            return parsed;
        }
        tracing::warn!(
            target: "discovery",
            "ignoring invalid network.advanced.bindHost={:?}; falling back to loopback for safety",
            s
        );
        return IpAddr::V4(Ipv4Addr::LOCALHOST);
    }
    if cfg.expose_to_lan {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    } else {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    }
}

#[cfg(test)]
mod resolve_bind_tests {
    use super::*;
    use discovery::{AdvancedConfig, DiscoveryConfig};
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn off_yields_loopback() {
        let cfg = DiscoveryConfig {
            expose_to_lan: false,
            ..Default::default()
        };
        assert_eq!(resolve_bind_host(&cfg), IpAddr::V4(Ipv4Addr::LOCALHOST));
    }

    #[test]
    fn on_yields_unspecified() {
        let cfg = DiscoveryConfig::default();
        assert!(cfg.expose_to_lan);
        assert_eq!(resolve_bind_host(&cfg), IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    }

    #[test]
    fn advanced_override_wins_when_present_and_valid() {
        let cfg = DiscoveryConfig {
            expose_to_lan: false, // would be loopback…
            advanced: AdvancedConfig {
                bind_host: Some("10.1.2.3".into()), // …but override wins
                http_port: 18791,
            },
            ..Default::default()
        };
        assert_eq!(
            resolve_bind_host(&cfg),
            IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3))
        );
    }

    #[test]
    fn advanced_override_with_garbage_falls_back_to_loopback() {
        let cfg = DiscoveryConfig {
            advanced: AdvancedConfig {
                bind_host: Some("not-an-ip".into()),
                http_port: 18791,
            },
            ..Default::default()
        };
        assert_eq!(resolve_bind_host(&cfg), IpAddr::V4(Ipv4Addr::LOCALHOST));
    }
}
