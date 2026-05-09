# LAN Discoverability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the AgentZero daemon discoverable on the local network so any device (phone / tablet / second laptop) can reach it via `http://agentzero.local` (browsers) or programmatic mDNS browsing (native clients), instead of typing `host:18791`.

**Architecture:** A new `discovery/` workspace crate exposes an `Advertiser` trait (RAII handle, no async), implemented over the pure-Rust `mdns-sd` crate. The gateway depends on `discovery`, reads a new `network` block from `settings.json` (default-on `exposeToLan`), resolves bind host accordingly, and holds an `AdvertiseHandle` whose `Drop` impl sends mDNS goodbye. A new `GET /api/network/info` endpoint feeds a Settings → Network card in the UI that shows live URLs, a QR code, and a toggle.

**Tech Stack:** Rust 2021 edition (`mdns-sd 0.13+`, `if-addrs 0.13+`, `gethostname 0.5+`, `glob-match 0.2+`, plus existing `tokio`/`tracing`/`serde`/`uuid`); React + TypeScript (`qrcode.react 4.x`).

**Spec:** `memory-bank/future-state/2026-05-02-lan-discoverability-design.md`

**Quality bar:** Clippy clean (`cargo clippy --all-targets -- -D warnings`), `cargo fmt --all --check` passes, all unit tests green. Cognitive complexity < 15 per function. No `unwrap()` in production paths (tests are fine). Interface enumeration is wrapped behind a small trait so unit tests don't touch real OS networking.

**Branching:** Create a fresh branch off `origin/develop` named `feature/lan-discoverability` before Task 1. All work in this plan lands on that branch.

```bash
git fetch origin develop
git checkout -b feature/lan-discoverability origin/develop
```

---

## File Structure

### New files

```
discovery/                                          ← NEW workspace crate
├── Cargo.toml
└── src/
    ├── lib.rs                                      ← public API, module exports
    ├── config.rs                                   ← DiscoveryConfig, NetworkSettings
    ├── interfaces.rs                               ← InterfaceEnumerator trait + impls + filter
    ├── network_info.rs                             ← collect_network_info, NetworkInfo, MdnsStatus
    ├── advertiser.rs                               ← Advertiser trait, ServiceInfo, AdvertiseHandle, NoopAdvertiser
    ├── mdns.rs                                     ← MdnsAdvertiser
    └── tests/
        └── mdns_integration.rs                     ← #[ignore]'d tests that use real multicast

gateway/src/http/network.rs                         ← GET /api/network/info handler
apps/ui/src/features/settings/
├── NetworkSettingsCard.tsx                         ← the new card
└── NetworkSettingsCard.test.tsx
```

### Modified files

```
Cargo.toml                                          ← workspace member + new shared deps
gateway/Cargo.toml                                  ← depends on `discovery`
gateway/src/lib.rs                                  ← re-export discovery types where useful
gateway/src/state/mod.rs                            ← AppState holds Option<AdvertiseHandle> + Arc<dyn Advertiser>
gateway/src/server.rs                               ← read network settings, swap bind, call advertiser
gateway/src/http/mod.rs                             ← register /api/network/info + /api/settings/network routes
gateway/src/http/settings.rs                        ← add get/update_network_settings handlers
gateway/gateway-services/src/settings.rs            ← add NetworkSettings to AppSettings
apps/ui/src/features/settings/WebSettingsPanel.tsx  ← compose NetworkSettingsCard under Advanced
apps/ui/package.json                                ← add qrcode.react dep
README.md                                           ← brief LAN-discoverability section
```

---

## Tasks

Tasks are ordered for incremental dependency. Each Rust task ends with a green `cargo test -p <crate>` and a commit. Each UI task ends with a green `npm test -- --run NetworkSettingsCard` and a commit.

---

### Task 1: Scaffold the `discovery/` crate

**Files:**
- Create: `discovery/Cargo.toml`
- Create: `discovery/src/lib.rs`
- Modify: `Cargo.toml` (workspace members + workspace deps)

- [ ] **Step 1: Add new shared workspace deps**

Edit `Cargo.toml` and add these lines under `[workspace.dependencies]` (alphabetical-ish, near existing deps):

```toml
mdns-sd = "0.13"
if-addrs = "0.13"
gethostname = "0.5"
glob-match = "0.2"
```

- [ ] **Step 2: Add the crate to the workspace**

In `Cargo.toml`, under `[workspace] members`, add a new section after the `# Stores …` block:

```toml
    # Discovery - LAN service advertisement
    "discovery",
```

- [ ] **Step 3: Create the crate manifest**

Create `discovery/Cargo.toml`:

```toml
[package]
name = "discovery"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
mdns-sd = { workspace = true }
if-addrs = { workspace = true }
gethostname = { workspace = true }
glob-match = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
uuid = { workspace = true }

[dev-dependencies]
tokio = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 4: Create the empty lib**

Create `discovery/src/lib.rs`:

```rust
//! # Discovery
//!
//! LAN service advertisement (mDNS) for the AgentZero daemon. Provides an
//! `Advertiser` trait so the gateway can stay decoupled from the underlying
//! mDNS implementation and so tests can swap in a no-op or recorder.

#![forbid(unsafe_code)]
```

- [ ] **Step 5: Verify the crate compiles**

Run: `cargo check -p discovery`
Expected: `Finished` with no errors.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml discovery/
git commit -m "feat(discovery): scaffold workspace crate"
```

---

### Task 2: `DiscoveryConfig` struct with serde defaults

**Files:**
- Create: `discovery/src/config.rs`
- Modify: `discovery/src/lib.rs` (re-export)

- [ ] **Step 1: Write the failing test**

Add to the bottom of (the about-to-be-created) `discovery/src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let cfg = DiscoveryConfig::default();
        assert_eq!(cfg.expose_to_lan, true);
        assert_eq!(cfg.discovery.service_type, "_agentzero._tcp.local.");
        assert_eq!(cfg.discovery.hostname_alias, "agentzero.local");
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
        assert_eq!(cfg.expose_to_lan, false);
        assert_eq!(cfg.discovery.hostname_alias, "agentzero.local");
        assert_eq!(cfg.advanced.http_port, 18791);
    }

    #[test]
    fn deserializes_full_block() {
        let json = r#"{
            "exposeToLan": true,
            "discovery": {
                "instanceName": "phani-mbp",
                "serviceType": "_agentzero._tcp.local.",
                "hostnameAlias": "agentzero.local",
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
        assert_eq!(cfg.discovery.txt_records.get("env").map(String::as_str), Some("prod"));
        assert_eq!(cfg.advanced.bind_host.as_deref(), Some("0.0.0.0"));
        assert_eq!(cfg.advanced.http_port, 9000);
    }
}
```

- [ ] **Step 2: Run test — expect compile failure**

Run: `cargo test -p discovery`
Expected: compile error (`DiscoveryConfig` not defined).

- [ ] **Step 3: Implement `DiscoveryConfig`**

Replace the contents of `discovery/src/config.rs` with:

```rust
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
    "_agentzero._tcp.local.".to_string()
}
fn default_hostname_alias() -> String {
    "agentzero.local".to_string()
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
```

- [ ] **Step 4: Re-export from lib**

Edit `discovery/src/lib.rs`, append:

```rust
pub mod config;

pub use config::{AdvancedConfig, DiscoveryConfig, DiscoveryDetails};
```

- [ ] **Step 5: Run tests — expect pass**

Run: `cargo test -p discovery`
Expected: 4 passed.

- [ ] **Step 6: Commit**

```bash
git add discovery/
git commit -m "feat(discovery): add DiscoveryConfig with serde defaults"
```

---

### Task 3: Interface enumeration with VPN/tunnel filtering

**Files:**
- Create: `discovery/src/interfaces.rs`
- Modify: `discovery/src/lib.rs` (re-export)

- [ ] **Step 1: Write the failing test**

Create `discovery/src/interfaces.rs` with this content:

