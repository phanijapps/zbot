# LAN Discoverability for AgentZero

**Status:** Design (awaiting review)
**Date:** 2026-05-02
**Owner:** phanijapps
**Target branch:** `develop`

## Problem

The AgentZero daemon binds to `127.0.0.1:18791` and is invisible to other devices on the same network. To use AgentZero from a phone, tablet, or second laptop today, a user must:

1. Look up the host machine's LAN IP.
2. Type `http://<ip>:18791` in a browser.
3. Repeat the lookup whenever the IP changes (Wi-Fi switch, DHCP lease change).

The goal is to make the daemon **discoverable** so that:

- **Browsers** on other devices can reach the UI by typing a friendly hostname (`http://agentzero.local`).
- **Native or programmatic clients** can browse for available daemons over the LAN without configuration.

## Goals

1. Browser-friendly hostname resolution from any device on the same LAN.
2. Programmatic discovery via a standard mDNS service type.
3. Cross-platform: Windows, macOS, Linux desktop, Raspberry Pi.
4. Modular: discovery is a swappable component, not tangled into the gateway.
5. Default-on, but easily toggled off via `settings.json` and the Settings UI.
6. No new auth model in v1 — built so auth can be added later without redesign.

## Non-goals

- Authentication, authorization, or transport encryption (deliberately out of scope; user accepts LAN trust posture).
- IPv6 advertisement.
- Cross-subnet / WAN discovery.
- Hot-reload of the toggle (restart-required is acceptable).
- Bonjour-for-Windows installation prompts (use a pure-Rust responder so this is unnecessary).

## High-level architecture

A new workspace crate, `discovery/`, owns mDNS advertisement and LAN URL enumeration. The gateway depends on it but knows nothing about mDNS internals. The crate exposes a small surface so it can be swapped (for tests, for a future transport, or for a no-op when disabled).

```text
agentzero/
├── discovery/                      # NEW crate (peer of gateway)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                  # public API: Advertiser trait, ServiceInfo, AdvertiseHandle
│       ├── config.rs               # DiscoveryConfig (serde, defaults)
│       ├── mdns.rs                 # MdnsAdvertiser (mdns-sd backed)
│       ├── noop.rs                 # NoopAdvertiser (used when disabled)
│       ├── network.rs              # interface enumeration, URL list, NetworkInfo
│       └── tests/                  # integration tests (gated by --ignored)
└── gateway/
    └── src/
        ├── network_info.rs         # NEW: GET /api/network/info handler
        ├── server.rs               # MODIFIED: read config, hold handle, swap bind addr
        └── config.rs               # MODIFIED: add network: NetworkSettings
```

### Public API of `discovery`

```rust
pub trait Advertiser: Send + Sync {
    fn advertise(&self, info: ServiceInfo) -> Result<AdvertiseHandle>;
}

pub struct ServiceInfo {
    pub instance_name: String,            // e.g. "phani-mbp"
    pub service_type: String,             // "_agentzero._tcp.local."
    pub hostname_alias: String,           // "agentzero.local"
    pub port: u16,                        // ACTUAL bound port, not config value
    pub txt: BTreeMap<String, String>,
}

// RAII: dropping the handle sends mDNS goodbye and stops the responder.
pub struct AdvertiseHandle { /* … */ }
impl Drop for AdvertiseHandle { /* withdraw, blocking up to 100ms */ }

pub struct NetworkInfo {
    pub expose_to_lan: bool,
    pub bind_host: String,
    pub port: u16,
    pub hostname_urls: Vec<String>,       // empty when expose_to_lan=false
    pub ip_urls: Vec<String>,
    pub mdns: MdnsStatus,
}

pub struct MdnsStatus {
    pub active: bool,
    pub interfaces: Vec<String>,
    pub alias_claimed: bool,
    pub instance_id: String,              // UUID
}

pub fn collect_network_info(cfg: &DiscoveryConfig, port: u16, alias_claimed: bool) -> NetworkInfo;
```

### Why this shape

