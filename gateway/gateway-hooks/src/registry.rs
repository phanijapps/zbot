//! Hook registry for managing and routing to different hooks.

use crate::{Hook, HookContext, HookType};
use async_trait::async_trait;
use gateway_events::EventBus;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Attachment for hook responses (images, files, etc).
#[derive(Debug, Clone)]
pub struct Attachment {
    /// MIME type of the attachment.
    pub content_type: String,

    /// File name or identifier.
    pub name: String,

    /// Attachment data.
    pub data: Vec<u8>,
}

impl Attachment {
    /// Create a new attachment.
    pub fn new(content_type: impl Into<String>, name: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            content_type: content_type.into(),
            name: name.into(),
            data,
        }
    }

    /// Create a text file attachment.
    pub fn text(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new("text/plain", name, content.into().into_bytes())
    }
}

/// Response format for hook responses.
#[derive(Debug, Clone, Default)]
pub enum ResponseFormat {
    /// Plain text.
    #[default]
    Text,
    /// Markdown.
    Markdown,
    /// HTML.
    Html,
}

/// Registry for managing hooks.
///
/// Provides a central place to register hooks and route responses
/// back to the originating hook based on the HookContext.
pub struct HookRegistry {
    /// Registered hooks by type name.
    hooks: RwLock<HashMap<String, Arc<dyn Hook>>>,

    /// Event bus for emitting respond events.
    event_bus: Arc<EventBus>,
}

impl HookRegistry {
    /// Create a new hook registry.
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            hooks: RwLock::new(HashMap::new()),
            event_bus,
        }
    }

    /// Register a hook.
    pub async fn register(&self, hook: Arc<dyn Hook>) {
        let type_name = hook.hook_type().type_name().to_string();
        let mut hooks = self.hooks.write().await;
        hooks.insert(type_name, hook);
    }

    /// Get a hook by type.
    pub async fn get(&self, hook_type: &HookType) -> Option<Arc<dyn Hook>> {
        let hooks = self.hooks.read().await;
        hooks.get(hook_type.type_name()).cloned()
    }

    /// Get the event bus.
    pub fn event_bus(&self) -> Arc<EventBus> {
        self.event_bus.clone()
    }

    /// Route a response to the correct hook based on context.
    ///
    /// This is the main method used by the respond tool.
    pub async fn respond(
        &self,
        ctx: &HookContext,
        message: &str,
        format: ResponseFormat,
        attachments: Option<Vec<Attachment>>,
    ) -> Result<(), String> {
        // Check if hook type supports responses
        if !ctx.hook_type.supports_response() {
            return Err(format!(
                "Hook type '{}' does not support responses",
                ctx.hook_type.type_name()
            ));
        }

        // Find the hook
        let hook = self.get(&ctx.hook_type).await.ok_or_else(|| {
            format!(
                "No hook registered for type '{}'",
                ctx.hook_type.type_name()
            )
        })?;

        // Check if hook can handle this context
        if !hook.can_handle(ctx) {
            return Err(format!(
                "Hook '{}' cannot handle this context",
                ctx.hook_type.type_name()
            ));
        }

        // Send response
        hook.respond(ctx, message, format, attachments).await
    }

    /// List all registered hook type names.
    pub async fn list_hook_types(&self) -> Vec<String> {
        let hooks = self.hooks.read().await;
        hooks.keys().cloned().collect()
    }
}

/// No-op hook for testing or when no real hook is needed.
pub struct NoOpHook {
    hook_type: HookType,
}

impl NoOpHook {
    /// Create a new no-op hook.
    pub fn new(hook_type: HookType) -> Self {
        Self { hook_type }
    }

    /// Create a CLI no-op hook.
    pub fn cli() -> Self {
        Self::new(HookType::Cli)
    }

    /// Create a cron no-op hook.
    pub fn cron(job_id: impl Into<String>) -> Self {
        Self::new(HookType::Cron {
            job_id: job_id.into(),
        })
    }
}

#[async_trait]
impl Hook for NoOpHook {
    fn hook_type(&self) -> HookType {
        self.hook_type.clone()
    }

    async fn respond(
        &self,
        _ctx: &HookContext,
        message: &str,
        _format: ResponseFormat,
        _attachments: Option<Vec<Attachment>>,
    ) -> Result<(), String> {
        // Just log and return
        tracing::info!("NoOpHook response: {}", message);
        Ok(())
    }

    fn can_handle(&self, ctx: &HookContext) -> bool {
        ctx.hook_type.type_name() == self.hook_type.type_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hook_registry() {
        let event_bus = Arc::new(EventBus::new());
        let registry = HookRegistry::new(event_bus);

        let hook = Arc::new(NoOpHook::cli());
        registry.register(hook).await;

        let types = registry.list_hook_types().await;
        assert!(types.contains(&"cli".to_string()));

        let ctx = HookContext::cli("test");
        let result = registry
            .respond(&ctx, "Hello", ResponseFormat::Text, None)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cron_no_response() {
        let event_bus = Arc::new(EventBus::new());
        let registry = HookRegistry::new(event_bus);

        let hook = Arc::new(NoOpHook::cron("job-1"));
        registry.register(hook).await;

        let ctx = HookContext::cron("job-1");
        let result = registry
            .respond(&ctx, "Hello", ResponseFormat::Text, None)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not support responses"));
    }
}
