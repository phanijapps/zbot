//! # Delegation Callbacks
//!
//! Callback formatting and handling for delegation completion.

use super::context::DelegationContext;
use crate::database::ConversationRepository;
use crate::events::{EventBus, GatewayEvent};
use std::sync::Arc;

// ============================================================================
// CALLBACK FORMATTING
// ============================================================================

/// Format an agent ID into a display name.
///
/// Converts kebab-case to Title Case (e.g., "research-agent" -> "Research Agent").
pub fn format_agent_display_name(agent_id: &str) -> String {
    agent_id
        .split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// Format a successful callback message as markdown.
pub fn format_callback_message(
    agent_id: &str,
    response: &str,
    conversation_id: &str,
) -> String {
    let agent_display_name = format_agent_display_name(agent_id);

    let response_content = if response.is_empty() {
        "_No response generated._".to_string()
    } else {
        // Check if response looks like JSON and format it
        if response.trim().starts_with('{') || response.trim().starts_with('[') {
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(response) {
                format!(
                    "```json\n{}\n```",
                    serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| response.to_string())
                )
            } else {
                response.to_string()
            }
        } else {
            response.to_string()
        }
    };

    format!(
        "## From {}\n\n{}\n\n---\n_Conversation: `{}`_",
        agent_display_name, response_content, conversation_id
    )
}

/// Format an error callback message as markdown.
pub fn format_error_callback_message(
    agent_id: &str,
    error: &str,
    conversation_id: &str,
) -> String {
    let agent_display_name = format_agent_display_name(agent_id);

    format!(
        "## Delegation Failed\n\n**Agent:** {}\n**Error:** {}\n\n---\n_Conversation: `{}`_",
        agent_display_name, error, conversation_id
    )
}

// ============================================================================
// CALLBACK SENDING
// ============================================================================

/// Send a callback message to the parent execution.
///
/// Returns true if the message was sent successfully.
pub async fn send_callback_to_parent(
    conversation_repo: &ConversationRepository,
    event_bus: &EventBus,
    session_id: &str,
    parent_execution_id: &str,
    message: &str,
) -> bool {
    match conversation_repo.add_message(parent_execution_id, "system", message, None, None) {
        Ok(_) => {
            // Emit event so frontend can refresh
            event_bus
                .publish(GatewayEvent::MessageAdded {
                    session_id: session_id.to_string(),
                    execution_id: parent_execution_id.to_string(),
                    role: "system".to_string(),
                    content: message.to_string(),
                    conversation_id: Some(parent_execution_id.to_string()),
                })
                .await;
            true
        }
        Err(e) => {
            tracing::error!(
                parent_execution = %parent_execution_id,
                "Failed to add callback message: {}", e
            );
            false
        }
    }
}

/// Handle successful delegation completion with callback.
pub async fn handle_delegation_success(
    delegation_ctx: Option<&DelegationContext>,
    conversation_repo: &ConversationRepository,
    event_bus: &EventBus,
    session_id: &str,
    parent_execution_id: &str,
    child_agent_id: &str,
    child_conversation_id: &str,
    response: &str,
) {
    if let Some(ctx) = delegation_ctx {
        if ctx.callback_on_complete {
            let callback_msg =
                format_callback_message(child_agent_id, response, child_conversation_id);

            if send_callback_to_parent(
                conversation_repo,
                event_bus,
                session_id,
                parent_execution_id,
                &callback_msg,
            )
            .await
            {
                tracing::info!(
                    parent_execution = %parent_execution_id,
                    child_agent = %child_agent_id,
                    "Sent callback to parent execution"
                );
            }
        }
    }
}

/// Handle delegation failure with error callback.
pub async fn handle_delegation_failure(
    conversation_repo: &ConversationRepository,
    event_bus: &EventBus,
    session_id: &str,
    parent_execution_id: &str,
    child_agent_id: &str,
    child_conversation_id: &str,
    error: &str,
) {
    let error_msg = format_error_callback_message(child_agent_id, error, child_conversation_id);

    if send_callback_to_parent(conversation_repo, event_bus, session_id, parent_execution_id, &error_msg).await
    {
        tracing::warn!(
            parent_execution = %parent_execution_id,
            child_agent = %child_agent_id,
            error = %error,
            "Sent error callback to parent execution"
        );
    }
}

/// Handle subagent completion and send callback to parent.
///
/// This is called when a delegated subagent completes its task.
/// If callback_on_complete is true, it sends a message to the parent
/// conversation with the result.
pub async fn handle_subagent_completion(
    event_bus: Arc<EventBus>,
    delegation: &DelegationContext,
    session_id: &str,
    child_execution_id: &str,
    child_agent_id: &str,
    child_conversation_id: &str,
    result: Option<String>,
) {
    // Emit delegation completed event
    event_bus
        .publish(GatewayEvent::DelegationCompleted {
            session_id: session_id.to_string(),
            parent_execution_id: delegation.parent_execution_id.clone(),
            child_execution_id: child_execution_id.to_string(),
            parent_agent_id: delegation.parent_agent_id.clone(),
            child_agent_id: child_agent_id.to_string(),
            result,
            parent_conversation_id: Some(delegation.parent_conversation_id.clone()),
            child_conversation_id: Some(child_conversation_id.to_string()),
        })
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_agent_display_name() {
        assert_eq!(format_agent_display_name("research-agent"), "Research Agent");
        assert_eq!(format_agent_display_name("code-reviewer"), "Code Reviewer");
        assert_eq!(format_agent_display_name("simple"), "Simple");
    }
}