- **Trait + handle.** Gateway holds an `Arc<dyn Advertiser>`. Tests inject a `RecordingAdvertiser`. Disabled state uses `NoopAdvertiser` and the rest of the gateway code is identical.
- **RAII handle.** `Drop` sends the mDNS goodbye packet. No way to forget to withdraw.
- **`network.rs` separate from `mdns.rs`.** Interface enumeration / URL list building works whether or not mDNS is active. The Settings panel always shows IP+QR even when mDNS fails.
- **Sync trait.** The advertiser interface is sync at the boundary; the mDNS responder runs on its own task internally. Keeps the trait dyn-compatible without `async-trait`.
- **Layer order preserved.** `gateway` depends on `discovery`, not vice versa. Matches the existing `framework → runtime → services → gateway → apps` stack.

### Crate dependencies (all pure Rust, no C bindings)

| Crate | Purpose |
|---|---|
| `mdns-sd` | mDNS responder (advertise + browse). Coexists with system responders via `SO_REUSEPORT` on UDP 5353. |
| `if-addrs` | Interface enumeration for URL list and tunnel exclusion. |
| `gethostname` | Default instance name derivation. |
| `uuid` | Instance ID generation (already in workspace). |
| `qrcode.react` (UI side) | QR code rendering in the Network panel. |

## Configuration schema

A new `network` block in `settings.json` (camelCase, matching `AppSettings` convention):

```jsonc
{
  // … existing tools, logs, execution …
  "network": {
    // Primary toggle. true → bind 0.0.0.0 + start mDNS responder.
    // false → bind 127.0.0.1, no mDNS (today's behavior). Default: true.
    "exposeToLan": true,

    // mDNS advertisement details. All fields optional; defaults shown.
    "discovery": {
      // Service instance label. Default: derived from system hostname (e.g. "phani-mbp").
      "instanceName": null,

      // mDNS service type. Default: "_agentzero._tcp.local."
      "serviceType": null,

      // Hostname alias. Default: "agentzero.local". On collision the daemon
      // keeps only "<instanceName>-agentzero.local".
      "hostnameAlias": null,

      // Extra TXT record entries (merged with built-in version/instance/path).
      "txtRecords": {},

      // Glob patterns (matched on interface name) to exclude from advertising.
      // Default keeps mDNS off VPN tunnels, which is the LAN-correct behavior.
      "excludeInterfaces": ["utun*", "tun*", "ppp*", "tap*"],

      // Stable instance UUID. Auto-generated and persisted on first start.
      "instanceId": null
    },

    // Power-user overrides. Leave null in the common case.
    "advanced": {
      // Bind host. null = derived from exposeToLan.
      "bindHost": null,

      // HTTP port. Default: 18791.
      "httpPort": 18791
    }
  }
}
```

### Bind resolution

```rust
fn resolve_bind_host(net: &NetworkSettings) -> IpAddr {
    match (&net.advanced.bind_host, net.expose_to_lan) {
        (Some(explicit), _) => explicit.parse().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        (None, true)  => IpAddr::V4(Ipv4Addr::UNSPECIFIED),  // 0.0.0.0
        (None, false) => IpAddr::V4(Ipv4Addr::LOCALHOST),    // 127.0.0.1
    }
}
```

### Apply semantics — restart required

Toggling `exposeToLan` writes to `settings.json` and shows a "Restart daemon to apply" notice in the UI. No hot-reload because:

1. Rebinding a TCP listener in-flight is fiddly (existing connections, port reuse races).
2. Other settings.json fields in this codebase already require restart (provider config, orchestrator config).
3. This is a one-time setup decision, not a frequent toggle.
4. Keeps the `discovery` crate's API minimal — no mutable advertise/withdraw state to coordinate.

### Upgrade path

Default `exposeToLan: true` applies to existing users who upgrade and restart. Their daemon will start listening on `0.0.0.0`. This is the chosen behavior (default-ON was the explicit pick during design). The release notes for the change must call this out so it isn't a surprise.

## mDNS advertisement

