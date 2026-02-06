//! # Stream Handling
//!
//! Stream event processing and logging for agent execution.

use gateway_database::DatabaseManager;
use gateway_events::{EventBus, GatewayEvent};
use api_logs::{ExecutionLog, LogCategory, LogLevel, LogService};
use agent_runtime::StreamEvent;
use execution_state::{AgentExecution, DelegationType, StateService};
use std::sync::Arc;
use tokio::sync::mpsc;

use super::batch_writer::BatchWriterHandle;
use super::super::delegation::DelegationRequest;
use super::super::events::convert_stream_event;

// ============================================================================
// STREAM CONTEXT
// ============================================================================

/// Context for stream event processing.
///
/// Contains all the identifiers and services needed to process
/// stream events during agent execution.
#[derive(Clone)]
pub struct StreamContext {
    /// Agent ID
    pub agent_id: String,
    /// Conversation ID (for gateway events)
    pub conversation_id: String,
    /// Session ID
    pub session_id: String,
    /// Execution ID
    pub execution_id: String,
    /// Event bus for broadcasting events
    pub event_bus: Arc<EventBus>,
    /// Log service for execution tracing
    pub log_service: Arc<LogService<DatabaseManager>>,
    /// State service for token tracking
    pub state_service: Arc<StateService<DatabaseManager>>,
    /// Channel for delegation requests
    pub delegation_tx: mpsc::UnboundedSender<DelegationRequest>,
    /// Batch writer for non-blocking DB writes (token updates, logs)
    pub batch_writer: Option<BatchWriterHandle>,
}

impl StreamContext {
    /// Create a new stream context.
    pub fn new(
        agent_id: String,
        conversation_id: String,
        session_id: String,
        execution_id: String,
        event_bus: Arc<EventBus>,
        log_service: Arc<LogService<DatabaseManager>>,
        state_service: Arc<StateService<DatabaseManager>>,
        delegation_tx: mpsc::UnboundedSender<DelegationRequest>,
    ) -> Self {
        Self {
            agent_id,
            conversation_id,
            session_id,
            execution_id,
            event_bus,
            log_service,
            state_service,
            delegation_tx,
            batch_writer: None,
        }
    }

    /// Attach a batch writer for non-blocking DB writes.
    pub fn with_batch_writer(mut self, writer: BatchWriterHandle) -> Self {
        self.batch_writer = Some(writer);
        self
    }
}

// ============================================================================
// EVENT LOGGING
// ============================================================================

/// Log a delegation event.
pub fn log_delegation(ctx: &StreamContext, child_agent: &str, task: &str) {
    let entry = ExecutionLog::new(
        &ctx.execution_id,
        &ctx.session_id,
        &ctx.agent_id,
        LogLevel::Info,
        LogCategory::Delegation,
        format!("Delegating to {}", child_agent),
    )
    .with_metadata(serde_json::json!({
        "child_agent": child_agent,
        "task": task,
    }));

    if let Some(writer) = &ctx.batch_writer {
        writer.log(entry);
    } else {
        let _ = ctx.log_service.log(entry);
    }
}

/// Log a tool call start event.
pub fn log_tool_call(ctx: &StreamContext, tool_id: &str, tool_name: &str, args: &serde_json::Value) {
    let entry = ExecutionLog::new(
        &ctx.execution_id,
        &ctx.session_id,
        &ctx.agent_id,
        LogLevel::Info,
        LogCategory::ToolCall,
        format!("Calling tool: {}", tool_name),
    )
    .with_metadata(serde_json::json!({
        "tool_id": tool_id,
        "tool_name": tool_name,
        "args": args,
    }));

    if let Some(writer) = &ctx.batch_writer {
        writer.log(entry);
    } else {
        let _ = ctx.log_service.log(entry);
    }
}

/// Log a tool result event.
pub fn log_tool_result(
    ctx: &StreamContext,
    tool_id: &str,
    result: &str,
    error: &Option<String>,
) {
    // Tool failures are expected behavior, use Warn not Error
    let level = if error.is_some() {
        LogLevel::Warn
    } else {
        LogLevel::Info
    };

    // Truncate result for logging
    let truncated = if result.len() > 500 {
        format!("{}...", &result[..500])
    } else {
        result.to_string()
    };

    let entry = ExecutionLog::new(
        &ctx.execution_id,
        &ctx.session_id,
        &ctx.agent_id,
        level,
        LogCategory::ToolResult,
        if error.is_some() {
            "Tool returned error"
        } else {
            "Tool completed"
        },
    )
    .with_metadata(serde_json::json!({
        "tool_id": tool_id,
        "result": truncated,
        "error": error,
    }));

    if let Some(writer) = &ctx.batch_writer {
        writer.log(entry);
    } else {
        let _ = ctx.log_service.log(entry);
    }
}

/// Log an error event.
pub fn log_error(ctx: &StreamContext, error: &str) {
    let entry = ExecutionLog::new(
        &ctx.execution_id,
        &ctx.session_id,
        &ctx.agent_id,
        LogLevel::Error,
        LogCategory::Error,
        error,
    );

    if let Some(writer) = &ctx.batch_writer {
        writer.log(entry);
    } else {
        let _ = ctx.log_service.log(entry);
    }
}