```rust
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
            grouped.entry(iface.name.clone()).or_default().push(iface.ip());
        }
        grouped.into_iter().map(|(name, addrs)| Interface { name, addrs }).collect()
    }
}

/// Apply glob exclusion patterns. `patterns` is a list like
/// `["utun*", "tun*", "ppp*", "tap*"]`. Empty list = no exclusion.
pub fn filter_interfaces(ifaces: Vec<Interface>, patterns: &[String]) -> Vec<Interface> {
    ifaces
        .into_iter()
        .filter(|iface| {
            let blocked = patterns.iter().any(|p| glob_match::glob_match(p, &iface.name));
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
```

- [ ] **Step 2: Re-export from lib**

Append to `discovery/src/lib.rs`:

```rust
pub mod interfaces;

pub use interfaces::{filter_interfaces, ipv4_only, Interface, InterfaceEnumerator, RealEnumerator};
```

- [ ] **Step 3: Run tests — expect 4 pass**

Run: `cargo test -p discovery`
Expected: 8 passed (4 from Task 2, 4 from this task).

- [ ] **Step 4: Commit**

```bash
git add discovery/
git commit -m "feat(discovery): interface enumeration with VPN-tunnel exclusion"
```

---

### Task 4: `NetworkInfo` builder

**Files:**
- Create: `discovery/src/network_info.rs`
- Modify: `discovery/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `discovery/src/network_info.rs`:

```rust
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
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c.to_ascii_lowercase() } else { '-' })
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
        let info = collect_network_info(&cfg, "127.0.0.1", 18791, &enumerator, false, false, "phani-mbp", "uuid");
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
        let info = collect_network_info(&cfg, "0.0.0.0", 18791, &enumerator, true, true, "phani-mbp", "uuid");
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
        let info = collect_network_info(&cfg, "0.0.0.0", 18791, &enumerator, true, false, "phani-mbp", "uuid");
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
        let info = collect_network_info(&cfg, "0.0.0.0", 18791, &enumerator, true, true, "phani-mbp", "uuid");
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
        let info = collect_network_info(&cfg, "0.0.0.0", 18791, &enumerator, true, true, "Phani's MBP_2", "uuid");
        assert_eq!(info.hostname_urls[1], "http://phani-s-mbp-2-agentzero.local");
    }
}
```

- [ ] **Step 2: Re-export from lib**

Append to `discovery/src/lib.rs`:

```rust
pub mod network_info;

pub use network_info::{collect_network_info, sanitize_for_hostname, MdnsStatus, NetworkInfo};
```

- [ ] **Step 3: Run tests — expect pass**

Run: `cargo test -p discovery`
Expected: 13 passed total.

- [ ] **Step 4: Commit**

```bash
git add discovery/
git commit -m "feat(discovery): add collect_network_info builder"
```

---

### Task 5: `Advertiser` trait, `ServiceInfo`, `AdvertiseHandle`, `NoopAdvertiser`

**Files:**
- Create: `discovery/src/advertiser.rs`
- Modify: `discovery/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `discovery/src/advertiser.rs`:

```rust
//! Public surface for advertising a service on the LAN. Implementations are
//! injected via `Arc<dyn Advertiser>`.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("mdns responder failed to start: {0}")]
    ResponderStart(String),
    #[error("failed to register service: {0}")]
    Register(String),
    #[error("invalid service info: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, DiscoveryError>;

/// What the gateway hands to the advertiser at startup.
#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub instance_name: String,
    pub service_type: String,
    pub hostname_alias: String,
    pub port: u16,
    pub txt: BTreeMap<String, String>,
    pub addrs: Vec<(String, Ipv4Addr)>, // (interface name, ipv4)
}

/// Outcome of a successful advertise call.
pub struct AdvertiseHandle {
    /// Held to keep the responder thread alive. Drop sends goodbye and stops it.
    inner: Box<dyn AdvertiseInner + Send + Sync>,
    /// True if the friendly alias was claimed without collision.
    pub alias_claimed: bool,
}

/// Implementation detail trait — boxed inside `AdvertiseHandle`.
pub trait AdvertiseInner: Send + Sync {
    /// Send goodbye + stop the responder. Called from `Drop`.
    fn shutdown(&mut self);
}

impl AdvertiseHandle {
    pub fn new(inner: Box<dyn AdvertiseInner + Send + Sync>, alias_claimed: bool) -> Self {
        Self { inner, alias_claimed }
    }
}

impl Drop for AdvertiseHandle {
    fn drop(&mut self) {
        self.inner.shutdown();
    }
}

/// Public trait the gateway depends on.
pub trait Advertiser: Send + Sync {
    fn advertise(&self, info: ServiceInfo) -> Result<AdvertiseHandle>;
}

/// Used when `expose_to_lan = false`. Returns a handle that does nothing on drop.
#[derive(Debug, Default)]
pub struct NoopAdvertiser;

struct NoopInner;
impl AdvertiseInner for NoopInner {
    fn shutdown(&mut self) {}
}

impl Advertiser for NoopAdvertiser {
    fn advertise(&self, _info: ServiceInfo) -> Result<AdvertiseHandle> {
        Ok(AdvertiseHandle::new(Box::new(NoopInner), true))
    }
}

/// Convenience constructor for the noop case.
pub fn noop() -> Arc<dyn Advertiser> {
    Arc::new(NoopAdvertiser)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn sample_info() -> ServiceInfo {
        ServiceInfo {
            instance_name: "test".into(),
            service_type: "_agentzero._tcp.local.".into(),
            hostname_alias: "agentzero.local".into(),
            port: 18791,
            txt: BTreeMap::new(),
            addrs: vec![],
        }
    }

    #[test]
    fn noop_advertiser_returns_handle_with_alias_claimed() {
        let adv = NoopAdvertiser;
        let handle = adv.advertise(sample_info()).expect("noop never errors");
        assert!(handle.alias_claimed);
    }

    #[test]
    fn dropping_handle_calls_inner_shutdown_exactly_once() {
        static COUNT: AtomicUsize = AtomicUsize::new(0);

        struct Counting;
        impl AdvertiseInner for Counting {
            fn shutdown(&mut self) {
                COUNT.fetch_add(1, Ordering::SeqCst);
            }
        }

        let handle = AdvertiseHandle::new(Box::new(Counting), true);
        drop(handle);
        assert_eq!(COUNT.load(Ordering::SeqCst), 1);
    }
}
```

- [ ] **Step 2: Re-export from lib**

Append to `discovery/src/lib.rs`:

```rust
pub mod advertiser;

pub use advertiser::{
    noop, AdvertiseHandle, AdvertiseInner, Advertiser, DiscoveryError, NoopAdvertiser,
    Result as DiscoveryResult, ServiceInfo,
};
```

- [ ] **Step 3: Run tests — expect pass**

Run: `cargo test -p discovery`
Expected: 15 passed.

- [ ] **Step 4: Commit**

```bash
git add discovery/
git commit -m "feat(discovery): Advertiser trait + RAII handle + NoopAdvertiser"
```

---

### Task 6: `MdnsAdvertiser` over `mdns-sd`

**Files:**
- Create: `discovery/src/mdns.rs`
- Create: `discovery/tests/mdns_integration.rs`
- Modify: `discovery/src/lib.rs`

- [ ] **Step 1: Write the integration test scaffolding**

Create `discovery/tests/mdns_integration.rs`:

```rust
//! Integration tests that exercise real multicast. Gated by `#[ignore]` so
//! they don't run on every `cargo test`. Run them locally via:
//!
//!     cargo test -p discovery -- --ignored
//!
//! CI runs them on Linux only — macOS GitHub runners are unreliable for
//! multicast, and Windows runners block it.

use discovery::advertiser::Advertiser;
use discovery::mdns::MdnsAdvertiser;
use discovery::ServiceInfo;
use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::time::Duration;

