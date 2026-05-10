//! # Stream Handling
//!
//! Stream event processing and logging for agent execution.

use agent_runtime::StreamEvent;
use execution_state::{AgentExecution, DelegationType};
use gateway_events::{EventBus, GatewayEvent};
use gateway_services::skills::{SkillFrontmatter, WardSetup};
use std::path::Path;
use std::sync::Arc;

use super::super::delegation::DelegationRequest;
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

pub(crate) use super::event_logging::{log_delegation, log_error, log_tool_call, log_tool_result};

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
#[allow(clippy::too_many_arguments)]
/// the subagent execution exists.
pub fn handle_delegation(
    ctx: &StreamContext,
    child_agent: &str,
    task: &str,
    context: &Option<serde_json::Value>,
    max_iterations: Option<u32>,
    output_schema: &Option<serde_json::Value>,
    skills: &[String],
    complexity: &Option<String>,
    parallel: bool,
    child_execution_id: Option<&str>,
) {
    let delegation_type = if parallel {
        DelegationType::Parallel
    } else {
        DelegationType::Sequential
    };

    // Create the delegated execution immediately (status=QUEUED)
    // This ensures try_complete_session() sees it as pending
    let child_execution_id = if let Some(id) = child_execution_id {
        // Use the pre-generated ID from DelegateTool — keeps parent's execution_id consistent
        match ctx.state_service.create_delegated_execution_with_id(
            id,
            &ctx.session_id,
            child_agent,
            &ctx.execution_id,
            delegation_type,
            task,
        ) {
            Ok(exec) => {
                tracing::debug!(
                    session_id = %ctx.session_id,
                    child_execution_id = %exec.id,
                    child_agent = %child_agent,
                    "Created delegated execution synchronously with pre-generated id"
                );
                exec.id
            }
            Err(e) => {
                tracing::error!(
                    session_id = %ctx.session_id,
                    child_agent = %child_agent,
                    error = %e,
                    "Failed to create delegated execution with provided id, using fallback"
                );
                AgentExecution::new_delegated(
                    &ctx.session_id,
                    child_agent,
                    &ctx.execution_id,
                    delegation_type,
                    task,
                )
                .id
            }
        }
    } else {
        // Legacy path (no pre-generated ID)
        match ctx.state_service.create_delegated_execution(
            &ctx.session_id,
            child_agent,
            &ctx.execution_id,
            delegation_type,
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
                    delegation_type,
                    task,
                )
                .id
            }
        }
    };

    // Increment pending_delegations SYNCHRONOUSLY so the executor's Ok() path
    // sees it before checking has_pending_delegations(). The spawn handler will
    // NOT double-increment — register_delegation is idempotent (checks current count).
    if let Err(e) = ctx.state_service.register_delegation(&ctx.session_id) {
        tracing::warn!("Failed to register delegation synchronously: {}", e);
    }

    // Send request with pre-created execution_id
    let _ = ctx.delegation_tx.send(DelegationRequest {
        parent_agent_id: ctx.agent_id.clone(),
        session_id: ctx.session_id.clone(),
        parent_execution_id: ctx.execution_id.clone(),
        parent_conversation_id: ctx.conversation_id.clone(),
        child_agent_id: child_agent.to_string(),
        child_execution_id,
        task: task.to_string(),
        context: context.clone(),
        max_iterations,
        output_schema: output_schema.clone(),
        skills: skills.to_vec(),
        complexity: complexity.clone(),
        parallel,
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

/// Read `ward_setup` from specific skills' SKILL.md files.
///
/// Only reads skills in `skill_names` — prevents life-os dirs in coding wards, etc.
pub fn collect_ward_setups_for_skills(skills_dir: &Path, skill_names: &[String]) -> Vec<WardSetup> {
    let mut setups = Vec::new();
    for name in skill_names {
        setups.extend(collect_ward_setup_for_skill(skills_dir, name));
    }
    setups
}

/// Read `ward_setup` from a single skill's SKILL.md.
pub fn collect_ward_setup_for_skill(skills_dir: &Path, skill_name: &str) -> Vec<WardSetup> {
    let skill_md = skills_dir.join(skill_name).join("SKILL.md");
    if !skill_md.exists() {
        return vec![];
    }
    let content = match std::fs::read_to_string(&skill_md) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let yaml = match extract_yaml_frontmatter(&content) {
        Some(y) => y,
        None => return vec![],
    };
    match serde_yaml::from_str::<SkillFrontmatter>(yaml) {
        Ok(fm) => fm.ward_setup.into_iter().collect(),
        Err(_) => vec![],
    }
}

/// Extract the YAML frontmatter block from a `---`-delimited document.
///
/// Returns the trimmed content between the first pair of `---` markers,
/// or `None` if the document doesn't start with `---`.
fn extract_yaml_frontmatter(content: &str) -> Option<&str> {
    let content = content.trim_start();
    if !content.starts_with("---") {
        return None;
    }
    let after_first = &content[3..];
    let end = after_first.find("\n---")?;
    Some(after_first[..end].trim())
}

