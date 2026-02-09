//! Gateway Bus - Unified intake interface for all session triggers.
//!
//! Most types are provided by the `gateway-bus` crate.
//! `HttpGatewayBus` stays here due to its dependency on the execution module.

// Re-export all bus types and trait from gateway-bus crate
pub use gateway_bus::*;

// HttpGatewayBus stays in gateway (depends on execution module)
mod http_bus;
pub use http_bus::HttpGatewayBus;