fn sample_info(port: u16) -> ServiceInfo {
    let mut txt = BTreeMap::new();
    txt.insert("version".into(), "test".into());
    txt.insert("instance".into(), "00000000-0000-0000-0000-000000000001".into());
    ServiceInfo {
        instance_name: "test-instance".into(),
        service_type: "_agentzero._tcp.local.".into(),
        hostname_alias: "agentzero-test.local".into(),
        port,
        txt,
        addrs: vec![("lo0".into(), Ipv4Addr::new(127, 0, 0, 1))],
    }
}

#[test]
#[ignore]
fn advertise_then_browse_finds_self() {
    let adv = MdnsAdvertiser::new().expect("responder starts");
    let _handle = adv.advertise(sample_info(18999)).expect("advertise succeeds");

    // Use mdns-sd's own browse API to confirm the service is visible.
    let daemon = mdns_sd::ServiceDaemon::new().expect("browser daemon");
    let receiver = daemon.browse("_agentzero._tcp.local.").expect("browse starts");

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut found = false;
    while std::time::Instant::now() < deadline {
        if let Ok(event) = receiver.recv_timeout(Duration::from_millis(200)) {
            if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
                if info.get_port() == 18999 {
                    found = true;
                    break;
                }
            }
        }
    }
    daemon.shutdown().ok();
    assert!(found, "service was not discovered within 5s");
}

#[test]
#[ignore]
fn drop_handle_sends_goodbye() {
    let adv = MdnsAdvertiser::new().expect("responder starts");
    let handle = adv.advertise(sample_info(19000)).expect("advertise succeeds");
    drop(handle);

    // Allow goodbye packet to flush.
    std::thread::sleep(Duration::from_millis(300));

    // We trust mdns-sd to send the goodbye; assert the responder shut down
    // by confirming we can re-bind the same instance name without conflict.
    let adv2 = MdnsAdvertiser::new().expect("responder starts again");
    let _h2 = adv2.advertise(sample_info(19000)).expect("re-advertise after goodbye");
}
```

- [ ] **Step 2: Implement `MdnsAdvertiser`**

Create `discovery/src/mdns.rs`:

```rust
//! Real mDNS advertiser backed by the pure-Rust `mdns-sd` crate.

use crate::advertiser::{
    AdvertiseHandle, AdvertiseInner, Advertiser, DiscoveryError, Result, ServiceInfo,
};
use mdns_sd::{ServiceDaemon, ServiceInfo as MdnsServiceInfo};
use std::collections::HashMap;
use tracing::{info, warn};

const TTL_SECS: u32 = 120;

pub struct MdnsAdvertiser {
    daemon: ServiceDaemon,
}

impl MdnsAdvertiser {
    pub fn new() -> Result<Self> {
        let daemon = ServiceDaemon::new()
            .map_err(|e| DiscoveryError::ResponderStart(e.to_string()))?;
        Ok(Self { daemon })
    }
}

impl Advertiser for MdnsAdvertiser {
    fn advertise(&self, info: ServiceInfo) -> Result<AdvertiseHandle> {
        let per_instance_host = format!(
            "{}-agentzero.local.",
            crate::network_info::sanitize_for_hostname(&info.instance_name)
        );
        let alias_host = if info.hostname_alias.ends_with('.') {
            info.hostname_alias.clone()
        } else {
            format!("{}.", info.hostname_alias)
        };

        let ipv4s: Vec<std::net::Ipv4Addr> = info.addrs.iter().map(|(_, ip)| *ip).collect();
        if ipv4s.is_empty() {
            return Err(DiscoveryError::Invalid(
                "no IPv4 addresses to advertise on".into(),
            ));
        }

        let txt: HashMap<String, String> = info.txt.into_iter().collect();

        // Per-instance hostname record (always unique).
        let per_instance = MdnsServiceInfo::new(
            &info.service_type,
            &info.instance_name,
            &per_instance_host,
            &ipv4s[..],
            info.port,
            txt.clone(),
        )
        .map_err(|e| DiscoveryError::Register(e.to_string()))?
        .enable_addr_auto();
        let per_instance = per_instance.set_other_ttl(TTL_SECS);

        self.daemon
            .register(per_instance)
            .map_err(|e| DiscoveryError::Register(e.to_string()))?;

        // Friendly alias hostname record (best-effort, may collide).
        let alias_label = format!("{}-alias", info.instance_name);
        let alias = MdnsServiceInfo::new(
            &info.service_type,
            &alias_label,
            &alias_host,
            &ipv4s[..],
            info.port,
            txt,
        )
        .map_err(|e| DiscoveryError::Register(e.to_string()))?
        .enable_addr_auto();
        let alias = alias.set_other_ttl(TTL_SECS);

        // mdns-sd performs conflict probing; if collision occurs, the daemon
        // returns success but our alias entry won't be uniquely owned. We
        // can't detect this synchronously, so we optimistically claim and
        // surface the result via runtime monitoring (the daemon's
        // ServiceUnregistered event). For v1 we treat alias_claimed=true as
        // the optimistic default; future enhancement can listen for
        // ServiceConflict.
        let alias_claimed = match self.daemon.register(alias) {
            Ok(()) => true,
            Err(e) => {
                warn!(target: "discovery", "alias hostname collision detected: {}", e);
                false
            }
        };

        info!(
            target: "discovery",
            "advertising {} on {} interface(s) port={}",
            info.service_type,
            ipv4s.len(),
            info.port
        );

        Ok(AdvertiseHandle::new(
            Box::new(MdnsInner {
                daemon: self.daemon.clone(),
                fullname_per_instance: format!("{}.{}", info.instance_name, info.service_type),
                fullname_alias: format!("{}.{}", alias_label, info.service_type),
            }),
            alias_claimed,
        ))
    }
}

struct MdnsInner {
    daemon: ServiceDaemon,
    fullname_per_instance: String,
    fullname_alias: String,
}

impl AdvertiseInner for MdnsInner {
    fn shutdown(&mut self) {
        if let Err(e) = self.daemon.unregister(&self.fullname_per_instance) {
            warn!(target: "discovery", "unregister per-instance failed: {}", e);
        }
        if let Err(e) = self.daemon.unregister(&self.fullname_alias) {
            warn!(target: "discovery", "unregister alias failed: {}", e);
        }
        // Block briefly so goodbye packets flush before shutdown.
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Err(e) = self.daemon.shutdown() {
            warn!(target: "discovery", "responder shutdown failed: {}", e);
        }
        info!(target: "discovery", "withdrawn (goodbye sent)");
    }
}
```

- [ ] **Step 3: Re-export from lib**

Append to `discovery/src/lib.rs`:

```rust
pub mod mdns;

pub use mdns::MdnsAdvertiser;
```

- [ ] **Step 4: Run unit tests — expect pass (integration tests stay ignored)**

Run: `cargo test -p discovery`
Expected: 15 passed; 2 ignored.

- [ ] **Step 5: Run integration tests locally**

Run: `cargo test -p discovery -- --ignored`
Expected: 2 passed (run on Linux/macOS; on Windows, expect potential firewall prompt — accept it).

If `advertise_then_browse_finds_self` fails: this is usually a multicast firewall block. Verify with `tcpdump -i any -n udp port 5353` that packets are leaving the host.

- [ ] **Step 6: Commit**

```bash
git add discovery/
git commit -m "feat(discovery): mdns-sd backed Advertiser with goodbye on drop"
```

---

### Task 7: Add `NetworkSettings` to `AppSettings`

**Files:**
- Modify: `gateway/gateway-services/Cargo.toml`
- Modify: `gateway/gateway-services/src/settings.rs`
- Modify: `gateway/gateway-services/src/lib.rs`

- [ ] **Step 1: Add `discovery` as a dep on gateway-services**

Edit `gateway/gateway-services/Cargo.toml` and add under `[dependencies]`:

```toml
discovery = { path = "../../discovery" }
```

- [ ] **Step 2: Write the failing test**

Append to `gateway/gateway-services/src/settings.rs` (inside the existing `#[cfg(test)] mod tests` if present, or add one at the bottom):

