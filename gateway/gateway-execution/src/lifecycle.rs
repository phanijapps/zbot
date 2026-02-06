//! # Lifecycle Module
//!
//! Manages session and execution lifecycle transitions.
//!
//! This module centralizes all state transitions for sessions and executions,
//! including creation, completion, error handling, and cancellation.

use gateway_connectors::{ConnectorRegistry, DispatchContext};
use gateway_database::{ConversationRepository, DatabaseManager};
use gateway_events::{EventBus, GatewayEvent};
use api_logs::{LogService, SessionStatus};
use execution_state::{AgentExecution, Session, StateService};
use std::sync::Arc;

// ============================================================================
// SESSION CREATION
// ============================================================================

/// Result of session/execution setup.
pub struct SessionSetup {
    /// Session ID
    pub session_id: String,
    /// Execution ID
    pub execution_id: String,
}

/// Get or create a session and execution for an agent invocation.
///
/// If `existing_session_id` is provided and the session exists, creates a new
/// execution within that session. Otherwise, creates a new session and execution.
///
/// If the existing session was in a terminal state (completed/crashed), it will
/// be reactivated to running status.
pub fn get_or_create_session(
    state_service: &StateService<DatabaseManager>,
    agent_id: &str,
    existing_session_id: Option<&str>,
) -> SessionSetup {
    if let Some(session_id) = existing_session_id {
        // Try to continue existing session
        match state_service.get_session(session_id) {
            Ok(Some(_session)) => {
                // Session exists, create a new execution for this message
                let execution = AgentExecution::new_root(session_id, agent_id);
                if let Err(e) = state_service.create_execution(&execution) {
                    tracing::warn!("Failed to create execution in existing session: {}", e);
                }

                // Reactivate session if it was in a terminal state (completed/crashed)
                // This handles the case where user sends a new message to a completed session
                if let Err(e) = state_service.reactivate_session(session_id) {
                    tracing::warn!("Failed to reactivate session: {}", e);
                }

                return SessionSetup {
                    session_id: session_id.to_string(),
                    execution_id: execution.id,
                };
            }
            Ok(None) => {
                tracing::warn!("Session {} not found, creating new session", session_id);
            }
            Err(e) => {
                tracing::warn!("Failed to get session: {}", e);
            }
        }
    }

    // Create new session
    let (session, execution) = state_service
        .create_session(agent_id)
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to create session: {}", e);
            let s = Session::new(agent_id);
            let e = AgentExecution::new_root(&s.id, agent_id);
            (s, e)
        });

    SessionSetup {
        session_id: session.id,
        execution_id: execution.id,
    }
}

/// Start an execution and log the start event.
pub fn start_execution(
    state_service: &StateService<DatabaseManager>,
    log_service: &LogService<DatabaseManager>,
    execution_id: &str,
    session_id: &str,
    agent_id: &str,
    parent_execution_id: Option<&str>,
) {
    // Transition execution state: QUEUED -> RUNNING
    if let Err(e) = state_service.start_execution(execution_id) {
        tracing::warn!("Failed to start execution: {}", e);
    }

    // Log the execution start
    if let Err(e) = log_service.log_session_start(
        execution_id,
        session_id,
        agent_id,
        parent_execution_id,
    ) {
        tracing::warn!("Failed to log execution start: {}", e);
    }

    tracing::info!(
        session_id = %session_id,
        execution_id = %execution_id,
        agent_id = %agent_id,
        "Execution started"
    );
}

// ============================================================================
// MESSAGE PERSISTENCE
// ============================================================================

/// Save conversation messages (user input and assistant response).
///
/// # Arguments
/// * `conversation_repo` - Repository for message storage
/// * `execution_id` - ID of the current execution
/// * `user_message` - The user's input message
/// * `assistant_response` - The assistant's response text
/// * `tool_calls_json` - Optional JSON string of tool calls made during this turn
pub fn save_messages(
    conversation_repo: &ConversationRepository,
    execution_id: &str,
    user_message: &str,
    assistant_response: &str,
    tool_calls_json: Option<&str>,
) {
    // Save user message
    if let Err(e) = conversation_repo.add_message(execution_id, "user", user_message, None, None) {
        tracing::error!("Failed to save user message: {}", e);
    }

    // Save assistant response if there's content OR tool calls
    // (agent may have made tool calls without text output)
    let has_content = !assistant_response.is_empty();
    let has_tool_calls = tool_calls_json.is_some();

    tracing::debug!(
        execution_id = %execution_id,
        response_len = assistant_response.len(),
        has_tool_calls = has_tool_calls,
        "Saving assistant message"
    );

    if has_content || has_tool_calls {
        // If no text content but has tool calls, save a placeholder
        let content = if has_content {
            assistant_response
        } else {
            "[Tool calls only - see tool_calls field]"
        };

        if let Err(e) = conversation_repo.add_message(
            execution_id,
            "assistant",
            content,
            tool_calls_json,
            None,
        ) {
            tracing::error!("Failed to save assistant message: {}", e);
        }
    }
}

