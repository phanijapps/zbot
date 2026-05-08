//! Configuration for LAN discovery, deserialized from `settings.json`'s
//! `network` block.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Top-level network/discovery configuration. Maps 1:1 to `settings.json`'s
/// `network` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryConfig {
    #[serde(default = "default_expose_to_lan")]
    pub expose_to_lan: bool,

    #[serde(default)]
    pub discovery: DiscoveryDetails,

    #[serde(default)]
    pub advanced: AdvancedConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryDetails {
    /// Instance label. None → derive from `gethostname()` with `.local` stripped.
    #[serde(default)]
    pub instance_name: Option<String>,

    #[serde(default = "default_service_type")]
    pub service_type: String,

    #[serde(default = "default_hostname_alias")]
    pub hostname_alias: String,

    #[serde(default)]
    pub txt_records: BTreeMap<String, String>,

    #[serde(default = "default_exclude_interfaces")]
    pub exclude_interfaces: Vec<String>,

    /// Stable instance UUID. Auto-generated and persisted on first start.
    #[serde(default)]
    pub instance_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedConfig {
    /// Override bind host. None → derive from `expose_to_lan`.
    #[serde(default)]
    pub bind_host: Option<String>,

    #[serde(default = "default_http_port")]
    pub http_port: u16,
}

fn default_expose_to_lan() -> bool {
    true
}
fn default_service_type() -> String {
    "_zbot._tcp.local.".to_string()
}
fn default_hostname_alias() -> String {
    "zbot.local".to_string()
}
fn default_exclude_interfaces() -> Vec<String> {
    vec![
        "utun*".to_string(),
        "tun*".to_string(),
        "ppp*".to_string(),
        "tap*".to_string(),
    ]
}
fn default_http_port() -> u16 {
    18791
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            expose_to_lan: default_expose_to_lan(),
            discovery: DiscoveryDetails::default(),
            advanced: AdvancedConfig::default(),
        }
    }
}

impl Default for DiscoveryDetails {
    fn default() -> Self {
        Self {
            instance_name: None,
            service_type: default_service_type(),
            hostname_alias: default_hostname_alias(),
            txt_records: BTreeMap::new(),
            exclude_interfaces: default_exclude_interfaces(),
            instance_id: None,
        }
    }
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            bind_host: None,
            http_port: default_http_port(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let cfg = DiscoveryConfig::default();
        assert!(cfg.expose_to_lan);
        assert_eq!(cfg.discovery.service_type, "_zbot._tcp.local.");
        assert_eq!(cfg.discovery.hostname_alias, "zbot.local");
        assert_eq!(
            cfg.discovery.exclude_interfaces,
            vec!["utun*", "tun*", "ppp*", "tap*"]
        );
        assert!(cfg.discovery.txt_records.is_empty());
        assert!(cfg.discovery.instance_name.is_none());
        assert!(cfg.discovery.instance_id.is_none());
        assert!(cfg.advanced.bind_host.is_none());
        assert_eq!(cfg.advanced.http_port, 18791);
    }

    #[test]
    fn deserializes_empty_object() {
        let cfg: DiscoveryConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg, DiscoveryConfig::default());
    }

    #[test]
    fn deserializes_partial_with_defaults_filling_in() {
        let json = r#"{ "exposeToLan": false }"#;
        let cfg: DiscoveryConfig = serde_json::from_str(json).unwrap();
        assert!(!cfg.expose_to_lan);
        assert_eq!(cfg.discovery.hostname_alias, "zbot.local");
        assert_eq!(cfg.advanced.http_port, 18791);
    }

    #[test]
    fn deserializes_full_block() {
        let json = r#"{
            "exposeToLan": true,
            "discovery": {
                "instanceName": "phani-mbp",
                "serviceType": "_zbot._tcp.local.",
                "hostnameAlias": "zbot.local",
                "txtRecords": { "env": "prod" },
                "excludeInterfaces": ["utun*"],
                "instanceId": "11111111-2222-3333-4444-555555555555"
            },
            "advanced": {
                "bindHost": "0.0.0.0",
                "httpPort": 9000
            }
        }"#;
        let cfg: DiscoveryConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.discovery.instance_name.as_deref(), Some("phani-mbp"));
        assert_eq!(
            cfg.discovery.txt_records.get("env").map(String::as_str),
            Some("prod")
        );
        assert_eq!(cfg.advanced.bind_host.as_deref(), Some("0.0.0.0"));
        assert_eq!(cfg.advanced.http_port, 9000);
    }
}