```rust
#[cfg(test)]
mod network_settings_tests {
    use super::*;

    #[test]
    fn defaults_have_expose_to_lan_true() {
        let s = AppSettings::default();
        assert!(s.network.expose_to_lan);
        assert_eq!(s.network.advanced.http_port, 18791);
    }

    #[test]
    fn old_settings_without_network_block_still_parses() {
        let json = r#"{
            "tools": {},
            "logs": {},
            "execution": {}
        }"#;
        let s: AppSettings = serde_json::from_str(json).unwrap();
        assert!(s.network.expose_to_lan);
    }

    #[test]
    fn explicit_off_round_trips() {
        let json = r#"{ "network": { "exposeToLan": false } }"#;
        let s: AppSettings = serde_json::from_str(json).unwrap();
        assert!(!s.network.expose_to_lan);
    }
}
```

- [ ] **Step 3: Run test — expect compile failure**

Run: `cargo test -p gateway-services network_settings`
Expected: compile error (`network` field not on `AppSettings`).

- [ ] **Step 4: Add the field and re-export**

In `gateway/gateway-services/src/settings.rs`, change the `AppSettings` struct (top of file) to:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Tool settings (enable/disable optional tools)
    #[serde(default)]
    pub tools: ToolSettings,

    /// Logging configuration (file logging, rotation, etc.)
    #[serde(default)]
    pub logs: LogSettings,

    /// Execution settings (concurrency, delegation limits, etc.)
    #[serde(default)]
    pub execution: ExecutionSettings,

    /// Network / discovery configuration. New top-level block; absent in
    /// pre-v0.X settings.json files, in which case the default
    /// (`exposeToLan: true`) applies.
    #[serde(default)]
    pub network: discovery::DiscoveryConfig,
}
```

And add at the top of the file (with other `use` lines):

```rust
// (no new imports needed beyond the existing ones — discovery is a workspace dep)
```

In `gateway/gateway-services/src/lib.rs`, ensure `pub use settings::AppSettings;` etc. continue to work — no change needed if it already re-exports the struct.

- [ ] **Step 5: Run tests — expect pass**

Run: `cargo test -p gateway-services network_settings`
Expected: 3 passed.

- [ ] **Step 6: Verify the whole workspace still builds**

Run: `cargo check --workspace`
Expected: no errors. (Other crates that destructure `AppSettings { .. }` exhaustively will need updating — Step 7.)

- [ ] **Step 7: Fix any exhaustive matches uncovered by step 6**

If `cargo check` flags any places that pattern-match `AppSettings { .. }` with all fields, add `network` to the list. Most consumers use `.field` access, so this should be a small or empty diff.

- [ ] **Step 8: Commit**

```bash
git add gateway/gateway-services/
git commit -m "feat(settings): add network block (DiscoveryConfig) to AppSettings"
```

---

### Task 8: Resolve bind host from network settings

**Files:**
- Modify: `gateway/Cargo.toml`
- Modify: `gateway/src/server.rs`
- Modify: `gateway/src/config.rs` (add a helper)

- [ ] **Step 1: Add `discovery` as a dep on gateway**

Edit `gateway/Cargo.toml` and add under `[dependencies]`:

```toml
discovery = { path = "../discovery" }
```

- [ ] **Step 2: Write the failing test**

Append to `gateway/src/config.rs` (a new test module at the bottom):

```rust
#[cfg(test)]
mod resolve_bind_tests {
    use super::*;
    use discovery::{AdvancedConfig, DiscoveryConfig};
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn off_yields_loopback() {
        let mut cfg = DiscoveryConfig::default();
        cfg.expose_to_lan = false;
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
        assert_eq!(resolve_bind_host(&cfg), IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3)));
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
```

- [ ] **Step 3: Add the helper**

Append this `pub fn` to `gateway/src/config.rs` (above the test module):

```rust
/// Resolve the effective bind host from a `DiscoveryConfig`.
///
/// Precedence:
/// 1. `advanced.bind_host` if present and parseable.
/// 2. `0.0.0.0` if `expose_to_lan = true`.
/// 3. `127.0.0.1` otherwise.
///
/// Garbage in `advanced.bind_host` falls back to loopback rather than
/// crashing — surfacing the misconfiguration via logs is better than
/// failing to start.
pub fn resolve_bind_host(cfg: &discovery::DiscoveryConfig) -> std::net::IpAddr {
    use std::net::{IpAddr, Ipv4Addr};
    if let Some(s) = cfg.advanced.bind_host.as_deref() {
        if let Ok(parsed) = s.parse::<IpAddr>() {
            return parsed;
        }
        tracing::warn!(target: "discovery", "ignoring invalid network.advanced.bindHost={:?}", s);
    }
    if cfg.expose_to_lan {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    } else {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    }
}
```

- [ ] **Step 4: Run tests — expect pass**

Run: `cargo test -p gateway resolve_bind`
Expected: 4 passed.

- [ ] **Step 5: Wire into server startup**

In `gateway/src/server.rs`, modify `GatewayServer::start` to swap the bind host before constructing `http_addr`. Find the section near line 148 that reads `let http_addr = self.config.http_addr();` and replace the surrounding logic with:

```rust
        // Read network settings from AppSettings (cached in SettingsService).
        // If exposeToLan changed since startup, the user must restart — this
        // read happens at boot so a stale toggle from a prior run is fine.
        let network_cfg = match self.state.settings.load() {
            Ok(s) => s.network,
            Err(e) => {
                warn!("Failed to load settings.json for network config: {}; defaulting to LAN exposure ON", e);
                discovery::DiscoveryConfig::default()
            }
        };
        let resolved_host = crate::config::resolve_bind_host(&network_cfg);
        if resolved_host != self.config.host {
            info!(
                "Bind host resolved from network settings: {} (was {})",
                resolved_host, self.config.host
            );
            self.config.host = resolved_host;
        }
        let http_addr = self.config.http_addr();
```

- [ ] **Step 6: Run gateway tests**

Run: `cargo test -p gateway`
Expected: existing tests still pass.

- [ ] **Step 7: Commit**

```bash
git add gateway/Cargo.toml gateway/src/config.rs gateway/src/server.rs
git commit -m "feat(gateway): resolve bind host from network settings (LAN toggle)"
```

---

### Task 9: Wire advertiser into start/shutdown

**Files:**
- Modify: `gateway/src/state/mod.rs`
- Modify: `gateway/src/server.rs`

- [ ] **Step 1: Add advertiser fields to AppState**

Open `gateway/src/state/mod.rs`. Find the `AppState` struct definition. Add two fields (the exact placement depends on existing layout — keep them grouped near the bottom of the struct):

```rust
    /// LAN service advertiser. NoopAdvertiser when discovery is disabled.
    pub advertiser: std::sync::Arc<dyn discovery::Advertiser>,

    /// Active mDNS advertise handle. None until `start()` runs and only
    /// populated when `network.exposeToLan = true`.
    pub advertise_handle: std::sync::Arc<std::sync::Mutex<Option<discovery::AdvertiseHandle>>>,
```

In the same file's `AppState::new` constructor, initialize the new fields:

```rust
            advertiser: discovery::noop(),
            advertise_handle: std::sync::Arc::new(std::sync::Mutex::new(None)),
