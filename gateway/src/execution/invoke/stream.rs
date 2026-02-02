//! # Stream Handling
//!
//! Stream event processing and logging for agent execution.

use crate::database::DatabaseManager;
use crate::events::{EventBus, GatewayEvent};
use api_logs::{ExecutionLog, LogCategory, LogLevel, LogService};
use agent_runtime::StreamEvent;
use execution_state::{AgentExecution, DelegationType, StateService};
use std::sync::Arc;
use tokio::sync::mpsc;

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
        }
    }
}

// ============================================================================
// EVENT LOGGING
// ============================================================================

/// Log a delegation event.
pub fn log_delegation(ctx: &StreamContext, child_agent: &str, task: &str) {
    let _ = ctx.log_service.log(
        ExecutionLog::new(
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
        })),
    );
}

/// Log a tool call start event.
pub fn log_tool_call(ctx: &StreamContext, tool_id: &str, tool_name: &str, args: &serde_json::Value) {
    let _ = ctx.log_service.log(
        ExecutionLog::new(
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
        })),
    );
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

    let _ = ctx.log_service.log(
        ExecutionLog::new(
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
        })),
    );
}

/// Log an error event.
pub fn log_error(ctx: &StreamContext, error: &str) {
    let _ = ctx.log_service.log(ExecutionLog::new(
        &ctx.execution_id,
        &ctx.session_id,
        &ctx.agent_id,
        LogLevel::Error,
        LogCategory::Error,
        error,
    ));
}

// ============================================================================
// TOKEN TRACKING
// ============================================================================

/// Update token counts and emit token usage event.
pub fn handle_token_update(ctx: &StreamContext, tokens_in: u64, tokens_out: u64) {
    // Update execution token counts in database
    if let Err(e) =
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
/// Returns the gateway event and whether the response accumulator should be updated.
pub fn process_stream_event(
    ctx: &StreamContext,
    event: &StreamEvent,
) -> (GatewayEvent, Option<String>) {
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

    // Convert to gateway event
    let gateway_event = convert_stream_event(
        event.clone(),
        &ctx.agent_id,
        &ctx.conversation_id,
        &ctx.session_id,
    );

    // Extract response content for accumulation
    let response_delta = match &gateway_event {
        GatewayEvent::Token { delta, .. } => Some(delta.clone()),
        GatewayEvent::Respond { message, .. } => Some(format!("\n\n{}", message)),
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

/// Accumulator for building the final response from stream events.
#[derive(Default)]
pub struct ResponseAccumulator {
    content: String,
}

impl ResponseAccumulator {
    /// Create a new response accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append content to the response.
    pub fn append(&mut self, content: &str) {
        // Handle leading newlines for respond tool messages
        if content.starts_with("\n\n") && !self.content.is_empty() {
            self.content.push_str(content);
        } else {
            self.content.push_str(content);
        }
    }

    /// Get the accumulated response.
    pub fn into_response(self) -> String {
        self.content.trim().to_string()
    }

    /// Check if the accumulator is empty.
    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty()
    }

    /// Get a reference to the current content.
    pub fn content(&self) -> &str {
        &self.content
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
