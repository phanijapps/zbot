//! Cron hook implementation.
//!
//! Handles scheduled/cron-triggered agent invocations.
//! Since cron jobs don't have a response channel, responses are logged.

use super::context::{HookContext, HookType};
use super::registry::{Attachment, ResponseFormat};
use super::Hook;
use crate::events::{EventBus, GatewayEvent};
use async_trait::async_trait;
use std::sync::Arc;

/// Cron hook for scheduled agent invocations.
///
/// This hook handles agents triggered by cron schedules.
/// Since there's no interactive channel, responses are logged
/// and emitted as events for monitoring systems.
pub struct CronHook {
    /// Event bus for emitting events.
    event_bus: Arc<EventBus>,
}

impl CronHook {
    /// Create a new cron hook.
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self { event_bus }
    }
}

#[async_trait]
impl Hook for CronHook {
    fn hook_type(&self) -> HookType {
        HookType::Cron {
            job_id: String::new(),
        }
    }

    async fn respond(
        &self,
        ctx: &HookContext,
        message: &str,
        _format: ResponseFormat,
        _attachments: Option<Vec<Attachment>>,
    ) -> Result<(), String> {
        // Verify this is a Cron context
        let job_id = match &ctx.hook_type {
            HookType::Cron { job_id } => job_id.clone(),
            _ => return Err("CronHook can only handle Cron hook contexts".to_string()),
        };

        // Get conversation ID from metadata or use source_id
        let conversation_id = ctx
            .metadata
            .get("conversation_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| ctx.source_id.clone());

        // Log the response (cron jobs don't have interactive channels)
        tracing::info!(
            job_id = %job_id,
            conversation_id = %conversation_id,
            message_length = %message.len(),
            "Cron job response"
        );

        // Emit respond event for monitoring/logging systems
        self.event_bus
            .publish(GatewayEvent::Respond {
                conversation_id,
                message: message.to_string(),
                session_id: None,
            })
            .await;

        Ok(())
    }

    fn can_handle(&self, ctx: &HookContext) -> bool {
        matches!(ctx.hook_type, HookType::Cron { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cron_hook_can_handle() {
        let event_bus = Arc::new(EventBus::new());
        let hook = CronHook::new(event_bus);

        let cron_ctx = HookContext::cron("daily-backup");
        assert!(hook.can_handle(&cron_ctx));

        let web_ctx = HookContext::web("session-123");
        assert!(!hook.can_handle(&web_ctx));
    }

    #[test]
    fn test_cron_hook_type() {
        let event_bus = Arc::new(EventBus::new());
        let hook = CronHook::new(event_bus);
        assert!(matches!(hook.hook_type(), HookType::Cron { .. }));
    }
}