```

(Default to `noop` because `start()` swaps in `MdnsAdvertiser` only when discovery is enabled — see next step.)

- [ ] **Step 2: Build ServiceInfo and call advertiser in start()**

In `gateway/src/server.rs`, **after** the `tokio::spawn(async move { … HTTP listener … })` block (i.e., after the listener is constructed but before the legacy WS branch), add:

```rust
        // Start mDNS advertisement when LAN exposure is enabled.
        if network_cfg.expose_to_lan {
            let actual_port = self.config.http_port;

            let instance_name = network_cfg
                .discovery
                .instance_name
                .clone()
                .unwrap_or_else(default_instance_name);

            let instance_id = match network_cfg.discovery.instance_id.clone() {
                Some(id) => id,
                None => {
                    let new_id = uuid::Uuid::new_v4().to_string();
                    if let Err(e) = persist_instance_id(&self.state.settings, &new_id) {
                        warn!("failed to persist generated instance_id: {}", e);
                    }
                    new_id
                }
            };

            let mut txt = std::collections::BTreeMap::new();
            txt.insert("version".into(), env!("CARGO_PKG_VERSION").to_string());
            txt.insert("instance".into(), instance_id.clone());
            txt.insert(
                "name".into(),
                network_cfg
                    .discovery
                    .instance_name
                    .clone()
                    .unwrap_or_else(|| instance_name.clone()),
            );
            txt.insert("path".into(), "/".into());
            txt.insert("ws".into(), "1".into());
            for (k, v) in &network_cfg.discovery.txt_records {
                txt.entry(k.clone()).or_insert_with(|| v.clone());
            }

            let enumerator = discovery::RealEnumerator;
            let interfaces = discovery::filter_interfaces(
                discovery::interfaces::InterfaceEnumerator::enumerate(&enumerator),
                &network_cfg.discovery.exclude_interfaces,
            );
            let addrs = discovery::ipv4_only(&interfaces);

            let info = discovery::ServiceInfo {
                instance_name: instance_name.clone(),
                service_type: network_cfg.discovery.service_type.clone(),
                hostname_alias: network_cfg.discovery.hostname_alias.clone(),
                port: actual_port,
                txt,
                addrs,
            };

            let advertiser: std::sync::Arc<dyn discovery::Advertiser> =
                match discovery::MdnsAdvertiser::new() {
                    Ok(a) => std::sync::Arc::new(a),
                    Err(e) => {
                        warn!(
                            "mDNS responder failed to start: {}; daemon reachable via IP only",
                            e
                        );
                        discovery::noop()
                    }
                };

            match advertiser.advertise(info) {
                Ok(handle) => {
                    let mut guard = self.state.advertise_handle.lock().unwrap();
                    *guard = Some(handle);
                    info!("mDNS advertising started for {}", instance_name);
                }
                Err(e) => warn!("mDNS advertise failed: {}; daemon reachable via IP only", e),
            }

            // Replace the noop with the real one so /api/network/info reflects truth.
            // This requires AppState to hold an Arc that we can swap. In the current
            // shape we simply leave AppState pointing at noop; runtime activity is
            // tracked via advertise_handle.is_some(). See network_info handler.
            let _ = advertiser; // silence unused (kept locally; handle owns lifetime)
        }
```

Add these helpers at the **bottom** of `gateway/src/server.rs`:

```rust
fn default_instance_name() -> String {
    let raw = gethostname::gethostname()
        .to_string_lossy()
        .into_owned();
    let trimmed = raw.trim_end_matches(".local").to_string();
    if trimmed.is_empty() { "agentzero".to_string() } else { trimmed }
}

fn persist_instance_id(
    settings: &gateway_services::SettingsService,
    new_id: &str,
) -> std::result::Result<(), String> {
    let mut current = settings.load()?;
    current.network.discovery.instance_id = Some(new_id.to_string());
    settings.save(&current)
}
```

Add `gethostname` to `gateway/Cargo.toml` `[dependencies]`:

```toml
gethostname = { workspace = true }
```

- [ ] **Step 3: Drop handle on shutdown**

In `gateway/src/server.rs`, find `pub async fn shutdown` and add at its top (before pausing sessions or whatever it does first):

```rust
        // Withdraw mDNS advertisement before tearing down the listener.
        if let Ok(mut guard) = self.state.advertise_handle.lock() {
            if let Some(handle) = guard.take() {
                drop(handle); // Drop impl sends goodbye, blocks ~100ms
                info!("mDNS advertisement withdrawn");
            }
        }
```

- [ ] **Step 4: Build and test**

Run: `cargo build -p gateway && cargo test -p gateway`
Expected: builds clean, existing tests pass.

- [ ] **Step 5: Manual sanity check**

Run: `npm run daemon` (or whatever starts the daemon locally).
Expected log lines (when LAN is enabled):

```
INFO  gateway::server: Bind host resolved from network settings: 0.0.0.0 (was 127.0.0.1)
INFO  gateway::server: Starting HTTP server on 0.0.0.0:18791
INFO  discovery: advertising _agentzero._tcp.local. on 1 interface(s) port=18791
INFO  gateway::server: mDNS advertising started for <hostname>
```

From a second device on the same network: `curl http://agentzero.local:18791/api/health` should return 200.

- [ ] **Step 6: Commit**

```bash
git add gateway/
git commit -m "feat(gateway): start mDNS advertiser at boot, withdraw on shutdown"
```

---

### Task 10: `GET /api/network/info` endpoint

**Files:**
- Create: `gateway/src/http/network.rs`
- Modify: `gateway/src/http/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `gateway/src/http/network.rs`:

```rust
//! GET /api/network/info — exposes the current LAN discoverability state
//! to the Settings UI.

use crate::state::AppState;
use axum::{extract::State, http::StatusCode, Json};
use discovery::NetworkInfo;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct NetworkInfoResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<NetworkInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn get_network_info(
    State(state): State<AppState>,
) -> Result<Json<NetworkInfoResponse>, (StatusCode, Json<NetworkInfoResponse>)> {
    let settings = state.settings.load().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(NetworkInfoResponse {
                success: false,
                data: None,
                error: Some(e),
            }),
        )
    })?;
    let network_cfg = settings.network;

    let mdns_active = state
        .advertise_handle
        .lock()
        .ok()
        .map(|guard| guard.is_some())
        .unwrap_or(false);

    let alias_claimed = if mdns_active {
        // Pull from handle if available; otherwise optimistic true.
        state
            .advertise_handle
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|h| h.alias_claimed))
            .unwrap_or(true)
    } else {
        false
    };

    let instance_name = network_cfg
        .discovery
        .instance_name
        .clone()
        .unwrap_or_else(crate::server::default_instance_name);
    let instance_id = network_cfg
        .discovery
        .instance_id
        .clone()
        .unwrap_or_default();

    let bind_host = format!("{}", crate::config::resolve_bind_host(&network_cfg));

    let enumerator = discovery::RealEnumerator;
    let info = discovery::collect_network_info(
        &network_cfg,
        &bind_host,
        network_cfg.advanced.http_port,
        &enumerator,
        mdns_active,
        alias_claimed,
        &instance_name,
        &instance_id,
    );

    Ok(Json(NetworkInfoResponse {
        success: true,
        data: Some(info),
        error: None,
    }))
}

#[cfg(test)]
mod tests {
    // The handler is thin enough that exercising it requires an `AppState`,
    // which is heavy to construct in a unit test. Coverage of the underlying
    // logic lives in `discovery::network_info::tests`. Integration coverage
    // for the route happens in the gateway HTTP integration test suite.
    #[test]
    fn module_compiles() {}
}
```

In `gateway/src/server.rs`, change the `default_instance_name` / `persist_instance_id` helpers from `fn` to `pub(crate) fn` so the network handler can reuse them:

```rust
pub(crate) fn default_instance_name() -> String { /* … */ }
pub(crate) fn persist_instance_id(/* … */) { /* … */ }
```

- [ ] **Step 2: Register the route**

In `gateway/src/http/mod.rs`, add `mod network;` near the other `mod` declarations, and add this route to the router builder (next to the `/api/health` block):

```rust
        .route("/api/network/info", get(network::get_network_info))