When the gateway starts with `exposeToLan: true`, after the HTTP listener has bound, it constructs `ServiceInfo` with the **actual bound port** and hands it to the advertiser. The advertiser publishes:

```text
PTR  _agentzero._tcp.local.                      → <instance>._agentzero._tcp.local.
SRV  <instance>._agentzero._tcp.local.           → <instance>-agentzero.local.:<port>
TXT  <instance>._agentzero._tcp.local.           → version=… instance=… name=… path=/ ws=1
A    <instance>-agentzero.local.                 → 192.168.x.y   (one per non-excluded interface)
A    agentzero.local.                            → 192.168.x.y   (alias, may collide)
```

Two A records are deliberate. `<instance>-agentzero.local` is per-machine and always unique, so a phone with multiple AgentZero machines on the LAN can address a specific one. `agentzero.local` is the friendly alias for the common single-machine case.

### Hostname collision

mDNS does conflict probing before publishing. If another host on the LAN already claims `agentzero.local`, the advertiser drops only the alias A record and keeps the per-instance A record. The Network panel displays:

> Reachable at: `http://phani-mbp-agentzero.local` *(another AgentZero on this network is using `agentzero.local`)*

### Built-in TXT keys

| Key | Value | Purpose |
|---|---|---|
| `version` | crate `CARGO_PKG_VERSION` | Client compatibility check. |
| `instance` | UUID, persisted in settings.json | Stable identity across restarts; lets clients recognize "the same daemon" after IP change. |
| `name` | friendly name (defaults to instance name) | Shown in client UIs. |
| `path` | `/` | API base path (forward-compat for sub-mounts). |
| `ws` | `1` | WebSocket available on `/ws` of same port. |

User-provided keys from `network.discovery.txtRecords` are merged in **after** the built-ins, so they cannot shadow the reserved keys above.

### Instance UUID lifecycle

- Generated once and stored at `network.discovery.instanceId` in settings.json.
- If absent, the daemon generates a UUIDv4 on first start and writes it back.
- Stays constant across restarts, IP changes, and hostname changes.
- Settings UI exposes a "Regenerate" button (resets the durable identity — clients lose recognition).

### Port

The SRV record always reflects the actual bound port. If 18791 was unavailable and the daemon fell back to an ephemeral port (future enhancement), the SRV record reflects reality.

### TTL

120 seconds on all records. Short enough that abrupt termination doesn't leave stale entries cluttering clients for long; long enough to avoid chatty re-announce traffic.

### Withdraw on shutdown

The `AdvertiseHandle` lives in `AppState`. Graceful shutdown drops it → `Drop` impl sends mDNS goodbye packets (TTL=0) for all published records, blocking up to 100ms for them to flush. Abrupt termination (SIGKILL, panic) skips the goodbye; clients fall back to TTL expiry.

## Cross-platform behavior

| Platform | Pre-installed mDNS | Pure-Rust path | Notes |
|---|---|---|---|
| macOS | Apple mDNSResponder | Coexists via `SO_REUSEPORT` on UDP 5353 | First launch may show macOS firewall prompt. |
| Linux desktop | Usually Avahi | Coexists with Avahi if present; works without it | No system service required. |
| Raspberry Pi (Pi OS / Debian) | Avahi typically | Same as Linux | ARM cross-compile clean — no C deps. |
| Windows 10/11 | No responder; partial resolver in DNSCache (1809+) | Pure-Rust path provides the responder | Defender Firewall prompts on first bind to 0.0.0.0. |

## Settings UI: the Network panel

A new "Network" card under Settings → Advanced (alongside Orchestrator, Distillation, Multimodal — same layout convention).

