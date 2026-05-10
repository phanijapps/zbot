//! # Stream Handling
//!
//! Stream event processing and logging for agent execution.

use agent_runtime::StreamEvent;
use api_logs::{ExecutionLog, LogCategory, LogLevel};
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
pub fn log_tool_call(
    ctx: &StreamContext,
    tool_id: &str,
    tool_name: &str,
    args: &serde_json::Value,
) {
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
pub fn log_tool_result(ctx: &StreamContext, tool_id: &str, result: &str, error: &Option<String>) {
    // Tool failures are expected behavior, use Warn not Error
    let level = if error.is_some() {
        LogLevel::Warn
    } else {
        LogLevel::Info
    };

    // Truncate result for logging
    let truncated = if result.len() > 500 {
        format!("{}...", zero_core::truncate_str(result, 500))
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