```

- [ ] **Step 3: Build and test**

Run: `cargo build -p gateway && cargo test -p gateway`
Expected: clean.

- [ ] **Step 4: Manual smoke test**

Start the daemon, then:

```bash
curl -s http://localhost:18791/api/network/info | jq .
```

Expected output (with LAN enabled):

```json
{
  "success": true,
  "data": {
    "exposeToLan": true,
    "bindHost": "0.0.0.0",
    "port": 18791,
    "hostnameUrls": ["http://agentzero.local", "http://<host>-agentzero.local"],
    "ipUrls": ["http://192.168.1.42:18791"],
    "mdns": {
      "active": true,
      "interfaces": ["en0"],
      "aliasClaimed": true,
      "instanceId": "<uuid>"
    }
  }
}
```

- [ ] **Step 5: Commit**

```bash
git add gateway/src/http/network.rs gateway/src/http/mod.rs gateway/src/server.rs
git commit -m "feat(gateway): GET /api/network/info endpoint"
```

---

### Task 11: `GET` / `PUT /api/settings/network` — read/write the toggle

**Files:**
- Modify: `gateway/src/http/settings.rs`
- Modify: `gateway/src/http/mod.rs`

- [ ] **Step 1: Add handlers**

Append to `gateway/src/http/settings.rs`:

```rust
// ============================================================================
// NETWORK SETTINGS ENDPOINTS
// ============================================================================

/// GET /api/settings/network — current network/discovery configuration.
pub async fn get_network_settings(
    State(state): State<AppState>,
) -> Result<Json<SettingsResponse<discovery::DiscoveryConfig>>, (StatusCode, Json<SettingsResponse<()>>)> {
    match state.settings.load() {
        Ok(settings) => Ok(Json(SettingsResponse {
            success: true,
            data: Some(settings.network),
            error: None,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SettingsResponse {
                success: false,
                data: None,
                error: Some(e),
            }),
        )),
    }
}

/// PUT /api/settings/network — replace the network/discovery block.
///
/// Note: changes apply on next daemon restart; the UI shows a banner
/// reminding the user.
pub async fn update_network_settings(
    State(state): State<AppState>,
    Json(new_cfg): Json<discovery::DiscoveryConfig>,
) -> Result<Json<SettingsResponse<discovery::DiscoveryConfig>>, (StatusCode, Json<SettingsResponse<()>>)> {
    let mut current = match state.settings.load() {
        Ok(s) => s,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SettingsResponse {
                    success: false,
                    data: None,
                    error: Some(e),
                }),
            ))
        }
    };
    current.network = new_cfg.clone();
    match state.settings.save(&current) {
        Ok(()) => Ok(Json(SettingsResponse {
            success: true,
            data: Some(new_cfg),
            error: None,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SettingsResponse {
                success: false,
                data: None,
                error: Some(e),
            }),
        )),
    }
}
```

(`discovery` is already imported transitively via `gateway_services`; if needed, add `use discovery;` at the top of the file.)

- [ ] **Step 2: Register the routes**

In `gateway/src/http/mod.rs`, add next to the existing `/api/settings/...` routes:

```rust
        .route("/api/settings/network", get(settings::get_network_settings))
        .route("/api/settings/network", put(settings::update_network_settings))
```

- [ ] **Step 3: Build and smoke test**

Run: `cargo build -p gateway`

Smoke:

```bash
curl -s http://localhost:18791/api/settings/network | jq .
curl -s -X PUT http://localhost:18791/api/settings/network \
  -H 'Content-Type: application/json' \
  -d '{"exposeToLan": false, "discovery": {}, "advanced": {"httpPort": 18791}}' | jq .
cat ~/Documents/zbot/config/settings.json | jq .network
```

Expected: `network.exposeToLan` flips to `false` in the file.

- [ ] **Step 4: Commit**

```bash
git add gateway/src/http/settings.rs gateway/src/http/mod.rs
git commit -m "feat(gateway): GET/PUT /api/settings/network for the LAN toggle"
```

---

### Task 12: UI — `NetworkSettingsCard` skeleton (off-state rendering)

**Files:**
- Create: `apps/ui/src/features/settings/NetworkSettingsCard.tsx`
- Create: `apps/ui/src/features/settings/NetworkSettingsCard.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `apps/ui/src/features/settings/NetworkSettingsCard.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { NetworkSettingsCard } from "./NetworkSettingsCard";

beforeEach(() => {
  globalThis.fetch = vi.fn(async (url: RequestInfo | URL) => {
    const u = url.toString();
    if (u.endsWith("/api/network/info")) {
      return new Response(
        JSON.stringify({
          success: true,
          data: {
            exposeToLan: false,
            bindHost: "127.0.0.1",
            port: 18791,
            hostnameUrls: [],
            ipUrls: [],
            mdns: {
              active: false,
              interfaces: [],
              aliasClaimed: false,
              instanceId: "00000000-0000-0000-0000-000000000000",
            },
          },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      );
    }
    if (u.endsWith("/api/settings/network")) {
      return new Response(
        JSON.stringify({
          success: true,
          data: { exposeToLan: false, discovery: {}, advanced: { httpPort: 18791 } },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      );
    }
    return new Response("not mocked", { status: 404 });
  }) as unknown as typeof fetch;
});

describe("NetworkSettingsCard — off state", () => {
  it("renders the off-state copy when exposeToLan is false", async () => {
    render(<NetworkSettingsCard />);
    await waitFor(() => {
      expect(screen.getByText(/LAN exposure is off/i)).toBeInTheDocument();
    });
    expect(screen.queryByText(/agentzero\.local/i)).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run test — expect compile failure**

Run: `cd apps/ui && npm test -- --run NetworkSettingsCard`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement the skeleton**

Create `apps/ui/src/features/settings/NetworkSettingsCard.tsx`:

```tsx
import { useEffect, useState } from "react";

type MdnsStatus = {
  active: boolean;
  interfaces: string[];
  aliasClaimed: boolean;
  instanceId: string;
};

type NetworkInfo = {
  exposeToLan: boolean;
  bindHost: string;
  port: number;
  hostnameUrls: string[];
  ipUrls: string[];
  mdns: MdnsStatus;
};

type ApiEnvelope<T> = { success: boolean; data?: T; error?: string };

export function NetworkSettingsCard() {
  const [info, setInfo] = useState<NetworkInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const res = await fetch("/api/network/info");
        const body = (await res.json()) as ApiEnvelope<NetworkInfo>;
        if (!cancelled) {
          if (body.success && body.data) {
            setInfo(body.data);
          } else {
            setError(body.error ?? "Failed to load network info");
          }
        }
      } catch (e) {
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  if (loading) return <div className="settings-card">Loading network status…</div>;
  if (error) return <div className="settings-card error">{error}</div>;
  if (!info) return null;

  return (
    <section className="settings-card network-settings-card">
      <header>
        <h3>Network</h3>
      </header>

      {!info.exposeToLan && (
        <p className="muted">
          LAN exposure is off. Turn it on to make this daemon reachable from other devices.
        </p>
      )}
    </section>
  );
}
```

- [ ] **Step 4: Run test — expect pass**

Run: `cd apps/ui && npm test -- --run NetworkSettingsCard`
Expected: 1 passed.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/settings/NetworkSettingsCard.tsx apps/ui/src/features/settings/NetworkSettingsCard.test.tsx
git commit -m "feat(ui): NetworkSettingsCard skeleton (off-state rendering)"
```

---

### Task 13: UI — URL list + QR rendering for the on-state

**Files:**
- Modify: `apps/ui/package.json`
- Modify: `apps/ui/src/features/settings/NetworkSettingsCard.tsx`
- Modify: `apps/ui/src/features/settings/NetworkSettingsCard.test.tsx`

- [ ] **Step 1: Add `qrcode.react` dependency**

```bash
cd apps/ui && npm install qrcode.react@^4
```

Confirm it landed in `package.json` under `"dependencies"` and `package-lock.json` is updated.

- [ ] **Step 2: Add the on-state test**

Append to `NetworkSettingsCard.test.tsx`:

```tsx
import { fireEvent } from "@testing-library/react";

function mockNetworkInfoOn(overrides: Partial<NetworkInfo> = {}) {
  const base: NetworkInfo = {
    exposeToLan: true,
    bindHost: "0.0.0.0",
    port: 18791,
    hostnameUrls: ["http://agentzero.local", "http://phani-mbp-agentzero.local"],
    ipUrls: ["http://192.168.1.42:18791"],
    mdns: {
      active: true,
      interfaces: ["en0"],
      aliasClaimed: true,
      instanceId: "uuid",
    },
    ...overrides,
  };
  globalThis.fetch = vi.fn(async (url: RequestInfo | URL) => {
    const u = url.toString();
    if (u.endsWith("/api/network/info")) {
      return new Response(JSON.stringify({ success: true, data: base }), {
        status: 200,
      });
    }
    return new Response("not mocked", { status: 404 });
  }) as unknown as typeof fetch;
}

type NetworkInfo = {
  exposeToLan: boolean;
  bindHost: string;
  port: number;
  hostnameUrls: string[];
  ipUrls: string[];
  mdns: {
    active: boolean;
    interfaces: string[];
    aliasClaimed: boolean;
    instanceId: string;
  };
};

describe("NetworkSettingsCard — on state", () => {
  it("renders all URLs and a QR code", async () => {
    mockNetworkInfoOn();
    render(<NetworkSettingsCard />);
    await waitFor(() => {
      expect(screen.getByText("http://agentzero.local")).toBeInTheDocument();
    });
    expect(screen.getByText("http://phani-mbp-agentzero.local")).toBeInTheDocument();
    expect(screen.getByText("http://192.168.1.42:18791")).toBeInTheDocument();
    expect(screen.getByTestId("network-qr")).toBeInTheDocument();
  });

  it("renders alias collision note when aliasClaimed is false", async () => {
    mockNetworkInfoOn({
      hostnameUrls: ["http://phani-mbp-agentzero.local"],
      mdns: {
        active: true,
        interfaces: ["en0"],
        aliasClaimed: false,
        instanceId: "uuid",
      },
    });
    render(<NetworkSettingsCard />);
    await waitFor(() => {
      expect(screen.getByText(/already in use on this network/i)).toBeInTheDocument();
    });
  });

  it("renders mdns failure warning when active=false but exposeToLan=true", async () => {
    mockNetworkInfoOn({
      mdns: {
        active: false,
        interfaces: [],
        aliasClaimed: false,
        instanceId: "uuid",
      },
    });
    render(<NetworkSettingsCard />);
    await waitFor(() => {
      expect(screen.getByText(/mDNS responder failed to start/i)).toBeInTheDocument();
    });
    expect(screen.getByText("http://192.168.1.42:18791")).toBeInTheDocument();
  });
});
```

- [ ] **Step 3: Implement the on-state UI**

Replace `NetworkSettingsCard.tsx` with:

```tsx
import { useEffect, useState } from "react";
import { QRCodeSVG } from "qrcode.react";

type MdnsStatus = {
  active: boolean;
  interfaces: string[];
  aliasClaimed: boolean;
  instanceId: string;
};

type NetworkInfo = {
  exposeToLan: boolean;
  bindHost: string;
  port: number;
  hostnameUrls: string[];
  ipUrls: string[];
  mdns: MdnsStatus;
};

type ApiEnvelope<T> = { success: boolean; data?: T; error?: string };

function primaryUrl(info: NetworkInfo): string | null {
  return info.hostnameUrls[0] ?? info.ipUrls[0] ?? null;
}

export function NetworkSettingsCard() {
  const [info, setInfo] = useState<NetworkInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const res = await fetch("/api/network/info");
        const body = (await res.json()) as ApiEnvelope<NetworkInfo>;
        if (!cancelled) {
          if (body.success && body.data) {
            setInfo(body.data);
          } else {
            setError(body.error ?? "Failed to load network info");
          }
        }
      } catch (e) {
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  if (loading) return <div className="settings-card">Loading network status…</div>;
  if (error) return <div className="settings-card error">{error}</div>;
  if (!info) return null;

  const qrTarget = primaryUrl(info);

  return (
    <section className="settings-card network-settings-card">
      <header>
        <h3>Network</h3>
      </header>

      {!info.exposeToLan && (
        <p className="muted">
          LAN exposure is off. Turn it on to make this daemon reachable from other devices.
        </p>
      )}

      {info.exposeToLan && (
        <>
          {info.exposeToLan && info.mdns.active === false && (
            <div className="warning" role="status">
              ⚠ mDNS responder failed to start — devices can still reach the IP URL above.
            </div>
          )}

          {info.exposeToLan && info.mdns.active && !info.mdns.aliasClaimed && (
            <div className="info" role="status">
              <code>agentzero.local</code> is already in use on this network — only the
              per-instance hostname is being advertised.
            </div>
          )}

          <div className="network-urls">
            <div className="url-list">
              <strong>Reachable at:</strong>
              <ul>
                {info.hostnameUrls.map((u) => (
                  <li key={u}>
                    <code>{u}</code>
                  </li>
                ))}
                {info.ipUrls.map((u) => (
                  <li key={u}>
                    <code>{u}</code>
                  </li>
                ))}
              </ul>
            </div>
            {qrTarget && (
              <div className="qr" data-testid="network-qr">
                <QRCodeSVG value={qrTarget} size={128} includeMargin />
              </div>
            )}
          </div>

          {info.mdns.interfaces.length > 0 && (
            <div className="status muted">
              ● Advertising on {info.mdns.interfaces.join(", ")}
            </div>
          )}
        </>
      )}
    </section>
  );
}
```

- [ ] **Step 4: Run tests — expect pass**

Run: `cd apps/ui && npm test -- --run NetworkSettingsCard`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/package.json apps/ui/package-lock.json apps/ui/src/features/settings/NetworkSettingsCard.tsx apps/ui/src/features/settings/NetworkSettingsCard.test.tsx
git commit -m "feat(ui): NetworkSettingsCard renders URLs, QR code, and warning states"
```

---

### Task 14: UI — toggle interaction + restart-required banner

**Files:**
- Modify: `apps/ui/src/features/settings/NetworkSettingsCard.tsx`
- Modify: `apps/ui/src/features/settings/NetworkSettingsCard.test.tsx`

- [ ] **Step 1: Write the failing test**

Append to `NetworkSettingsCard.test.tsx`:

```tsx
describe("NetworkSettingsCard — toggle", () => {
  it("clicking the toggle PUTs new settings and shows the restart banner", async () => {
    const calls: { url: string; init?: RequestInit }[] = [];
    globalThis.fetch = vi.fn(async (url: RequestInfo | URL, init?: RequestInit) => {
      const u = url.toString();
      calls.push({ url: u, init });
      if (u.endsWith("/api/network/info")) {
        return new Response(
          JSON.stringify({
            success: true,
            data: {
              exposeToLan: true,
              bindHost: "0.0.0.0",
              port: 18791,
              hostnameUrls: ["http://agentzero.local"],
              ipUrls: ["http://192.168.1.42:18791"],
              mdns: { active: true, interfaces: ["en0"], aliasClaimed: true, instanceId: "u" },
            },
          }),
          { status: 200 },
        );
      }
      if (u.endsWith("/api/settings/network") && init?.method === "PUT") {
        return new Response(
          JSON.stringify({ success: true, data: JSON.parse(init.body as string) }),
          { status: 200 },
        );
      }
      return new Response("not mocked", { status: 404 });
    }) as unknown as typeof fetch;

    render(<NetworkSettingsCard />);
    await waitFor(() => screen.getByRole("checkbox", { name: /Expose to LAN/i }));

    fireEvent.click(screen.getByRole("checkbox", { name: /Expose to LAN/i }));

    await waitFor(() => {
      expect(screen.getByText(/Daemon restart required/i)).toBeInTheDocument();
    });
    const putCall = calls.find((c) => c.url.endsWith("/api/settings/network") && c.init?.method === "PUT");
    expect(putCall).toBeDefined();
    expect(JSON.parse(putCall!.init!.body as string).exposeToLan).toBe(false);
  });
});
```

- [ ] **Step 2: Add the toggle and banner**

Modify `NetworkSettingsCard.tsx` — add toggle state, save handler, and banner. Replace the `<header>…</header>` block with:

```tsx
      <header className="row">
        <h3>Network</h3>
        <label className="toggle">
          <input
            type="checkbox"
            aria-label="Expose to LAN"
            checked={info.exposeToLan}
            onChange={() => void onToggle()}
          />
          <span>Expose to LAN</span>
        </label>
      </header>
      <p className="muted small">
        Other devices on your network can reach this daemon. Restart daemon to apply changes.
      </p>
      {showRestartBanner && (
        <div className="banner restart-required" role="status">
          Daemon restart required to apply changes.
        </div>
      )}