```text
┌─ Network ───────────────────────────────────────────────────┐
│                                                              │
│  Expose to LAN                                       [ ON ]  │
│  Other devices on your network can reach this daemon.        │
│  Restart daemon to apply changes.                            │
│                                                              │
│  ─────────────────────────────────────────────────────────   │
│                                                              │
│  Reachable at:                          ┌──────────────┐    │
│    http://agentzero.local                │              │    │
│    http://phani-mbp-agentzero.local      │   QR CODE    │    │
│    http://192.168.1.42:18791             │              │    │
│                                          └──────────────┘    │
│                                                              │
│  Status: ● Advertising on Wi-Fi (eth0, en0)                 │
│                                                              │
│  [ Copy URL ]   [ Show advanced ]                            │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### Data source

New endpoint `GET /api/network/info` returns:

```ts
type NetworkInfo = {
  exposeToLan: boolean;
  bindHost: string;
  port: number;
  hostnameUrls: string[];     // [] when exposeToLan=false
  ipUrls: string[];
  mdns: {
    active: boolean;
    interfaces: string[];
    aliasClaimed: boolean;
    instanceId: string;
  };
};
```

Computed on every request — no caching. Thin wrapper over `discovery::collect_network_info()` plus the gateway's bound port and config state.

### QR code

Encodes the primary URL — first `hostnameUrls` entry, or first `ipUrls` if mDNS is off. Generated client-side with `qrcode.react`. No server-side QR generation; keeps the discovery crate UI-free.

### Toggle UX

1. Click toggle → optimistic UI flip → POST to existing settings endpoint to write `network.exposeToLan`.
2. Success shows persistent banner: *"Daemon restart required to apply. [Restart now] [Later]"*.
3. "Restart now" hits the existing daemon-restart path (verify during plan).

### "Show advanced" reveal

Collapsible reveals editable fields mapping 1:1 to `network.advanced.*` and `network.discovery.*`:

- Bind host override
- HTTP port override
- Service type override
- Hostname alias override
- Instance UUID (read-only with "Regenerate")

All optional. Same restart-required semantics.

### State variants

- **Off:** URL section says *"LAN exposure is off. Turn it on to make this daemon reachable from other devices."* No URLs, no QR.
- **mDNS failed to start:** Status row turns yellow: *"⚠ mDNS responder failed to start — devices can still reach the IP URL above."* IP URLs and QR still render.
- **Alias collision:** Status row neutral note: *"`agentzero.local` is already in use on this network — only `phani-mbp-agentzero.local` is being advertised."*

### File locations (UI)

```
apps/ui/src/features/settings/
├── NetworkSettingsCard.tsx        # NEW
└── NetworkSettingsCard.test.tsx   # NEW
```

## Runtime behavior

### Startup sequence (gateway, when `exposeToLan: true`)

1. Read `settings.json` → `NetworkSettings`.
2. Resolve bind host → `0.0.0.0`.
3. `TcpListener::bind(host:port)` → capture `actual_port`.
4. If bind fails, log error and exit (today's behavior; no fallback added yet).
5. Construct `ServiceInfo { instance, type, port: actual_port, txt }`.
6. `advertiser.advertise(info)` → `AdvertiseHandle` stashed in `AppState`. `mdns-sd` spawns its own task; we don't await readiness. First announce goes out within ~250ms after probe.
7. Begin serving HTTP/WS.

If step 6 fails (e.g., another process owns multicast 5353 exclusively), log a warning and continue serving — the daemon is still reachable via IP, just not via the friendly hostname. The Network panel reflects this.

### Shutdown sequence

1. Stop accepting new HTTP connections.
2. Drop `AdvertiseHandle` → `Drop` impl sends `mdns-sd` Goodbye (TTL=0), blocks up to 100ms.
3. `ServiceDaemon::shutdown()` joins the mdns-sd background task.
4. Drop other gateway state, exit.

### Network changes

Trust `mdns-sd`'s internal interface poll. We do not add an external watcher — adding one would just race with the library's own poll.

- **Wi-Fi swap:** old interface goes down → mdns-sd notices on next poll (~5s) → re-announces with the new IP. Instance UUID stays constant, so clients still recognize the daemon after re-pairing.
- **Suspend/resume:** OS-dependent. On resume, mdns-sd re-joins multicast groups. Small silent window (~5–10s) is acceptable.
- **VPN connect/disconnect:** new `tun`/`utun` interface appears. Excluded by default (`network.discovery.excludeInterfaces`), so the daemon is **not** advertised on the VPN. This is the LAN-correct behavior.

### Failure modes

| What fails | What happens | What the user sees |
|---|---|---|
| `TcpListener::bind` fails (port taken) | Daemon exits with error log. | CLI/dev console error; today's behavior. |
| `mdns-sd` fails to initialize | Warning logged; daemon keeps serving. | Network panel: yellow status, `mdns.active: false`, IP URLs still shown. |
| Hostname alias `agentzero.local` collides | Per-instance hostname still works. | Network panel neutral note about collision. |
| Multicast 5353 blocked by firewall | mDNS sends, nothing receives. | Panel says "active" (we sent), but devices can't resolve. Documented in troubleshooting copy. |
| All interfaces excluded | No A records advertised. | Panel: yellow status "No eligible network interfaces". |
| Process killed (-9) | No goodbye sent. | Clients see entries for up to 120s TTL. |
| User flips toggle mid-session | Setting written; banner shown. | "Restart daemon to apply" — no immediate effect. |

### Logging (tracing)

```text
INFO  discovery: advertising _agentzero._tcp.local on en0=192.168.1.42 (port=18791)
INFO  discovery: advertising _agentzero._tcp.local on eth0=10.0.0.5 (port=18791)
WARN  discovery: hostname alias agentzero.local collided; only phani-mbp-agentzero.local is advertised
WARN  discovery: failed to start mDNS responder: <err>; daemon reachable via IP only
DEBUG discovery: re-announce due to interface change (utun0 ignored by exclude pattern)
INFO  discovery: withdrawn (goodbye sent for 5 records)
```

### Deliberately not added

- Watchdog that restarts the mDNS responder. If it dies, restarting it doesn't help; daemon keeps serving over IP.
- Hot-reload of `exposeToLan` (see "Apply semantics" above).
- An external interface-change watcher.

## Testing

### Unit tests in `discovery/` (`cargo test -p discovery`)

| Test | What it pins down |
|---|---|
| `config_defaults` | `DiscoveryConfig::default()` matches the schema. |
| `config_deserializes_partial_settings` | Empty `network` block yields all defaults. |
| `instance_name_derived_from_hostname` | `instance_name: None` → `gethostname()` with `.local` stripped. |
| `txt_records_merge_builtin_first` | User keys cannot shadow reserved built-ins. |
| `interface_filter_excludes_tunnels` | After exclusion, `utun0`, `tun1`, `ppp0`, `tap0` are gone. |
| `interface_filter_keeps_real_lan` | `en0`, `eth0`, `wlan0` survive. |
| `network_info_when_disabled` | Empty hostnameUrls/ipUrls, `mdns.active: false`. |
| `network_info_when_enabled` | URLs include hostname alias + per-instance + each non-loopback IPv4. |
| `service_info_uses_actual_bound_port` | Construction takes `actual_port`, not config. |

Interface enumeration is wrapped in a small trait with a real impl + `MockEnumerator` for tests. No network access required.

### Integration tests (`discovery/tests/`, gated by `#[ignore]`)

