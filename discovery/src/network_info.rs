//! Builds the `NetworkInfo` snapshot for the `/api/network/info` endpoint.

use crate::config::DiscoveryConfig;
use crate::interfaces::{filter_interfaces, ipv4_only, InterfaceEnumerator};
use serde::Serialize;

/// Snapshot returned by `GET /api/network/info`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInfo {
    pub expose_to_lan: bool,
    pub bind_host: String,
    pub port: u16,
    pub hostname_urls: Vec<String>,
    pub ip_urls: Vec<String>,
    pub mdns: MdnsStatus,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MdnsStatus {
    pub active: bool,
    pub interfaces: Vec<String>,
    pub alias_claimed: bool,
    pub instance_id: String,
}

/// Pure function: given config + bound port + observed mdns status, build the snapshot.
#[allow(clippy::too_many_arguments)]
pub fn collect_network_info(
    cfg: &DiscoveryConfig,
    bind_host: &str,
    port: u16,
    enumerator: &dyn InterfaceEnumerator,
    mdns_active: bool,
    alias_claimed: bool,
    instance_name: &str,
    instance_id: &str,
) -> NetworkInfo {
    if !cfg.expose_to_lan {
        return NetworkInfo {
            expose_to_lan: false,
            bind_host: bind_host.to_string(),
            port,
            hostname_urls: Vec::new(),
            ip_urls: Vec::new(),
            mdns: MdnsStatus {
                active: false,
                interfaces: Vec::new(),
                alias_claimed: false,
                instance_id: instance_id.to_string(),
            },
        };
    }

    let kept = filter_interfaces(enumerator.enumerate(), &cfg.discovery.exclude_interfaces);
    let interface_names: Vec<String> = kept.iter().map(|i| i.name.clone()).collect();
    let ipv4_pairs = ipv4_only(&kept);

    let mut hostname_urls = Vec::new();
    if alias_claimed {
        hostname_urls.push(format!("http://{}", cfg.discovery.hostname_alias));
    }
    hostname_urls.push(format!(
        "http://{}-agentzero.local",
        sanitize_for_hostname(instance_name)
    ));

    let ip_urls = ipv4_pairs
        .iter()
        .map(|(_, ip)| format!("http://{}:{}", ip, port))
        .collect();

    NetworkInfo {
        expose_to_lan: true,
        bind_host: bind_host.to_string(),
        port,
        hostname_urls,
        ip_urls,
        mdns: MdnsStatus {
            active: mdns_active,
            interfaces: interface_names,
            alias_claimed,
            instance_id: instance_id.to_string(),
        },
    }
}

/// mDNS hostnames cannot contain underscores or uppercase. Conservative replacement.
pub fn sanitize_for_hostname(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interfaces::{Interface, MockEnumerator};
    use std::net::{IpAddr, Ipv4Addr};

    fn cfg_default() -> DiscoveryConfig {
        DiscoveryConfig::default()
    }

    fn enumerator_with(ifaces: Vec<Interface>) -> MockEnumerator {
        MockEnumerator(ifaces)
    }

    #[test]
    fn disabled_yields_empty_urls() {
        let mut cfg = cfg_default();
        cfg.expose_to_lan = false;
        let enumerator = enumerator_with(vec![]);
        let info = collect_network_info(
            &cfg,
            "127.0.0.1",
            18791,
            &enumerator,
            false,
            false,
            "phani-mbp",
            "uuid",
        );
        assert!(!info.expose_to_lan);
        assert!(info.hostname_urls.is_empty());
        assert!(info.ip_urls.is_empty());
        assert!(!info.mdns.active);
    }

    #[test]
    fn enabled_with_alias_yields_two_hostname_urls() {
        let cfg = cfg_default();
        let enumerator = enumerator_with(vec![Interface {
            name: "en0".into(),
            addrs: vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 42))],
        }]);
        let info = collect_network_info(
            &cfg,
            "0.0.0.0",
            18791,
            &enumerator,
            true,
            true,
            "phani-mbp",
            "uuid",
        );
        assert!(info.expose_to_lan);
        assert_eq!(info.hostname_urls.len(), 2);
        assert_eq!(info.hostname_urls[0], "http://agentzero.local");
        assert_eq!(info.hostname_urls[1], "http://phani-mbp-agentzero.local");
        assert_eq!(info.ip_urls, vec!["http://192.168.1.42:18791"]);
        assert!(info.mdns.alias_claimed);
    }

    #[test]
    fn enabled_with_alias_collision_only_per_instance_url() {
        let cfg = cfg_default();
        let enumerator = enumerator_with(vec![Interface {
            name: "en0".into(),
            addrs: vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 42))],
        }]);
        let info = collect_network_info(
            &cfg,
            "0.0.0.0",
            18791,
            &enumerator,
            true,
            false,
            "phani-mbp",
            "uuid",
        );
        assert_eq!(info.hostname_urls.len(), 1);
        assert_eq!(info.hostname_urls[0], "http://phani-mbp-agentzero.local");
    }

    #[test]
    fn enabled_with_excluded_interface_drops_its_ip() {
        let cfg = cfg_default();
        let enumerator = enumerator_with(vec![
            Interface {
                name: "en0".into(),
                addrs: vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 42))],
            },
            Interface {
                name: "utun0".into(),
                addrs: vec![IpAddr::V4(Ipv4Addr::new(10, 200, 0, 1))],
            },
        ]);
        let info = collect_network_info(
            &cfg,
            "0.0.0.0",
            18791,
            &enumerator,
            true,
            true,
            "phani-mbp",
            "uuid",
        );
        assert_eq!(info.ip_urls, vec!["http://192.168.1.42:18791"]);
        assert!(!info.mdns.interfaces.iter().any(|n| n == "utun0"));
    }

    #[test]
    fn instance_name_with_spaces_or_underscores_sanitized() {
        let cfg = cfg_default();
        let enumerator = enumerator_with(vec![Interface {
            name: "en0".into(),
            addrs: vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 42))],
        }]);
        let info = collect_network_info(
            &cfg,
            "0.0.0.0",
            18791,
            &enumerator,
            true,
            true,
            "Phani's MBP_2",
            "uuid",
        );
        assert_eq!(
            info.hostname_urls[1],
            "http://phani-s-mbp-2-agentzero.local"
        );
    }
}