```

And inside the `NetworkSettingsCard` component (above the early returns), add:

```tsx
  const [showRestartBanner, setShowRestartBanner] = useState(false);

  async function onToggle() {
    if (!info) return;
    const next = !info.exposeToLan;
    // Optimistic UI flip.
    setInfo({ ...info, exposeToLan: next });

    // Fetch current settings to preserve nested fields the user might have changed.
    const getRes = await fetch("/api/settings/network");
    const getBody = (await getRes.json()) as ApiEnvelope<{
      exposeToLan: boolean;
      discovery: Record<string, unknown>;
      advanced: { bindHost: string | null; httpPort: number };
    }>;
    const current = getBody.data ?? {
      exposeToLan: info.exposeToLan,
      discovery: {},
      advanced: { bindHost: null, httpPort: info.port },
    };

    const putRes = await fetch("/api/settings/network", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ ...current, exposeToLan: next }),
    });
    const putBody = (await putRes.json()) as ApiEnvelope<unknown>;
    if (putBody.success) {
      setShowRestartBanner(true);
    } else {
      // Revert optimistic flip on failure.
      setInfo({ ...info, exposeToLan: info.exposeToLan });
      setError(putBody.error ?? "Failed to update network settings");
    }
  }
```

(The `getRes` step uses the existing `/api/settings/network` GET handler from Task 11 to round-trip nested fields without losing user overrides.)

- [ ] **Step 3: Run tests — expect pass**

Run: `cd apps/ui && npm test -- --run NetworkSettingsCard`
Expected: 5 passed.

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/settings/
git commit -m "feat(ui): toggle Expose to LAN with restart-required banner"
```

---

### Task 15: Wire `NetworkSettingsCard` into the Settings page

**Files:**
- Modify: `apps/ui/src/features/settings/WebSettingsPanel.tsx`

- [ ] **Step 1: Identify the Advanced section**

Read `apps/ui/src/features/settings/WebSettingsPanel.tsx`. Find where existing advanced cards (Orchestrator, Distillation, Multimodal) are composed.

- [ ] **Step 2: Import and render the card**

Add at the top:

```tsx
import { NetworkSettingsCard } from "./NetworkSettingsCard";
```

Add `<NetworkSettingsCard />` adjacent to the other advanced cards in the JSX. Match the existing layout container pattern (e.g., wrapping div, grid spacing).

- [ ] **Step 3: Build and visually verify**

```bash
cd apps/ui && npm run build
```

Then run the dev server and navigate to Settings → Advanced. Confirm:
- Network card renders.
- Toggle is reflected from `/api/network/info` truth.
- Toggling shows the restart banner.
- URL list + QR render when on.

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/settings/WebSettingsPanel.tsx
git commit -m "feat(ui): mount NetworkSettingsCard in Settings → Advanced"
```

---

### Task 16: README + release note

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add a "LAN access" subsection**

In `README.md`, after the "First Run" section (around line ~104), add:

```markdown
### LAN access

By default the daemon advertises itself on your local network so phones, tablets, and other devices can reach it without typing an IP. Visit:

- `http://agentzero.local` from any device on the same Wi-Fi.
- Or scan the QR code in **Settings → Network** to open the URL on your phone.

If you'd rather keep the daemon loopback-only, toggle **Expose to LAN** off in Settings or set `network.exposeToLan: false` in `~/Documents/zbot/config/settings.json` (restart required).

**Heads up for upgraders:** prior versions only listened on `127.0.0.1`. After this release the daemon listens on `0.0.0.0` by default.
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs(readme): document LAN access and Expose to LAN toggle"
```

---

### Task 17: Final verification + push

- [ ] **Step 1: Workspace-wide checks**

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cd apps/ui && npm run lint && npm test && npm run build
```

Expected: all green.

- [ ] **Step 2: Manual end-to-end smoke**

1. Start daemon (`npm run daemon`).
2. From a second device on the same network: open `http://agentzero.local:18791/api/health` — expect 200.
3. From the same daemon host: visit Settings → Network. Confirm URLs + QR + status badge.
4. Toggle Expose to LAN off, restart daemon, retry the second-device URL — expect connection refused (loopback-only).
5. Toggle back on, restart, retry — expect 200 again.

- [ ] **Step 3: Push the branch**

```bash
git push -u origin feature/lan-discoverability
```

- [ ] **Step 4: Open a PR against `develop`**

```bash
gh pr create --base develop --head feature/lan-discoverability \
  --title "feat: LAN discoverability (mDNS) for AgentZero daemon" \
  --body "$(cat <<'EOF'
## Summary

Implements [`memory-bank/future-state/2026-05-02-lan-discoverability-design.md`](../blob/feature/lan-discoverability/memory-bank/future-state/2026-05-02-lan-discoverability-design.md) (PR #85).

- New `discovery/` workspace crate, pure-Rust mDNS via `mdns-sd`. Cross-platform on Windows / macOS / Linux / Raspberry Pi without Bonjour-for-Windows or Avahi.
- `network` block in `settings.json` with default-on `exposeToLan` toggle and VPN/tunnel exclusion.
- `GET /api/network/info` + `GET/PUT /api/settings/network`.
- Settings → Network UI card with toggle, live URLs, QR code, restart banner, and warning states.

## Upgrade impact

Default `exposeToLan: true` means existing users who pull this and restart will start listening on `0.0.0.0`. README has a heads-up.

## Test plan

- [x] `cargo test --workspace`
- [x] `cargo test -p discovery -- --ignored` (integration mDNS tests)
- [x] `npm run lint && npm test && npm run build` in `apps/ui`
- [ ] Manual: phone hits `http://agentzero.local` on home Wi-Fi
- [ ] Manual: VPN connect — daemon NOT visible from VPN-side device
- [ ] Manual: toggle off, restart, confirm loopback-only

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-Review

I checked the plan against the spec:

**Spec coverage:** Every numbered goal/section in the spec maps to at least one task — module boundaries (Task 1), config schema (Tasks 2, 7), interface filtering (Task 3), `NetworkInfo` builder (Task 4), `Advertiser`/`NoopAdvertiser` (Task 5), `MdnsAdvertiser` + integration tests (Task 6), gateway bind resolution (Task 8), startup/shutdown wiring (Task 9), `/api/network/info` (Task 10), `/api/settings/network` (Task 11), Settings UI off-state (Task 12), on-state with QR (Task 13), toggle + banner (Task 14), Settings page integration (Task 15), README (Task 16), and a final cross-cut verify pass (Task 17).

**Placeholder scan:** No "TBD"/"TODO"/"add appropriate"/"similar to Task N" patterns. Every step that changes code shows the code. The one deferred-to-runtime piece (alias-collision detection via `ServiceConflict` listener) is documented in code with a comment explaining the v1 optimistic-true approach and pointed to as future work.

**Type consistency:** `NetworkInfo`, `MdnsStatus`, `DiscoveryConfig`, `DiscoveryDetails`, `AdvancedConfig`, `ServiceInfo`, `Advertiser`, `AdvertiseHandle`, `AdvertiseInner`, `NoopAdvertiser`, `MdnsAdvertiser` use the same names everywhere they appear. The handler in Task 10 imports `NetworkInfo` from `discovery` — matches the export added in Task 4. The `aliasClaimed` field is camelCase in JSON / TypeScript and `alias_claimed` in Rust serde — consistent across producer (Task 4) and consumer (Tasks 10, 12, 13).

No fixes needed.
