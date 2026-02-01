//! # Hooks Module
//!
//! Unified abstraction for all inbound triggers (CLI, Web, Cron, WhatsApp, Telegram, etc).
//!
//! The hook system provides:
//! - **HookContext**: Tracks where a message came from
//! - **Hook trait**: Interface for responding back to the origin
//! - **HookRegistry**: Central registry for routing responses
//!
//! ## Usage
//!
//! When a message arrives (via WebSocket, webhook, CLI, etc), create a `HookContext`
//! and pass it through the execution pipeline. The `respond` tool uses this context
//! to route responses back to the correct channel.
//!
//! ```rust,ignore
//! // Create context when message arrives
//! let ctx = HookContext::web("session-123");
//!
//! // Pass through execution
//! runtime.invoke_with_hook(agent_id, conversation_id, message, ctx).await;
//!
//! // In the respond tool:
//! hook_registry.respond(&ctx, "Hello!", ResponseFormat::Text, None).await;
//! ```

pub mod cli;
pub mod context;
pub mod cron;
pub mod registry;
pub mod web;

pub use cli::CliHook;
pub use context::{HookContext, HookType};
pub use cron::CronHook;
pub use registry::{Attachment, HookRegistry, NoOpHook, ResponseFormat};
pub use web::WebHook;

use async_trait::async_trait;

/// Trait for hook implementations.
///
/// A hook represents a channel through which messages can be received
/// and responses can be sent back.
#[async_trait]
pub trait Hook: Send + Sync {
    /// Get the hook type this implementation handles.
    fn hook_type(&self) -> HookType;

    /// Send a response back through this hook.
    ///
    /// # Arguments
    /// * `ctx` - The hook context from the original message
    /// * `message` - The response message
    /// * `format` - The response format (text, markdown, html)
    /// * `attachments` - Optional attachments (images, files, etc)
    async fn respond(
        &self,
        ctx: &HookContext,
        message: &str,
        format: ResponseFormat,
        attachments: Option<Vec<Attachment>>,
    ) -> Result<(), String>;

    /// Check if this hook can handle the given context.
    ///
    /// This allows hooks to validate that they're the right handler
    /// for a particular context (e.g., checking session IDs).
    fn can_handle(&self, ctx: &HookContext) -> bool;
}
