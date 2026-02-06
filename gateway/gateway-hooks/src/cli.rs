//! CLI hook implementation.
//!
//! Routes responses to stdout for command-line interfaces.

use crate::{Hook, HookContext, HookType};
use crate::registry::{Attachment, ResponseFormat};
use gateway_events::{EventBus, GatewayEvent};
use async_trait::async_trait;
use std::sync::Arc;

/// CLI hook for routing responses to stdout.
///
/// This hook prints responses to the standard output stream,
/// making it suitable for command-line tool integrations.
pub struct CliHook {
    /// Event bus for emitting respond events.
    event_bus: Arc<EventBus>,
}

impl CliHook {
    /// Create a new CLI hook.
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self { event_bus }
    }
}

#[async_trait]
impl Hook for CliHook {
    fn hook_type(&self) -> HookType {
        HookType::Cli
    }

    async fn respond(
        &self,
        ctx: &HookContext,
        message: &str,
        format: ResponseFormat,
        _attachments: Option<Vec<Attachment>>,
    ) -> Result<(), String> {
        // Verify this is a CLI context
        if !matches!(ctx.hook_type, HookType::Cli) {
            return Err("CliHook can only handle CLI hook contexts".to_string());
        }

        // Get conversation ID from metadata or use source_id
        let conversation_id = ctx
            .metadata
            .get("conversation_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| ctx.source_id.clone());

        // Emit a respond event
        self.event_bus
            .publish(GatewayEvent::Respond {
                session_id: conversation_id.clone(), // Use conversation_id as session_id for CLI
                execution_id: conversation_id.clone(), // Use conversation_id as execution_id for CLI
                message: message.to_string(),
                conversation_id: Some(conversation_id.clone()),
            })
            .await;

        // Print to stdout based on format
        match format {
            ResponseFormat::Markdown => {
                // For markdown, we could use a terminal markdown renderer
                // For now, just print as-is with a visual indicator
                println!("\n--- Response ---");
                println!("{}", message);
                println!("----------------\n");
            }
            ResponseFormat::Html => {
                // Strip HTML tags for plain text display
                // For simplicity, just print as-is with a warning
                println!("\n[HTML Response]");
                println!("{}", message);
            }
            ResponseFormat::Text => {
                // Plain text - just print
                println!("{}", message);
            }
        }

        Ok(())
    }

    fn can_handle(&self, ctx: &HookContext) -> bool {
        matches!(ctx.hook_type, HookType::Cli)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cli_hook_can_handle() {
        let event_bus = Arc::new(EventBus::new());
        let hook = CliHook::new(event_bus);

        let cli_ctx = HookContext::cli("test-source");
        assert!(hook.can_handle(&cli_ctx));

        let web_ctx = HookContext::web("session-123");
        assert!(!hook.can_handle(&web_ctx));
    }

    #[test]
    fn test_cli_hook_type() {
        let event_bus = Arc::new(EventBus::new());
        let hook = CliHook::new(event_bus);
        assert!(matches!(hook.hook_type(), HookType::Cli));
    }
}
