//! WebSocket hook implementation.
//!
//! Routes responses back to the originating WebSocket session.

use super::context::{HookContext, HookType};
use super::registry::{Attachment, ResponseFormat};
use super::Hook;
use crate::events::{EventBus, GatewayEvent};
use crate::websocket::{ServerMessage, SessionRegistry};
use async_trait::async_trait;
use std::sync::Arc;

/// WebSocket hook for routing responses to web clients.
///
/// This hook sends responses through the WebSocket connection
/// that the original message came from.
pub struct WebHook {
    /// Session registry for finding WebSocket sessions.
    session_registry: Arc<SessionRegistry>,

    /// Event bus for emitting respond events.
    event_bus: Arc<EventBus>,
}

impl WebHook {
    /// Create a new WebSocket hook.
    pub fn new(session_registry: Arc<SessionRegistry>, event_bus: Arc<EventBus>) -> Self {
        Self {
            session_registry,
            event_bus,
        }
    }
}

#[async_trait]
impl Hook for WebHook {
    fn hook_type(&self) -> HookType {
        HookType::Web {
            session_id: String::new(), // Default, actual session comes from context
        }
    }

    async fn respond(
        &self,
        ctx: &HookContext,
        message: &str,
        _format: ResponseFormat,
        _attachments: Option<Vec<Attachment>>,
    ) -> Result<(), String> {
        // Get session ID from context
        let session_id = match &ctx.hook_type {
            HookType::Web { session_id } => session_id.clone(),
            _ => return Err("WebHook can only handle Web hook contexts".to_string()),
        };

        // Get conversation ID from metadata or use source_id
        let conversation_id = ctx
            .metadata
            .get("conversation_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| ctx.source_id.clone());

        // Emit a respond event that can be picked up by adapters
        self.event_bus
            .publish(GatewayEvent::Respond {
                session_id: session_id.clone(),
                execution_id: session_id.clone(), // Use session_id as execution_id for hook context
                message: message.to_string(),
                conversation_id: Some(conversation_id.clone()),
            })
            .await;

        // Try to send directly to the WebSocket session
        if let Some(session) = self.session_registry.get(&session_id).await {
            let msg = ServerMessage::TurnComplete {
                conversation_id,
                final_message: Some(message.to_string()),
                seq: None,
            };

            session
                .send(msg)
                .map_err(|e| format!("Failed to send to WebSocket: {}", e))?;
        }

        Ok(())
    }

    fn can_handle(&self, ctx: &HookContext) -> bool {
        matches!(ctx.hook_type, HookType::Web { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_web_hook_can_handle() {
        let registry = Arc::new(SessionRegistry::new());
        let event_bus = Arc::new(EventBus::new());
        let hook = WebHook::new(registry, event_bus);

        let web_ctx = HookContext::web("session-123");
        assert!(hook.can_handle(&web_ctx));

        let cli_ctx = HookContext::cli("test");
        assert!(!hook.can_handle(&cli_ctx));
    }
}
