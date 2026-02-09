//! # Services
//!
//! Shared services for gateway operations.
//!
//! Most services are provided by the `gateway-services` crate.
//! RuntimeService remains here due to its dependency on execution types.

// Re-export all services from the gateway-services crate
pub use gateway_services::*;

// RuntimeService stays in gateway (depends on execution types)
pub mod runtime;
pub use runtime::RuntimeService;