// ============================================================================
// COMPLETION HANDLING
// ============================================================================

/// Handle successful execution completion.
///
/// Updates state, logs the completion, emits events, and dispatches to connectors.
///
/// For root executions with pending delegations, this will request a continuation
/// turn to be spawned after all delegations complete.
///
/// If `respond_to` contains connector IDs and `connector_registry` is provided,
/// the response will be dispatched to those connectors.
pub async fn complete_execution(
    state_service: &StateService<DatabaseManager>,
    log_service: &LogService<DatabaseManager>,
    event_bus: &EventBus,
    execution_id: &str,
    session_id: &str,
    agent_id: &str,
    conversation_id: &str,
    response: Option<String>,
    connector_registry: Option<&Arc<ConnectorRegistry>>,
    respond_to: Option<&Vec<String>>,
) {
    // Update execution status to COMPLETED
    if let Err(e) = state_service.complete_execution(execution_id) {
        tracing::warn!("Failed to complete execution: {}", e);
    }

    // Eagerly aggregate session tokens so UI shows real values
    // (web sessions never auto-complete, so without this they'd stay at 0)
    if let Err(e) = state_service.aggregate_session_tokens(session_id) {
        tracing::warn!("Failed to aggregate session tokens: {}", e);
    }

    // Check if this is a root execution and has pending delegations
    // If so, request continuation after delegations complete
    if let Ok(Some(execution)) = state_service.get_execution(execution_id) {
        if execution.is_root() {
            if let Ok(Some(session)) = state_service.get_session(session_id) {
                if session.has_pending_delegations() {
                    // Root execution completed while delegations are pending
                    // Request continuation to process results when they arrive
                    if let Err(e) = state_service.request_continuation(session_id) {
                        tracing::warn!("Failed to request continuation: {}", e);
                    } else {
                        tracing::info!(
                            session_id = %session_id,
                            pending = session.pending_delegations,
                            "Root execution complete, continuation requested for pending delegations"
                        );
                    }
                }
            }
        }
    }

    // Try to complete session if no other executions are running
    match state_service.try_complete_session(session_id) {
        Ok(true) => tracing::debug!("Session completed"),
        Ok(false) => tracing::debug!("Session still has running executions"),
        Err(e) => tracing::warn!("Failed to check/complete session: {}", e),
    }

    // Log session end
    let _ = log_service.log_session_end(
        execution_id,
        session_id,
        agent_id,
        SessionStatus::Completed,
        Some("Execution completed successfully"),
    );

    // Emit completion event
    event_bus
        .publish(GatewayEvent::AgentCompleted {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            result: response.clone(),
            conversation_id: Some(conversation_id.to_string()),
        })
        .await;

    // Dispatch response to connectors if respond_to is specified
    if let (Some(registry), Some(connector_ids)) = (connector_registry, respond_to) {
        if !connector_ids.is_empty() {
            if let Some(response_text) = &response {
                let context = DispatchContext {
                    session_id: session_id.to_string(),
                    thread_id: None,
                    agent_id: agent_id.to_string(),
                    timestamp: chrono::Utc::now(),
                };

                let payload = serde_json::json!({
                    "message": response_text,
                    "execution_id": execution_id,
                    "conversation_id": conversation_id,
                });

                tracing::info!(
                    session_id = %session_id,
                    connectors = ?connector_ids,
                    "Dispatching response to connectors"
                );

                let results = registry
                    .dispatch_to_many(connector_ids, "respond", payload, &context)
                    .await;

                for (connector_id, result) in results {
                    match result {
                        Ok(resp) => {
                            tracing::info!(
                                connector_id = %connector_id,
                                success = resp.success,
                                "Connector dispatch completed"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                connector_id = %connector_id,
                                error = %e,
                                "Connector dispatch failed"
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Handle execution error/crash.
///
/// Updates state, logs the error, and emits events.
pub async fn crash_execution(
    state_service: &StateService<DatabaseManager>,
    log_service: &LogService<DatabaseManager>,
    event_bus: &EventBus,
    execution_id: &str,
    session_id: &str,
    agent_id: &str,
    conversation_id: &str,
    error: &str,
    crash_session: bool,
) {
    // Update execution status to CRASHED
    if let Err(e) = state_service.crash_execution(execution_id, error) {
        tracing::warn!("Failed to crash execution: {}", e);
    }

    // Optionally crash the session too (for root executions)
    if crash_session {
        // crash_session() already aggregates tokens
        if let Err(e) = state_service.crash_session(session_id) {
            tracing::warn!("Failed to crash session: {}", e);
        }
    } else {
        // Subagent crash: still aggregate tokens for UI visibility
        if let Err(e) = state_service.aggregate_session_tokens(session_id) {
            tracing::warn!("Failed to aggregate session tokens: {}", e);
        }
    }

    // Log session error
    let _ = log_service.log_session_end(
        execution_id,
        session_id,
        agent_id,
        SessionStatus::Error,
        Some(error),
    );

    // Emit error event
    event_bus
        .publish(GatewayEvent::Error {
            agent_id: Some(agent_id.to_string()),
            session_id: Some(session_id.to_string()),
            execution_id: Some(execution_id.to_string()),
            message: error.to_string(),
            conversation_id: Some(conversation_id.to_string()),
        })
        .await;
}

/// Handle user-initiated stop.
///
/// Updates state, logs the stop, and emits events.
pub async fn stop_execution(
    state_service: &StateService<DatabaseManager>,
    log_service: &LogService<DatabaseManager>,
    event_bus: &EventBus,
    execution_id: &str,
    session_id: &str,
    agent_id: &str,
    conversation_id: &str,
    iteration: u32,
) {
    // Update session status to CANCELLED
    if let Err(e) = state_service.cancel_session(session_id) {
        tracing::warn!("Failed to cancel session: {}", e);
    }

    // Log session stopped
    let _ = log_service.log_session_end(
        execution_id,
        session_id,
        agent_id,
        SessionStatus::Stopped,
        Some("Stopped by user"),
    );

    // Emit stopped event
    event_bus
        .publish(GatewayEvent::AgentStopped {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            iteration,
            conversation_id: Some(conversation_id.to_string()),
        })
        .await;
}

// ============================================================================
// EVENT EMISSION HELPERS
// ============================================================================

/// Emit agent started event.
pub async fn emit_agent_started(
    event_bus: &EventBus,
    agent_id: &str,
    conversation_id: &str,
    session_id: &str,
    execution_id: &str,
) {
    event_bus
        .publish(GatewayEvent::AgentStarted {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            conversation_id: Some(conversation_id.to_string()),
        })
        .await;
}

/// Emit delegation started event.
///
/// Includes child_conversation_id for frontend tracking (used as key for subagent activities).
pub async fn emit_delegation_started(
    event_bus: &EventBus,
    parent_agent_id: &str,
    session_id: &str,
    child_agent_id: &str,
    child_execution_id: &str,
    child_conversation_id: &str,
    task: &str,
) {
    event_bus
        .publish(GatewayEvent::DelegationStarted {
            session_id: session_id.to_string(),
            parent_execution_id: session_id.to_string(), // Will be updated by caller
            child_execution_id: child_execution_id.to_string(),
            parent_agent_id: parent_agent_id.to_string(),
            child_agent_id: child_agent_id.to_string(),
            task: task.to_string(),
            parent_conversation_id: None,
            child_conversation_id: Some(child_conversation_id.to_string()),
        })
        .await;
}

/// Emit delegation completed event.
pub async fn emit_delegation_completed(
    event_bus: &EventBus,
    parent_agent_id: &str,
    session_id: &str,
    child_agent_id: &str,
    child_execution_id: &str,
    parent_conversation_id: Option<&str>,
    child_conversation_id: Option<&str>,
    result: Option<String>,
) {
    event_bus
        .publish(GatewayEvent::DelegationCompleted {
            session_id: session_id.to_string(),
            parent_execution_id: session_id.to_string(), // Will be updated by caller
            child_execution_id: child_execution_id.to_string(),
            parent_agent_id: parent_agent_id.to_string(),
            child_agent_id: child_agent_id.to_string(),
            result,
            parent_conversation_id: parent_conversation_id.map(|s| s.to_string()),
            child_conversation_id: child_conversation_id.map(|s| s.to_string()),
        })
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_setup_struct() {
        let setup = SessionSetup {
            session_id: "session-123".to_string(),
            execution_id: "exec-456".to_string(),
        };
        assert_eq!(setup.session_id, "session-123");
        assert_eq!(setup.execution_id, "exec-456");
    }
}
