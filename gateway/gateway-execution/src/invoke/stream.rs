//! # Stream Handling
//!
//! Stream event processing and logging for agent execution.

use agent_runtime::StreamEvent;
use gateway_events::{EventBus, GatewayEvent};
use std::sync::Arc;

use super::super::events::convert_stream_event;

// ============================================================================
// STREAM CONTEXT
// ============================================================================

pub use super::stream_context::StreamContext;

// ============================================================================
// RESPONSE ACCUMULATOR
// ============================================================================

pub use super::response_accumulator::{ResponseAccumulator, TURN_COMPLETE_MARKER};

// ============================================================================
// EVENT LOGGING
// ============================================================================

pub(crate) use super::event_logging::{log_error, log_tool_call, log_tool_result};

// ============================================================================
// TOKEN TRACKING
// ============================================================================

pub(crate) use super::token_tracking::handle_token_update;

// ============================================================================
// DELEGATION HANDLING
// ============================================================================

pub(crate) use super::delegation_handler::handle_delegation;

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
    // Handle artifact declarations from respond actions
    if let StreamEvent::ActionRespond { ref artifacts, .. } = event {
        if !artifacts.is_empty() {
            // Fetch ward_id from the session record (persisted by WardChanged events)
            let ward_id = ctx
                .state_service
                .get_session(&ctx.session_id)
                .ok()
                .flatten()
                .and_then(|s| s.ward_id);

            crate::artifacts::process_artifact_declarations(
                artifacts,
                &ctx.session_id,
                &ctx.execution_id,
                &ctx.agent_id,
                ward_id.as_deref(),
                &ctx.vault_dir,
                &ctx.state_service,
            );
        }
    }

    // Handle delegation events
    if let StreamEvent::ActionDelegate {
        agent_id: child_agent,
        task,
        context,
        max_iterations,
        output_schema,
        skills,
        complexity,
        parallel,
        child_execution_id,
        ..
    } = event
    {
        handle_delegation(
            ctx,
            child_agent,
            task,
            context,
            *max_iterations,
            output_schema,
            skills,
            complexity,
            *parallel,
            child_execution_id.as_deref(),
        );
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
        StreamEvent::WardChanged { ward_id, .. } => {
            // Persist ward_id to session so it survives across continuations
            if let Err(e) = ctx
                .state_service
                .update_session_ward(&ctx.session_id, ward_id)
            {
                tracing::warn!("Failed to update session ward: {}", e);
            }

            // Scaffold ward structure from RECOMMENDED skills only (not all skills on disk).
            // This prevents life-os directories appearing in financial-analysis wards, etc.
            let ward_dir = ctx.vault_dir.join("wards").join(ward_id);
            if ward_dir.exists() {
                let skills_dir = ctx.vault_dir.join("skills");
                let setups = if ctx.recommended_skills.is_empty() {
                    // No intent analysis (simple approach or fallback) — use coding skill only
                    collect_ward_setup_for_skill(&skills_dir, "coding")
                } else {
                    collect_ward_setups_for_skills(&skills_dir, &ctx.recommended_skills)
                };
                if !setups.is_empty() {
                    crate::middleware::ward_scaffold::scaffold_ward(&ward_dir, ward_id, &setups);
                    tracing::info!(ward = %ward_id, skills = ?ctx.recommended_skills, "Ward scaffolded from recommended skills");
                }

                // AGENTS.md is curated manually by the agent after ward creation;
                // the runtime no longer auto-rewrites it here.
            }
        }
        StreamEvent::SessionTitleChanged { ref title, .. } => {
            // Persist title to session
            if let Err(e) = ctx
                .state_service
                .update_session_title(&ctx.session_id, title)
            {
                tracing::warn!("Failed to update session title: {}", e);
            }
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
            Some(format!("{}{}", TURN_COMPLETE_MARKER, message))
        }
        _ => None,
    };

    (gateway_event, response_delta)
}

/// Broadcast a gateway event synchronously to preserve token ordering.
///
/// Uses `publish_sync` (non-blocking `broadcast::Sender::send`) instead of
/// spawning async tasks, which would destroy insertion order between tokens.
pub fn broadcast_event(event_bus: Arc<EventBus>, event: GatewayEvent) {
    event_bus.publish_sync(event);
}

// ============================================================================
// TOOL CALL ACCUMULATOR
// ============================================================================

pub use super::tool_call_accumulator::{ToolCallAccumulator, ToolCallRecord};

// ============================================================================
// WARD SCAFFOLDING HELPERS
// ============================================================================

pub use super::ward_scaffolding::{collect_ward_setup_for_skill, collect_ward_setups_for_skills};

