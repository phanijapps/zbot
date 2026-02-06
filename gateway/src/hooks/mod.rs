//! # Hooks Module
//!
//! Unified abstraction for all inbound triggers (CLI, Web, Cron, WhatsApp, Telegram, etc).
//!
//! Most hook types are provided by the `gateway-hooks` crate.
//! WebHook stays here due to its dependency on the websocket module.

// Re-export all hook types from the gateway-hooks crate
pub use gateway_hooks::*;

// Context types re-exported via gateway-hooks (originally from gateway-events)
pub mod context {
    pub use gateway_events::{HookContext, HookType};
}

// WebHook stays in gateway (depends on websocket module)
pub mod web;
pub use web::WebHook;
