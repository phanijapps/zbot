//! # Discovery
//!
//! LAN service advertisement (mDNS) for the AgentZero daemon. Provides an
//! `Advertiser` trait so the gateway can stay decoupled from the underlying
//! mDNS implementation and so tests can swap in a no-op or recorder.

#![forbid(unsafe_code)]

pub mod config;
pub mod interfaces;
pub mod network_info;

pub use config::{AdvancedConfig, DiscoveryConfig, DiscoveryDetails};
pub use interfaces::{
    filter_interfaces, ipv4_only, Interface, InterfaceEnumerator, RealEnumerator,
};
pub use network_info::{collect_network_info, sanitize_for_hostname, MdnsStatus, NetworkInfo};
