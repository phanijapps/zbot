//! Interface enumeration with glob-pattern exclusion. Wrapped behind a
//! trait so unit tests don't touch real OS networking.

use std::net::IpAddr;
use tracing::debug;

/// One real network interface with at least one IPv4 address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    pub name: String,
    pub addrs: Vec<IpAddr>,
}

/// Trait so tests can inject a `MockEnumerator`.
pub trait InterfaceEnumerator: Send + Sync {
    fn enumerate(&self) -> Vec<Interface>;
}

/// Production impl using the `if-addrs` crate.
#[derive(Debug, Default)]
pub struct RealEnumerator;

impl InterfaceEnumerator for RealEnumerator {
    fn enumerate(&self) -> Vec<Interface> {
        let raw = if_addrs::get_if_addrs().unwrap_or_default();
        let mut grouped: std::collections::BTreeMap<String, Vec<IpAddr>> = Default::default();
        for iface in raw {
            if iface.is_loopback() {
                continue;
            }
            grouped
                .entry(iface.name.clone())
                .or_default()
                .push(iface.ip());
        }
        grouped
            .into_iter()
            .map(|(name, addrs)| Interface { name, addrs })
            .collect()
    }
}

/// Apply glob exclusion patterns. `patterns` is a list like
/// `["utun*", "tun*", "ppp*", "tap*"]`. Empty list = no exclusion.
pub fn filter_interfaces(ifaces: Vec<Interface>, patterns: &[String]) -> Vec<Interface> {
    ifaces
        .into_iter()
        .filter(|iface| {
            let blocked = patterns
                .iter()
                .any(|p| glob_match::glob_match(p, &iface.name));
            if blocked {
                debug!(target: "discovery", "interface {} excluded by pattern", iface.name);
            }
            !blocked
        })
        .collect()
}

/// Pick IPv4 addresses only (we don't advertise IPv6 in v1).
pub fn ipv4_only(ifaces: &[Interface]) -> Vec<(String, std::net::Ipv4Addr)> {
    let mut out = Vec::new();
    for iface in ifaces {
        for addr in &iface.addrs {
            if let IpAddr::V4(v4) = addr {
                if v4.is_link_local() || v4.is_loopback() {
                    continue;
                }
                out.push((iface.name.clone(), *v4));
            }
        }
    }
    out
}

#[cfg(test)]
pub struct MockEnumerator(pub Vec<Interface>);

#[cfg(test)]
impl InterfaceEnumerator for MockEnumerator {
    fn enumerate(&self) -> Vec<Interface> {
        self.0.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn iface(name: &str, ip: [u8; 4]) -> Interface {
        Interface {
            name: name.to_string(),
            addrs: vec![IpAddr::V4(Ipv4Addr::from(ip))],
        }
    }

    #[test]
    fn filter_excludes_default_tunnel_patterns() {
        let ifaces = vec![
            iface("en0", [192, 168, 1, 42]),
            iface("utun0", [10, 200, 0, 1]),
            iface("tun1", [10, 8, 0, 1]),
            iface("ppp0", [172, 16, 0, 1]),
            iface("tap0", [10, 9, 0, 1]),
        ];
        let patterns = vec![
            "utun*".to_string(),
            "tun*".to_string(),
            "ppp*".to_string(),
            "tap*".to_string(),
        ];
        let kept = filter_interfaces(ifaces, &patterns);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].name, "en0");
    }

    #[test]
    fn filter_keeps_real_lan_interfaces() {
        let ifaces = vec![
            iface("en0", [192, 168, 1, 42]),
            iface("eth0", [10, 0, 0, 5]),
            iface("wlan0", [192, 168, 0, 100]),
        ];
        let patterns = vec!["utun*".to_string(), "tun*".to_string()];
        let kept = filter_interfaces(ifaces, &patterns);
        assert_eq!(kept.len(), 3);
    }

    #[test]
    fn empty_patterns_is_no_op() {
        let ifaces = vec![iface("anything", [1, 2, 3, 4])];
        let kept = filter_interfaces(ifaces.clone(), &[]);
        assert_eq!(kept, ifaces);
    }

    #[test]
    fn ipv4_only_strips_link_local_and_loopback() {
        let ifaces = vec![
            Interface {
                name: "en0".into(),
                addrs: vec![
                    IpAddr::V4(Ipv4Addr::new(192, 168, 1, 42)),
                    IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1)), // link-local
                ],
            },
            Interface {
                name: "lo".into(),
                addrs: vec![IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))],
            },
        ];
        let pairs = ipv4_only(&ifaces);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "en0");
        assert_eq!(pairs[0].1, Ipv4Addr::new(192, 168, 1, 42));
    }
}
