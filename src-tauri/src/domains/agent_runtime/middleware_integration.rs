// ============================================================================
// MIDDLEWARE INTEGRATION
// Integrates zero-middleware with the agent execution flow
// ============================================================================

use std::sync::Arc;
use zero_app::prelude::*;
use crate::domains::agent_runtime::config_adapter::MiddlewareYamlConfig;

// Type alias for Result with String error type (for Tauri compatibility)
type TResult<T> = std::result::Result<T, String>;

// Middleware types are re-exported via zero_app::prelude
// We use them directly without crate prefix

// ============================================================================
// MIDDLEWARE BUILDER
// ============================================================================

/// Build middleware pipeline from YAML configuration
pub struct MiddlewareBuilder {
    llm: Arc<dyn Llm>,
    provider_id: String,
}

impl MiddlewareBuilder {
    pub fn new(llm: Arc<dyn Llm>, provider_id: String) -> Self {
        Self { llm, provider_id }
    }

    /// Build middleware pipeline from YAML config
    pub fn build_from_yaml(&self, yaml_config: &MiddlewareYamlConfig) -> TResult<MiddlewarePipeline> {
        let mut pipeline = MiddlewarePipeline::new();

        // Add summarization middleware if configured
        if let Some(summarization) = &yaml_config.summarization {
            if summarization.enabled.unwrap_or(false) {
                match self.build_summarization_middleware(summarization) {
                    Ok(middleware) => {
                        tracing::info!("Summarization middleware added to pipeline");
                        pipeline = pipeline.add_pre_processor(middleware);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to build summarization middleware: {}, skipping", e);
                    }
                }
            }
        }

        // Add context editing middleware if configured
        if let Some(context_editing) = &yaml_config.context_editing {
            if context_editing.enabled.unwrap_or(false) {
                match self.build_context_editing_middleware(context_editing) {
                    Ok(middleware) => {
                        tracing::info!("Context editing middleware added to pipeline");
                        pipeline = pipeline.add_pre_processor(middleware);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to build context editing middleware: {}, skipping", e);
                    }
                }
            }
        }

        Ok(pipeline)
    }

    /// Build summarization middleware from config
    fn build_summarization_middleware(
        &self,
        config: &crate::domains::agent_runtime::config_adapter::SummarizationYamlConfig,
    ) -> TResult<Box<dyn PreProcessMiddleware>> {
        let summarization_config = SummarizationConfig {
            enabled: config.enabled.unwrap_or(true),
            model: None, // Use agent's model
            provider: config.provider.clone(),
            trigger: TriggerCondition {
                tokens: config.trigger_at.map(|t| t as usize),
                messages: None,
                fraction: None,
            },
            keep: KeepPolicy {
                messages: Some(10), // Default: keep 10 messages
                tokens: None,
                fraction: None,
            },
            summary_max_tokens: config.max_tokens.unwrap_or(1000) as usize,
            summary_prompt: None,
            summary_prefix: "[Previous conversation summarized]".to_string(),
            custom_token_counter: None,
        };

        // Create the middleware - it uses placeholder summaries without needing an LLM
        Ok(Box::new(SummarizationMiddleware::new(summarization_config)))
    }

    /// Build context editing middleware from config
    fn build_context_editing_middleware(
        &self,
        config: &crate::domains::agent_runtime::config_adapter::ContextEditingYamlConfig,
    ) -> TResult<Box<dyn PreProcessMiddleware>> {
        let context_editing_config = ContextEditingConfig {
            enabled: config.enabled.unwrap_or(true),
            trigger_tokens: config.keep_last_n.unwrap_or(10) * 1000, // Rough estimate
            keep_tool_results: 3, // Default: keep 3 tool results
            min_reclaim: 0,
            clear_tool_inputs: false,
            exclude_tools: Vec::new(),
            placeholder: "[cleared]".to_string(),
        };

        Ok(Box::new(ContextEditingMiddleware::new(context_editing_config)))
    }
}

// ============================================================================
// MIDDLEWARE EXECUTOR
// ============================================================================

/// Execute middleware pipeline on messages
pub struct MiddlewareExecutor {
    pipeline: Arc<MiddlewarePipeline>,
}

impl MiddlewareExecutor {
    pub fn new(pipeline: Arc<MiddlewarePipeline>) -> Self {
        Self { pipeline }
    }

    /// Apply pre-processing to messages
    pub async fn apply_preprocessing(
        &self,
        messages: Vec<Content>,
        message_count: usize,
        estimated_tokens: usize,
        context_window: usize,
    ) -> TResult<Vec<Content>> {
        let context = MiddlewareContext {
            message_count,
            estimated_tokens,
            context_window,
            metadata: serde_json::json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
        };

        // Use the pipeline's process_messages method
        let processed = self.pipeline.process_messages(messages, &context, |event| {
            // Handle emitted events - for now just log them
            tracing::info!("Middleware emitted event: {}", event.event_type);
        }).await.map_err(|e| format!("Middleware error: {}", e))?;

        Ok(processed)
    }

