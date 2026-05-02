//! # Discovery
//!
//! LAN service advertisement (mDNS) for the AgentZero daemon. Provides an
//! `Advertiser` trait so the gateway can stay decoupled from the underlying
//! mDNS implementation and so tests can swap in a no-op or recorder.

#![forbid(unsafe_code)]

pub mod advertiser;
pub mod config;
pub mod interfaces;
pub mod mdns;
pub mod network_info;

pub use advertiser::{
    noop, AdvertiseHandle, AdvertiseInner, Advertiser, DiscoveryError, NoopAdvertiser,
    Result as DiscoveryResult, ServiceInfo,
};
pub use config::{AdvancedConfig, DiscoveryConfig, DiscoveryDetails};
pub use interfaces::{
    filter_interfaces, ipv4_only, Interface, InterfaceEnumerator, RealEnumerator,
};
pub use mdns::MdnsAdvertiser;
pub use network_info::{collect_network_info, sanitize_for_hostname, MdnsStatus, NetworkInfo};