// ============================================================================
// TOKEN TRACKING
// ============================================================================

/// Update token counts and emit token usage event.
pub fn handle_token_update(ctx: &StreamContext, tokens_in: u64, tokens_out: u64) {
    // Update execution token counts — via batch writer if available, else direct
    if let Some(writer) = &ctx.batch_writer {
        writer.token_update(&ctx.execution_id, tokens_in, tokens_out);
    } else if let Err(e) =
        ctx.state_service
            .update_execution_tokens(&ctx.execution_id, tokens_in, tokens_out)
    {
        tracing::warn!("Failed to update execution tokens: {}", e);
    }

    // Emit token usage event for real-time UI updates
    let event_bus = ctx.event_bus.clone();
    let sess_id = ctx.session_id.clone();
    let exec_id = ctx.execution_id.clone();
    let conv_id = ctx.conversation_id.clone();
    tokio::spawn(async move {
        event_bus
            .publish(GatewayEvent::TokenUsage {
                session_id: sess_id,
                execution_id: exec_id,
                tokens_in,
                tokens_out,
                conversation_id: Some(conv_id),
            })
            .await;
    });
}

// ============================================================================
// DELEGATION HANDLING
// ============================================================================

/// Handle a delegation event by creating the execution synchronously and sending a request.
///
/// The execution record is created immediately with status QUEUED to prevent a race
/// condition where `try_complete_session()` could mark the session COMPLETED before
/// the subagent execution exists.
pub fn handle_delegation(
    ctx: &StreamContext,
    child_agent: &str,
    task: &str,
    context: &Option<serde_json::Value>,
) {
    // Create the delegated execution immediately (status=QUEUED)
    // This ensures try_complete_session() sees it as pending
    let child_execution_id = match ctx.state_service.create_delegated_execution(
        &ctx.session_id,
        child_agent,
        &ctx.execution_id,
        DelegationType::Sequential,
        task,
    ) {
        Ok(exec) => {
            tracing::debug!(
                session_id = %ctx.session_id,
                child_execution_id = %exec.id,
                child_agent = %child_agent,
                "Created delegated execution synchronously"
            );
            exec.id
        }
        Err(e) => {
            tracing::error!(
                session_id = %ctx.session_id,
                child_agent = %child_agent,
                error = %e,
                "Failed to create delegated execution, using fallback"
            );
            // Fallback: create in-memory execution ID (handler will create record)
            AgentExecution::new_delegated(
                &ctx.session_id,
                child_agent,
                &ctx.execution_id,
                DelegationType::Sequential,
                task,
            )
            .id
        }
    };

    // Send request with pre-created execution_id
    let _ = ctx.delegation_tx.send(DelegationRequest {
        parent_agent_id: ctx.agent_id.clone(),
        session_id: ctx.session_id.clone(),
        parent_execution_id: ctx.execution_id.clone(),
        child_agent_id: child_agent.to_string(),
        child_execution_id,
        task: task.to_string(),
        context: context.clone(),
    });

    log_delegation(ctx, child_agent, task);
}

// ============================================================================
// EVENT PROCESSING
// ============================================================================

/// Process a stream event: log it, handle special cases, and return the gateway event.
///
/// Returns the gateway event (if any) and whether the response accumulator should be updated.
/// Returns `None` for the gateway event if it's an internal event that shouldn't be broadcast.
pub fn process_stream_event(
    ctx: &StreamContext,
    event: &StreamEvent,
) -> (Option<GatewayEvent>, Option<String>) {
    // Handle delegation events
    if let StreamEvent::ActionDelegate {
        agent_id: child_agent,
        task,
        context,
        ..
    } = event
    {
        handle_delegation(ctx, child_agent, task, context);
    }

    // Log based on event type
    match event {
        StreamEvent::TokenUpdate {
            tokens_in,
            tokens_out,
            ..
        } => {
            handle_token_update(ctx, *tokens_in, *tokens_out);
        }
        StreamEvent::ToolCallStart {
            tool_id,
            tool_name,
            args,
            ..
        } => {
            log_tool_call(ctx, tool_id, tool_name, args);
        }
        StreamEvent::ToolResult {
            tool_id,
            result,
            error,
            ..
        } => {
            log_tool_result(ctx, tool_id, result, error);
        }
        StreamEvent::Error { error, .. } => {
            log_error(ctx, error);
        }
        _ => {}
    }

    // Convert to gateway event (may return None for internal events)
    let gateway_event = convert_stream_event(
        event.clone(),
        &ctx.agent_id,
        &ctx.conversation_id,
        &ctx.session_id,
        &ctx.execution_id,
    );

    // Extract response content for accumulation
    // Note: Token events stream incrementally during final response (when no tool calls)
    // TurnComplete contains the final response and is used as fallback marker
    let response_delta = match &gateway_event {
        Some(GatewayEvent::Token { delta, .. }) => Some(delta.clone()),
        Some(GatewayEvent::Respond { message, .. }) => Some(format!("\n\n{}", message)),
        // TurnComplete is handled specially - marked with prefix so accumulator can detect fallback
        Some(GatewayEvent::TurnComplete { message, .. }) if !message.is_empty() => {
            Some(format!("\x00TURN_COMPLETE\x00{}", message))
        }
        _ => None,
    };

    (gateway_event, response_delta)
}

