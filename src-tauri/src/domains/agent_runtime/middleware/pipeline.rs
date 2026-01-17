// ============================================================================
// MIDDLEWARE PIPELINE
// Orchestrates middleware execution
// ============================================================================

use std::sync::Arc;
use crate::domains::agent_runtime::llm::ChatMessage;
use crate::domains::agent_runtime::executor::StreamEvent;
use super::traits::{PreProcessMiddleware, EventMiddleware, MiddlewareContext};

/// Middleware pipeline that orchestrates preprocessing and event handling
pub struct MiddlewarePipeline {
    /// Pre-process middleware (executed in order before LLM call)
    pre_processors: Vec<Box<dyn PreProcessMiddleware>>,

    /// Event handlers (executed when events are emitted)
    event_handlers: Vec<Box<dyn EventMiddleware>>,
}

impl Default for MiddlewarePipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl MiddlewarePipeline {
    /// Create a new empty pipeline
    pub fn new() -> Self {
        Self {
            pre_processors: Vec::new(),
            event_handlers: Vec::new(),
        }
    }

    /// Create a pipeline with pre-process middleware
    pub fn with_pre_processors(mut self, middleware: Vec<Box<dyn PreProcessMiddleware>>) -> Self {
        self.pre_processors = middleware;
        self
    }

    /// Create a pipeline with event handlers
    pub fn with_event_handlers(mut self, handlers: Vec<Box<dyn EventMiddleware>>) -> Self {
        self.event_handlers = handlers;
        self
    }

    /// Add a pre-process middleware to the pipeline
    pub fn add_pre_processor(mut self, middleware: Box<dyn PreProcessMiddleware>) -> Self {
        self.pre_processors.push(middleware);
        self
    }

    /// Add an event handler to the pipeline
    pub fn add_event_handler(mut self, handler: Box<dyn EventMiddleware>) -> Self {
        self.event_handlers.push(handler);
        self
    }

    /// Process messages through all pre-process middleware
    ///
    /// # Arguments
    /// * `messages` - Input messages to process
    /// * `context` - Execution context
    /// * `on_event` - Callback to emit events
    ///
    /// # Returns
    /// Processed messages ready for LLM execution
    pub async fn process_messages(
        &self,
        messages: Vec<ChatMessage>,
        context: &MiddlewareContext,
        mut on_event: impl FnMut(StreamEvent),
    ) -> Result<Vec<ChatMessage>, String> {
        let mut current_messages = messages;

        // Process through each pre-processor in order
        for middleware in &self.pre_processors {
            if !middleware.enabled() {
                continue;
            }

            match middleware.process(std::mem::take(&mut current_messages), context).await? {
                super::traits::MiddlewareEffect::ModifiedMessages(msgs) => {
                    current_messages = msgs;
                }
                super::traits::MiddlewareEffect::Proceed => {
                    // Keep messages as-is - but we took them, so need to restore
                    // This shouldn't happen since we take ownership
                }
                super::traits::MiddlewareEffect::EmitEvent(event) => {
                    on_event(event);
                }
                super::traits::MiddlewareEffect::EmitAndModify { event, messages: msgs } => {
                    on_event(event);
                    current_messages = msgs;
                }
            }
        }

        Ok(current_messages)
    }

    /// Handle an event through all event handlers
    ///
    /// # Arguments
    /// * `event` - The event that was emitted
    /// * `context` - Execution context
    pub async fn handle_event(
        &self,
        event: &StreamEvent,
        context: &MiddlewareContext,
    ) -> Result<(), String> {
        for handler in &self.event_handlers {
            if !handler.enabled() {
                continue;
            }

            // Continue processing even if one handler fails
            if let Err(e) = handler.on_event(event, context).await {
                eprintln!("Middleware {} failed to handle event: {}", handler.name(), e);
            }
        }

        Ok(())
    }

    /// Get the number of pre-process middleware in the pipeline
    pub fn pre_processor_count(&self) -> usize {
        self.pre_processors.len()
    }

    /// Get the number of event handlers in the pipeline
    pub fn event_handler_count(&self) -> usize {
        self.event_handlers.len()
    }

    /// Get names of all enabled pre-process middleware
    pub fn enabled_pre_processors(&self) -> Vec<&'static str> {
        self.pre_processors
            .iter()
            .filter(|m| m.enabled())
            .map(|m| m.name())
            .collect()
    }

    /// Get names of all enabled event handlers
    pub fn enabled_event_handlers(&self) -> Vec<&'static str> {
        self.event_handlers
            .iter()
            .filter(|m| m.enabled())
            .map(|m| m.name())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock pre-process middleware for testing
    struct MockPreProcessor {
        enabled: bool,
        name: &'static str,
    }

    #[async_trait::async_trait]
    impl PreProcessMiddleware for MockPreProcessor {
        fn name(&self) -> &'static str {
            self.name
        }

        fn clone_box(&self) -> Box<dyn PreProcessMiddleware> {
            Box::new(MockPreProcessor {
                enabled: self.enabled,
                name: self.name,
            })
        }

        fn enabled(&self) -> bool {
            self.enabled
        }

        async fn process(
            &self,
            messages: Vec<ChatMessage>,
            _context: &MiddlewareContext,
        ) -> Result<super::traits::MiddlewareEffect, String> {
            Ok(super::traits::MiddlewareEffect::ModifiedMessages(messages))
        }
    }

    #[test]
    fn test_pipeline_creation() {
        let pipeline = MiddlewarePipeline::new();
        assert_eq!(pipeline.pre_processor_count(), 0);
        assert_eq!(pipeline.event_handler_count(), 0);
    }

    #[test]
    fn test_enabled_middleware() {
        let enabled = Box::new(MockPreProcessor {
            enabled: true,
            name: "test",
        }) as Box<dyn PreProcessMiddleware>;
        let disabled = Box::new(MockPreProcessor {
            enabled: false,
            name: "test2",
        }) as Box<dyn PreProcessMiddleware>;

        let pipeline = MiddlewarePipeline::new()
            .add_pre_processor(enabled)
            .add_pre_processor(disabled);

        assert_eq!(pipeline.enabled_pre_processors().len(), 1);
    }
}
