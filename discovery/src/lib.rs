//! # Discovery
//!
//! LAN service advertisement (mDNS) for the AgentZero daemon. Provides an
//! `Advertiser` trait so the gateway can stay decoupled from the underlying
//! mDNS implementation and so tests can swap in a no-op or recorder.

#![forbid(unsafe_code)]