/// Broadcast a gateway event asynchronously.
pub fn broadcast_event(event_bus: Arc<EventBus>, event: GatewayEvent) {
    tokio::spawn(async move {
        event_bus.publish(event).await;
    });
}

// ============================================================================
// RESPONSE ACCUMULATOR
// ============================================================================

/// Marker prefix for TurnComplete fallback content.
const TURN_COMPLETE_MARKER: &str = "\x00TURN_COMPLETE\x00";

/// Accumulator for building the final response from stream events.
#[derive(Default)]
pub struct ResponseAccumulator {
    /// Content accumulated from Token events
    content: String,
    /// Fallback content from TurnComplete (used if no Token events received)
    turn_complete_fallback: Option<String>,
}

impl ResponseAccumulator {
    /// Create a new response accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append content to the response.
    pub fn append(&mut self, content: &str) {
        // Check for TurnComplete marker (fallback for when Token events aren't streamed)
        if let Some(message) = content.strip_prefix(TURN_COMPLETE_MARKER) {
            // Store as fallback - only used if no Token events were accumulated
            self.turn_complete_fallback = Some(message.to_string());
            return;
        }

        // Handle leading newlines for respond tool messages
        if content.starts_with("\n\n") && !self.content.is_empty() {
            self.content.push_str(content);
        } else {
            self.content.push_str(content);
        }
    }

    /// Get the accumulated response.
    ///
    /// Returns Token-accumulated content if available, otherwise falls back to
    /// TurnComplete content (for cases where agent made tool calls and Token
    /// events weren't streamed).
    pub fn into_response(self) -> String {
        let trimmed = self.content.trim();
        if !trimmed.is_empty() {
            trimmed.to_string()
        } else if let Some(fallback) = self.turn_complete_fallback {
            fallback.trim().to_string()
        } else {
            String::new()
        }
    }

    /// Check if the accumulator has any content (from tokens or fallback).
    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty() && self.turn_complete_fallback.is_none()
    }

    /// Get a reference to the current token content.
    pub fn content(&self) -> &str {
        &self.content
    }
}

// ============================================================================
// TOOL CALL ACCUMULATOR
// ============================================================================

/// Record of a single tool call during execution.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCallRecord {
    /// Unique ID for this tool call
    pub tool_id: String,
    /// Name of the tool called
    pub tool_name: String,
    /// Arguments passed to the tool
    pub args: serde_json::Value,
    /// Result returned by the tool (if completed)
    pub result: Option<String>,
    /// Error message (if tool failed)
    pub error: Option<String>,
}

/// Accumulator for tool calls during execution.
///
/// Tracks all tool calls made during a single execution turn,
/// allowing them to be persisted and loaded for context continuity.
#[derive(Default)]
pub struct ToolCallAccumulator {
    calls: Vec<ToolCallRecord>,
}

impl ToolCallAccumulator {
    /// Create a new tool call accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the start of a tool call.
    pub fn start_call(&mut self, tool_id: String, tool_name: String, args: serde_json::Value) {
        self.calls.push(ToolCallRecord {
            tool_id,
            tool_name,
            args,
            result: None,
            error: None,
        });
    }

    /// Record the completion of a tool call.
    pub fn complete_call(&mut self, tool_id: &str, result: String, error: Option<String>) {
        if let Some(call) = self.calls.iter_mut().find(|c| c.tool_id == tool_id) {
            call.result = Some(result);
            call.error = error;
        }
    }

    /// Convert accumulated tool calls to JSON for storage.
    /// Returns None if no tool calls were made.
    pub fn to_json(&self) -> Option<String> {
        if self.calls.is_empty() {
            None
        } else {
            serde_json::to_string(&self.calls).ok()
        }
    }

    /// Check if any tool calls were accumulated.
    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }

    /// Get the number of tool calls.
    pub fn len(&self) -> usize {
        self.calls.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_accumulator() {
        let mut acc = ResponseAccumulator::new();
        assert!(acc.is_empty());

        acc.append("Hello");
        assert!(!acc.is_empty());
        assert_eq!(acc.content(), "Hello");

        acc.append(" World");
        assert_eq!(acc.content(), "Hello World");

        let response = acc.into_response();
        assert_eq!(response, "Hello World");
    }

    #[test]
    fn test_response_accumulator_with_respond_tool() {
        let mut acc = ResponseAccumulator::new();
        acc.append("Initial response");
        acc.append("\n\nFrom respond tool");

        let response = acc.into_response();
        assert_eq!(response, "Initial response\n\nFrom respond tool");
    }
}
