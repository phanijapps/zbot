# discovery

LAN service advertisement (mDNS) for the AgentZero daemon. Allows z-Bot instances to be auto-discovered on the local network via mDNS-SD (`_zbot._tcp.local`).

## Key Types

```rust
pub use advertiser::{Advertiser, AdvertiseHandle, AdvertiseInner,
    NoopAdvertiser, DiscoveryError, ServiceInfo};
pub use config::{DiscoveryConfig, DiscoveryDetails, AdvancedConfig};
pub use interfaces::{filter_interfaces, ipv4_only, Interface, InterfaceEnumerator, RealEnumerator};
pub use mdns::MdnsAdvertiser;
pub use network_info::{collect_network_info, sanitize_for_hostname, MdnsStatus, NetworkInfo};
```

## Modules

| Module | Purpose |
|--------|---------|
| `advertiser` | `Advertiser` trait + `NoopAdvertiser` for tests; `AdvertiseHandle` lifecycle |
| `mdns` | `MdnsAdvertiser` — real mDNS-SD implementation via `mdns-sd` crate |
| `config` | `DiscoveryConfig` — enable/disable, service name, port, metadata |
| `interfaces` | `InterfaceEnumerator` trait + real impl; IPv4 filtering |
| `network_info` | `collect_network_info()` — gathers local IPs, hostname, mDNS status |

## Usage

The gateway instantiates `MdnsAdvertiser` or `NoopAdvertiser` based on settings, wraps it in `AdvertiseHandle`, and drops the handle on shutdown.

```rust
let advertiser = MdnsAdvertiser::new(config)?;
let handle = advertiser.advertise(service_info).await?;
// handle dropped on shutdown → service deregistered
```

## Intra-Repo Dependencies

None — pure library crate. Used only by `gateway`.

## External Dependencies

- `mdns-sd` — mDNS-SD protocol implementation
- `if-addrs` — enumerate network interfaces
- `gethostname` — get system hostname