    /// Estimate token count for messages
    pub fn estimate_tokens(&self, messages: &[Content]) -> usize {
        // Use the token counter from zero-middleware (re-exported via zero-app)
        // For now, use a simple estimation
        messages.iter()
            .map(|m| m.parts.iter()
                .map(|p| match p {
                    Part::Text { text } => text.len() / 4, // Rough estimate: 4 chars per token
                    _ => 0,
                })
                .sum::<usize>())
            .sum()
    }

    /// Get context window for a model
    pub fn get_context_window(&self, model: &str) -> usize {
        // Default context window for most models
        // In production, this would query the model's actual context window
        match model {
            m if m.contains("gpt-4") => 128000,
            m if m.contains("gpt-3.5") => 16000,
            m if m.contains("claude-3") => 200000,
            _ => 128000,
        }
    }
}

// ============================================================================
// MIDDLEWARE FACTORY
// ============================================================================

/// Factory for creating middleware integrations
pub struct MiddlewareFactory {
    llm: Arc<dyn Llm>,
    provider_id: String,
}

impl MiddlewareFactory {
    pub fn new(llm: Arc<dyn Llm>, provider_id: String) -> Self {
        Self { llm, provider_id }
    }

    /// Create a middleware executor from YAML config
    pub async fn create_executor(
        &self,
        yaml_config: Option<&MiddlewareYamlConfig>,
    ) -> TResult<Arc<MiddlewareExecutor>> {
        let pipeline = if let Some(yaml) = yaml_config {
            let builder = MiddlewareBuilder::new(self.llm.clone(), self.provider_id.clone());
            builder.build_from_yaml(yaml)?
        } else {
            MiddlewarePipeline::new()
        };

        Ok(Arc::new(MiddlewareExecutor::new(Arc::new(pipeline))))
    }

    /// Create a minimal middleware pipeline (no processing)
    pub fn create_minimal_pipeline() -> Arc<MiddlewarePipeline> {
        Arc::new(MiddlewarePipeline::new())
    }

    /// Create a default middleware pipeline with common middlewares
    pub fn create_default_pipeline() -> Arc<MiddlewarePipeline> {
        // For now, just return an empty pipeline
        // In the future, this could add sensible defaults
        Arc::new(MiddlewarePipeline::new())
    }
}

// ============================================================================
// HELPERS
// ============================================================================

/// Convert agent-runtime middleware config to zero-middleware format
pub fn convert_middleware_config(
    yaml_config: Option<&MiddlewareYamlConfig>,
) -> Option<MiddlewareConfig> {
    yaml_config.map(|yaml| {
        let summarization = yaml.summarization.as_ref().map(|s| {
            SummarizationConfig {
                enabled: s.enabled.unwrap_or(true),
                model: None,
                provider: s.provider.clone(),
                trigger: TriggerCondition {
                    tokens: s.trigger_at.map(|t| t as usize),
                    messages: None,
                    fraction: None,
                },
                keep: KeepPolicy {
                    messages: Some(10),
                    tokens: None,
                    fraction: None,
                },
                summary_max_tokens: s.max_tokens.unwrap_or(1000) as usize,
                summary_prompt: None,
                summary_prefix: "[Previous conversation summarized]".to_string(),
                custom_token_counter: None,
            }
        });

        let context_editing = yaml.context_editing.as_ref().map(|c| {
            ContextEditingConfig {
                enabled: c.enabled.unwrap_or(true),
                trigger_tokens: c.keep_last_n.unwrap_or(10) * 1000,
                keep_tool_results: 3,
                min_reclaim: 0,
                clear_tool_inputs: false,
                exclude_tools: Vec::new(),
                placeholder: "[cleared]".to_string(),
            }
        });

        MiddlewareConfig {
            summarization,
            context_editing,
            custom: std::collections::HashMap::new(),
        }
    })
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_middleware_config() {
        let yaml = MiddlewareYamlConfig {
            summarization: Some(crate::domains::agent_runtime::config_adapter::SummarizationYamlConfig {
                enabled: Some(true),
                max_tokens: Some(1000),
                trigger_at: Some(4000),
                provider: Some("openai".to_string()),
            }),
            context_editing: None,
        };

        let config = convert_middleware_config(Some(&yaml));
        assert!(config.is_some());
        assert!(config.unwrap().summarization.is_some());
    }

    #[test]
    fn test_context_editing_conversion() {
        let yaml = MiddlewareYamlConfig {
            summarization: None,
            context_editing: Some(crate::domains::agent_runtime::config_adapter::ContextEditingYamlConfig {
                enabled: Some(true),
                keep_last_n: Some(20),
                keep_policy: Some("user_only".to_string()),
            }),
        };

        let config = convert_middleware_config(Some(&yaml));
        assert!(config.is_some());
        assert!(config.unwrap().context_editing.is_some());
    }
}
