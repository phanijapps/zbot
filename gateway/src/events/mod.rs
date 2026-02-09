//! # Events Module
//!
//! Re-exports from the `gateway-events` crate.
//!
//! This module exists for backward compatibility — all types are defined
//! in the extracted `gateway-events` crate and re-exported here so that
//! `use crate::events::{EventBus, GatewayEvent}` continues to work.

pub use gateway_events::*;