Run via `cargo test -p discovery -- --ignored`.

| Test | What it pins down |
|---|---|
| `advertise_then_browse_finds_self` | `mdns-sd` browse finds the advertised instance with correct port + TXT. |
| `drop_handle_sends_goodbye` | After drop, browse sees TTL=0 record or no current record. |
| `two_instances_collide_on_alias` | Second instance logs collision warning and keeps only its per-instance hostname. |
| `excluded_interfaces_not_advertised` | With `excludeInterfaces: ["lo*"]`, no loopback addresses appear in records. |

CI runs these on Linux only (multicast on macOS GitHub runners is unreliable; Windows runners block it). Local dev runs them on whichever platform the developer is on.

### Gateway tests (`cargo test -p gateway`)

Discovery injected via `Arc<dyn Advertiser>` using `RecordingAdvertiser`:

| Test | What it pins down |
|---|---|
| `bind_loopback_when_expose_to_lan_false` | Listener bound to `127.0.0.1`. |
| `bind_unspecified_when_expose_to_lan_true` | Listener bound to `0.0.0.0`. |
| `advanced_bind_host_overrides_toggle` | `network.advanced.bindHost` wins over `exposeToLan`. |
| `advertiser_called_with_actual_port` | When listener binds on ephemeral port, advertiser receives that port. |
| `advertiser_not_called_when_disabled` | `exposeToLan: false` → `NoopAdvertiser`, zero calls recorded. |
| `network_info_endpoint_off_state` | `GET /api/network/info` returns empty URL arrays. |
| `network_info_endpoint_on_state` | Returns hostname URLs, IP URLs, mdns status. |
| `network_info_endpoint_alias_collision` | `aliasClaimed: false` propagated correctly. |

