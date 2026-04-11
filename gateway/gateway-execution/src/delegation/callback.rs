//! # Delegation Callbacks
//!
//! Callback formatting and handling for delegation completion.

use super::context::DelegationContext;
use gateway_database::ConversationRepository;
use gateway_events::{EventBus, GatewayEvent};
use serde_json::Value;
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

/// Extract the RESULT line from a subagent response.
/// Looks for "RESULT: APPROVED" or "RESULT: DEFECTS" near the end.
fn extract_result_line(response: &str) -> Option<&str> {
    response.lines().rev().take(20).find(|line| {
        let trimmed = line.trim();
        trimmed.starts_with("RESULT: APPROVED") || trimmed.starts_with("RESULT: DEFECTS")
    })
}

/// Extract defect lines after the RESULT: DEFECTS marker.
fn extract_defects(response: &str) -> String {
    let mut in_defects = false;
    let mut defects = Vec::new();

    for line in response.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("RESULT: DEFECTS") {
            in_defects = true;
            continue;
        }
        if in_defects && trimmed.starts_with("- ") {
            defects.push(trimmed.to_string());
        }
    }

    defects.join("\n")
}

/// Format the response body: pretty-print if JSON, otherwise return as-is.
fn format_response_content(response: &str) -> String {
    let trimmed = response.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(response) {
            return format!(
                "```json\n{}\n```",
                serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| response.to_string())
            );
        }
    }
    response.to_string()
}

/// Format a successful callback message as markdown.
pub fn format_callback_message(agent_id: &str, response: &str, conversation_id: &str) -> String {
    let agent_display_name = format_agent_display_name(agent_id);

    let response_content = if response.is_empty() {
        "_No response generated._".to_string()
    } else {
        format_response_content(response)
    };

    // Detect structured review result for fast root decision-making
    let action_hint = if let Some(result_line) = extract_result_line(response) {
        if result_line.contains("APPROVED") {
            "\n\n**Action:** This node APPROVED. Proceed to the next node in the execution plan."
                .to_string()
        } else if result_line.contains("DEFECTS") {
            let defects = extract_defects(response);
            format!(
                "\n\n**Action:** DEFECTS found. Re-delegate to coding agent with these defects:\n{}",
                defects
            )
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    format!(
        "## From {}\n\n{}{}\n\n---\n_Conversation: `{}`_\n\n\
         [Recall] Delegation completed. Consider recalling to absorb any new learnings.",
        agent_display_name, response_content, action_hint, conversation_id
    )
}

/// Format an error callback message as markdown.
pub fn format_error_callback_message(agent_id: &str, error: &str, conversation_id: &str) -> String {
    let agent_display_name = format_agent_display_name(agent_id);

    format!(
        "## Delegation Failed\n\n**Agent:** {}\n**Error:** {}\n\n---\n_Conversation: `{}`_",
        agent_display_name, error, conversation_id
    )
}

// ============================================================================
// RESPONSE VALIDATION
// ============================================================================

/// Validate a delegation response against an output schema.
///
/// Performs lenient validation:
/// - If no schema is provided, the response is returned as-is.
/// - If the response is valid JSON, it is re-serialized (normalized) and returned.
/// - If the response is not valid JSON, it is wrapped as `{ "summary": "...", "_schema_valid": false }`
///   so the parent agent always receives valid JSON without losing the child's work.
///
/// Full JSON Schema validation is future work; this currently only checks that
/// the response parses as JSON.
pub fn validate_delegation_response(response: &str, schema: &Option<Value>) -> String {
    if schema.is_none() {
        return response.to_string();
    }
    match serde_json::from_str::<Value>(response.trim()) {
        Ok(json) => {
            // Valid JSON — return normalized form
            serde_json::to_string(&json).unwrap_or_else(|_| response.to_string())
        }
        Err(_) => {
            // Not JSON — wrap as summary with validation flag
            let wrapped = serde_json::json!({
                "summary": response.trim(),
                "_schema_valid": false
            });
            serde_json::to_string(&wrapped).unwrap_or_else(|_| response.to_string())
        }
    }
}

// ============================================================================
// CALLBACK SENDING
// ============================================================================

/// Send a callback message to the parent session stream.
///
/// Returns true if the message was sent successfully.
pub async fn send_callback_to_parent(
    conversation_repo: &ConversationRepository,
    event_bus: &EventBus,
    session_id: &str,
    parent_execution_id: &str,
    message: &str,
) -> bool {
    // Write to parent session stream (new path) for continuity
    match conversation_repo.append_session_message(
        session_id,
        parent_execution_id,
        "system",
        message,
        None,
        None,
    ) {
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
                session_id = %session_id,
                "Failed to add callback message to session: {}", e
            );
            false
        }
    }
}

/// Handle successful delegation completion with callback.
///
/// When the delegation has an `output_schema`, the response is validated
/// (or wrapped) before being forwarded to the parent.
#[allow(clippy::too_many_arguments)]
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
            // Validate response against output_schema (if present)
            let validated_response = validate_delegation_response(response, &ctx.output_schema);
            let callback_msg =
                format_callback_message(child_agent_id, &validated_response, child_conversation_id);

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
                    has_output_schema = ctx.output_schema.is_some(),
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

    if send_callback_to_parent(
        conversation_repo,
        event_bus,
        session_id,
        parent_execution_id,
        &error_msg,
    )
    .await
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
        assert_eq!(
            format_agent_display_name("research-agent"),
            "Research Agent"
        );
        assert_eq!(format_agent_display_name("code-reviewer"), "Code Reviewer");
        assert_eq!(format_agent_display_name("simple"), "Simple");
    }

    #[test]
    fn test_extract_result_approved() {
        let response = "Everything looks good.\n\nRESULT: APPROVED";
        assert!(extract_result_line(response).unwrap().contains("APPROVED"));
    }

    #[test]
    fn test_extract_result_defects() {
        let response = "Found issues.\n\nRESULT: DEFECTS\n- core/fetch.py: Missing error handling (severity: medium)";
        assert!(extract_result_line(response).unwrap().contains("DEFECTS"));
    }

    #[test]
    fn test_extract_result_none() {
        let response = "Just a normal response without structured result.";
        assert!(extract_result_line(response).is_none());
    }

    #[test]
    fn test_extract_defects() {
        let response = "Review done.\n\nRESULT: DEFECTS\n- file.py: Bug (severity: high)\n- data.json: Missing field (severity: medium)";
        let defects = extract_defects(response);
        assert!(defects.contains("file.py: Bug"));
        assert!(defects.contains("data.json: Missing field"));
    }

    #[test]
    fn test_callback_with_approved() {
        let msg = format_callback_message(
            "code-agent",
            "Code looks clean.\n\nRESULT: APPROVED",
            "conv-123",
        );
        assert!(msg.contains("APPROVED"));
        assert!(msg.contains("Proceed to the next node"));
    }

    #[test]
    fn test_callback_with_defects() {
        let msg = format_callback_message(
            "data-analyst",
            "Found issues.\n\nRESULT: DEFECTS\n- output.json: Wrong values (severity: high)",
            "conv-123",
        );
        assert!(msg.contains("DEFECTS found"));
        assert!(msg.contains("Re-delegate to coding agent"));
        assert!(msg.contains("Wrong values"));
    }

    #[test]
    fn test_callback_without_result() {
        let msg = format_callback_message(
            "research-agent",
            "Here are the results of my research.",
            "conv-123",
        );
        assert!(!msg.contains("Action:"));
    }
}
