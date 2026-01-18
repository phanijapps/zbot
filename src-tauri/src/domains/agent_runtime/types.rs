// ============================================================================
// TYPE CONVERSIONS
// Converts between zero-app types and Tauri app types
// ============================================================================

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use zero_app::prelude::*;

// Type alias for Result with String error type (for Tauri compatibility)
type TResult<T> = std::result::Result<T, String>;

// ============================================================================
// TAURI-SPECIFIC TYPES
// ============================================================================

/// Agent event for Tauri frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TauriAgentEvent {
    pub id: String,
    #[serde(rename = "invocationId")]
    pub invocation_id: String,
    pub timestamp: i64,
    pub author: String,
    pub content: Option<TauriContent>,
    #[serde(rename = "turnComplete")]
    pub turn_complete: bool,
    pub actions: TauriEventActions,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl From<Event> for TauriAgentEvent {
    fn from(event: Event) -> Self {
        Self {
            id: event.id,
            invocation_id: event.invocation_id,
            timestamp: event.timestamp.timestamp(),
            author: event.author,
            content: event.content.map(|c| TauriContent::from(c)),
            turn_complete: event.turn_complete,
            actions: TauriEventActions::from(event.actions),
            metadata: event.metadata,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TauriContent {
    pub role: String,
    pub parts: Vec<TauriPart>,
}

impl From<Content> for TauriContent {
    fn from(content: Content) -> Self {
        Self {
            role: content.role,
            parts: content.parts.into_iter().map(TauriPart::from).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TauriPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "functionCall")]
    FunctionCall {
        id: Option<String>,
        name: String,
        #[serde(rename = "arguments")]
        args: serde_json::Value,
    },
    #[serde(rename = "functionResponse")]
    FunctionResponse {
        id: String,
        response: serde_json::Value,
    },
}

impl From<Part> for TauriPart {
    fn from(part: Part) -> Self {
        match part {
            Part::Text { text } => TauriPart::Text { text },
            Part::FunctionCall { id, name, args } => TauriPart::FunctionCall {
                id,
                name,
                args,
            },
            Part::FunctionResponse { id, response } => TauriPart::FunctionResponse {
                id,
                response: response.clone().into(),
            },
            // Note: Binary parts are not yet supported in TauriPart
            // For now, convert them to empty text
            Part::Binary { .. } => TauriPart::Text { text: String::new() },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TauriEventActions {
    #[serde(rename = "stateDelta")]
    pub state_delta: HashMap<String, serde_json::Value>,
    #[serde(rename = "skipSummarization")]
    pub skip_summarization: bool,
    #[serde(rename = "transferToAgent")]
    pub transfer_to_agent: Option<String>,
    pub escalate: bool,
}

impl From<EventActions> for TauriEventActions {
    fn from(actions: EventActions) -> Self {
        Self {
            state_delta: actions.state_delta,
            skip_summarization: actions.skip_summarization,
            transfer_to_agent: actions.transfer_to_agent,
            escalate: actions.escalate,
        }
    }
}

/// LLM configuration for Tauri
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TauriLlmConfig {
    #[serde(rename = "providerId")]
    pub provider_id: String,
    pub model: String,
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    pub temperature: Option<f32>,
    #[serde(rename = "maxTokens")]
    pub max_tokens: Option<u32>,
    #[serde(rename = "thinkingEnabled")]
    pub thinking_enabled: Option<bool>,
}

impl From<LlmConfig> for TauriLlmConfig {
    fn from(config: LlmConfig) -> Self {
        Self {
            provider_id: "openai".to_string(), // Default provider
            model: config.model.clone(),
            api_key: Some(config.api_key.clone()),
            base_url: config.base_url().to_string(),
            temperature: config.temperature,
            max_tokens: config.max_tokens,
            thinking_enabled: None, // Not in LlmConfig
        }
    }
}

impl From<TauriLlmConfig> for LlmConfig {
    fn from(config: TauriLlmConfig) -> Self {
        LlmConfig {
            api_key: config.api_key.unwrap_or_default(),
            base_url: Some(config.base_url),
            model: config.model,
            organization_id: None, // Not in TauriLlmConfig
            temperature: config.temperature,
            max_tokens: config.max_tokens,
        }
    }
}

/// MCP server configuration for Tauri
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TauriMcpServerConfig {
    pub id: String,
    pub name: String,
    pub transport: String, // "stdio" or "http"
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub url: Option<String>,
    pub enabled: Option<bool>,
}

impl From<McpServerConfig> for TauriMcpServerConfig {
    fn from(config: McpServerConfig) -> Self {
        let transport = match &config.transport {
            McpTransport::Stdio => "stdio".to_string(),
            McpTransport::Http => "http".to_string(),
            McpTransport::Sse => "sse".to_string(),
        };

        Self {
            id: config.id,
            name: config.name,
            transport,
            command: config.command.as_ref().map(|c| c.command.clone()),
            args: config.command.as_ref().map(|c| c.args.clone()),
            url: config.url,
            enabled: Some(config.enabled),
        }
    }
}

impl From<TauriMcpServerConfig> for McpServerConfig {
    fn from(config: TauriMcpServerConfig) -> Self {
        let transport = match config.transport.as_str() {
            "stdio" => McpTransport::Stdio,
            "http" => McpTransport::Http,
            "sse" => McpTransport::Sse,
            _ => McpTransport::Stdio,
        };

        let command = config.command.map(|cmd| McpCommand {
            command: cmd,
            args: config.args.unwrap_or_default(),
        });

        Self {
            id: config.id,
            name: config.name,
            transport,
            command,
            url: config.url,
            headers: Default::default(),
            env: Default::default(),
            enabled: config.enabled.unwrap_or(true),
        }
    }
}

/// Middleware configuration for Tauri
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TauriMiddlewareConfig {
    pub summarization: Option<TauriSummarizationConfig>,
    #[serde(rename = "contextEditing")]
    pub context_editing: Option<TauriContextEditingConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TauriSummarizationConfig {
    pub enabled: bool,
    #[serde(rename = "maxTokens")]
    pub max_tokens: u32,
    #[serde(rename = "triggerAt")]
    pub trigger_at: u32,
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TauriContextEditingConfig {
    pub enabled: bool,
    #[serde(rename = "keepLastN")]
    pub keep_last_n: usize,
    #[serde(rename = "keepPolicy")]
    pub keep_policy: String,
}

impl From<MiddlewareConfig> for TauriMiddlewareConfig {
    fn from(config: MiddlewareConfig) -> Self {
        Self {
            summarization: config.summarization.map(|s| TauriSummarizationConfig {
                enabled: s.enabled,
                max_tokens: s.summary_max_tokens as u32,
                trigger_at: s.trigger.tokens.unwrap_or(8000) as u32,
                provider: s.provider,
            }),
            context_editing: config.context_editing.map(|c| TauriContextEditingConfig {
                enabled: c.enabled,
                keep_last_n: c.keep_tool_results,
                keep_policy: "all".to_string(), // Default policy
            }),
        }
    }
}

/// Content conversions between zero-app and Tauri
impl From<TauriContent> for Content {
    fn from(content: TauriContent) -> Self {
        Self {
            role: content.role,
            parts: content.parts.into_iter().map(|p| match p {
                TauriPart::Text { text } => Part::Text { text },
                TauriPart::FunctionCall { id, name, args } => Part::FunctionCall {
                    id,
                    name,
                    args,
                },
                TauriPart::FunctionResponse { id, response } => Part::FunctionResponse {
                    id,
                    response: response.to_string(),
                },
            }).collect(),
        }
    }
}

// ============================================================================
// STREAM EVENT CONVERSIONS
// ============================================================================

/// Stream event for Tauri (simplified version of zero-app Event)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TauriStreamEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub content: Option<String>,
    #[serde(rename = "toolCall")]
    pub tool_call: Option<TauriToolCallInfo>,
    #[serde(rename = "turnComplete")]
    pub turn_complete: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TauriToolCallInfo {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

// ============================================================================
// AGENT RUNTIME CONFIG
// ============================================================================

/// Configuration for creating an executor
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    pub agent_id: String,
    pub agent_name: String,
    pub agent_type: String,
    pub llm_config: LlmConfig,
    pub system_instruction: Option<String>,
    pub sub_agents: Vec<ExecutorConfig>,
    pub conditional_config: Option<ConditionalExecutorConfig>,
    pub loop_config: Option<LoopExecutorConfig>,
}

#[derive(Debug, Clone)]
pub struct ConditionalExecutorConfig {
    pub condition: String,
    pub if_agent: String,
    pub else_agent: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LoopExecutorConfig {
    pub max_iterations: Option<u32>,
    pub until_escalation: bool,
}

// ============================================================================
// CONVERSION HELPERS
// ============================================================================

/// Convert zero-app Content to Tauri content format
pub fn content_to_tauri(content: &Content) -> TauriContent {
    TauriContent::from(content.clone())
}

/// Convert zero-app Event to Tauri event format
pub fn event_to_tauri(event: Event) -> TauriAgentEvent {
    TauriAgentEvent::from(event)
}

/// Convert zero-app events to Tauri events
pub fn events_to_tauri(events: Vec<Event>) -> Vec<TauriAgentEvent> {
    events.into_iter().map(event_to_tauri).collect()
}

/// Convert Tauri content to zero-app Content
pub fn content_from_tauri(content: TauriContent) -> Content {
    Content::from(content)
}

/// Convert JSON value to zero-app Content
pub fn json_to_content(json: serde_json::Value) -> TResult<Content> {
    serde_json::from_value(json)
        .map_err(|e| format!("Failed to convert JSON to content: {}", e))
}

/// Convert zero-app Content to JSON
pub fn content_to_json(content: &Content) -> TResult<serde_json::Value> {
    serde_json::to_value(content)
        .map_err(|e| format!("Failed to convert content to JSON: {}", e))
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_conversion() {
        let content = Content::user("Hello world");
        let tauri_content = TauriContent::from(content.clone());

        assert_eq!(tauri_content.role, "user");
        assert_eq!(tauri_content.parts.len(), 1);

        let converted_back = Content::from(tauri_content);
        assert_eq!(converted_back.role, content.role);
    }

    #[test]
    fn test_llm_config_conversion() {
        let config = LlmConfig::new("test-key", "gpt-4");
        let tauri_config = TauriLlmConfig::from(config.clone());

        assert_eq!(tauri_config.model, "gpt-4");
        assert_eq!(tauri_config.api_key, Some("test-key".to_string()));
    }

    #[test]
    fn test_event_conversion() {
        let event = Event::new("test-invocation")
            .with_author("assistant")
            .with_content(Content::assistant("Hello"));

        let tauri_event = TauriAgentEvent::from(event);

        assert_eq!(tauri_event.author, "assistant");
        assert!(tauri_event.content.is_some());
    }
}