### UI tests (`cd apps/ui && npm test`)

| Test | What it pins down |
|---|---|
| Renders off state | Toggle off → URL section says "LAN exposure is off". |
| Renders on state with URLs | All URLs rendered, QR present. |
| Renders alias collision note | `aliasClaimed: false` → neutral message visible. |
| Renders mdns failure warning | `mdns.active: false, exposeToLan: true` → yellow warning, IP URLs still rendered. |
| Toggle click writes settings and shows restart banner | POST fires, banner appears. |
| Copy URL copies primary hostname URL | Clipboard receives the first hostname URL. |

### Manual platform pass (pre-merge checklist)

Done from a second device on the same network (phone or laptop). For each target:

- [ ] **macOS host:** `http://agentzero.local` loads in mobile Safari. Bonjour Browser shows `_agentzero._tcp` instance. Toggle off → resolves fail within ~5s.
- [ ] **Linux host (Avahi running):** Same, plus `avahi-browse -art _agentzero._tcp` shows instance.
- [ ] **Linux host (Avahi stopped):** Same — pure-Rust responder is sufficient.
- [ ] **Raspberry Pi (Pi OS):** Same as Linux. Verify ARM build works, no panics on boot.
- [ ] **Windows 10/11 host:** Phone hits `http://agentzero.local`. Defender prompt accepted on first run. Confirm Windows resolver finds `.local` (1809+).
- [ ] **VPN test:** Connect VPN. Daemon logs show no advertisement on `utun*`. Bonjour browser on a corp-network device does NOT see the instance (intended privacy behavior).

## Open questions for plan stage

These are fine to defer to the writing-plans phase but documented here so they aren't lost:

1. Does the daemon already expose a restart endpoint, or must the user run `npm run daemon` themselves? Affects the "Restart now" button copy.
2. Where exactly does `AppSettings` get loaded vs validated? Pinpoint the place to insert the new `network` block read.
3. Tauri shell — does it need any entitlement changes for the new bind behavior? (Likely not since the daemon is a separate process, but worth confirming.)
4. Is there an existing pattern for "settings change requires restart" banners in the UI? If so, reuse it.

## Out-of-scope future work

- IPv6 advertisement (config flag `network.discovery.ipv6: bool`).
- Optional `_http._tcp.local` shadow advertisement for visibility in generic Bonjour browsers.
- Authentication: per-device approval flow (Plex/Syncthing-style), pre-shared token, or mTLS. The current design's RAII handle and trait-based advertiser do not block any of these.
- Auto-detection of "trusted networks" (SSID-based) so LAN exposure can be granular.
- Graceful port fallback (try 18791, fall back to ephemeral if taken).
- Hot-reload of the `exposeToLan` toggle.

## References

- `gateway/src/lib.rs:63-66` — current default ports.
- `gateway/src/config.rs:55` — current bind host (`127.0.0.1`).
- `gateway/gateway-services/src/settings.rs` — `AppSettings` struct conventions.
- [`mdns-sd` crate documentation](https://docs.rs/mdns-sd) — pure-Rust mDNS responder.
- [RFC 6762 — Multicast DNS](https://datatracker.ietf.org/doc/html/rfc6762)
- [RFC 6763 — DNS-Based Service Discovery](https://datatracker.ietf.org/doc/html/rfc6763)
