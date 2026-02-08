//! # Delegation Spawning
//!
//! Handles spawning of delegated subagents.

use super::callback::{handle_delegation_failure, handle_delegation_success};
use super::context::{DelegationContext, DelegationRequest};
use super::registry::DelegationRegistry;
use gateway_database::{ConversationRepository, DatabaseManager};
use gateway_events::{EventBus, GatewayEvent};
use gateway_services::{AgentService, McpService, ProviderService, SkillService};
use agent_runtime::AgentExecutor;
use api_logs::LogService;
use execution_state::StateService;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::handle::ExecutionHandle;
use crate::invoke::{
    broadcast_event, collect_agents_summary, collect_skills_summary, process_stream_event,
    spawn_batch_writer, AgentLoader, ExecutorBuilder, ResponseAccumulator, StreamContext,
    WorkspaceCache,
};
use crate::lifecycle::{
    complete_execution, crash_execution, emit_delegation_completed, emit_delegation_started,
    save_messages, start_execution,
};

/// Spawn a delegated agent.
///
/// This is a standalone function that runs a delegated agent using a pre-created
/// execution record. The execution record is created synchronously by
/// `handle_delegation()` in `stream.rs` to prevent a race condition.
///
/// This function handles:
/// - Starting the pre-created execution (QUEUED → RUNNING)
/// - Loading the child agent configuration
/// - Building and running the executor
/// - Sending callbacks to the parent on completion
/// - Marking execution as CRASHED if spawn fails
pub async fn spawn_delegated_agent(
    request: &DelegationRequest,
    event_bus: Arc<EventBus>,
    agent_service: Arc<AgentService>,
    provider_service: Arc<ProviderService>,
    mcp_service: Arc<McpService>,
    skill_service: Arc<SkillService>,
    config_dir: PathBuf,
    conversation_repo: Arc<ConversationRepository>,
    handles: Arc<RwLock<HashMap<String, ExecutionHandle>>>,
    delegation_registry: Arc<DelegationRegistry>,
    delegation_tx: mpsc::UnboundedSender<DelegationRequest>,
    log_service: Arc<LogService<DatabaseManager>>,
    state_service: Arc<StateService<DatabaseManager>>,
    workspace_cache: WorkspaceCache,
) -> Result<String, String> {
    // Generate child conversation ID (legacy, for conversation_repo)
    let child_conversation_id = format!(
        "{}-sub-{}",
        request.session_id,
        uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("0")
    );

    // Use the pre-created execution_id from the request
    // The execution was created synchronously by handle_delegation() to prevent
    // a race condition where try_complete_session() could mark the session
    // COMPLETED before the subagent execution exists.
    let execution_id = request.child_execution_id.clone();
    let session_id = request.session_id.clone();

    // Start execution (QUEUED → RUNNING) and log
    start_execution(
        &state_service,
        &log_service,
        &execution_id,
        &session_id,
        &request.child_agent_id,
        Some(&request.parent_execution_id),
    );

    // Register the delegation
    let delegation_context = DelegationContext::new(
        &session_id,
        &request.parent_execution_id,
        &request.parent_agent_id,
        &child_conversation_id, // legacy conversation_id
    );
    let delegation_context = if let Some(ctx) = request.context.clone() {
        delegation_context.with_context(ctx)
    } else {
        delegation_context
    };
    delegation_registry.register(&execution_id, delegation_context);

    // Track delegation for continuation
    if let Err(e) = state_service.register_delegation(&session_id) {
        tracing::warn!("Failed to register delegation: {}", e);
    }

    // Emit delegation started event
    emit_delegation_started(
        &event_bus,
        &request.parent_agent_id,
        &session_id,
        &request.child_agent_id,
        &execution_id,
        &child_conversation_id,
        &request.task,
    )
    .await;

    // Load agent and provider using AgentLoader
    let agent_loader = AgentLoader::new(&agent_service, &provider_service, config_dir.clone());
    let (agent, provider) = match agent_loader.load(&request.child_agent_id).await {
        Ok(result) => result,
        Err(e) => {
            // Mark the pre-created execution as crashed so session can complete
            crash_spawn_failure(&state_service, &execution_id, &e);
            delegation_registry.remove(&execution_id);
            return Err(e);
        }
    };

    // Collect available agents and skills for executor state
    let available_agents = collect_agents_summary(&agent_service).await;
    let available_skills = collect_skills_summary(&skill_service).await;

    // Get tool settings
    let settings_service = gateway_services::SettingsService::new(config_dir.clone());
    let tool_settings = settings_service.get_tool_settings().unwrap_or_default();

    // Look up active ward from parent session
    let session_ward_id = state_service
        .get_session(&request.session_id)
        .ok()
        .flatten()
        .and_then(|s| s.ward_id);

    // Build executor using ExecutorBuilder
    let builder = ExecutorBuilder::new(config_dir.clone(), tool_settings)
        .with_workspace_cache(workspace_cache);
    let executor = match builder
        .build(
            &agent,
            &provider,
            &child_conversation_id,
            &request.session_id,
            available_agents,
            available_skills,
            None,
            &mcp_service,
            session_ward_id.as_deref(),
        )
        .await
    {
        Ok(e) => e,
        Err(e) => {
            // Mark the pre-created execution as crashed so session can complete
            crash_spawn_failure(&state_service, &execution_id, &e);
            delegation_registry.remove(&execution_id);
            return Err(e);
        }
    };

    // Create execution handle
    let handle = ExecutionHandle::new(20);
    let handle_clone = handle.clone();

    // Store handle
    {
        let mut handles_guard = handles.write().await;
        handles_guard.insert(child_conversation_id.clone(), handle.clone());
    }

    // Spawn the execution task
    spawn_execution_task(
        executor,
        handle_clone,
        request.clone(),
        execution_id.clone(),
        session_id,
        child_conversation_id.clone(),
        event_bus,
        conversation_repo,
        delegation_registry,
        delegation_tx,
        log_service,
        state_service,
    );

    tracing::info!(
        parent_agent = %request.parent_agent_id,
        child_agent = %request.child_agent_id,
        child_conversation = %child_conversation_id,
        "Spawned delegated subagent"
    );

    Ok(child_conversation_id)
}

/// Spawn the async execution task for the delegated agent.
fn spawn_execution_task(
    executor: AgentExecutor,
    handle: ExecutionHandle,
    request: DelegationRequest,
    execution_id: String,
    session_id: String,
    conv_id: String,
    event_bus: Arc<EventBus>,
    conversation_repo: Arc<ConversationRepository>,
    delegation_registry: Arc<DelegationRegistry>,
    delegation_tx: mpsc::UnboundedSender<DelegationRequest>,
    log_service: Arc<LogService<DatabaseManager>>,
    state_service: Arc<StateService<DatabaseManager>>,
) {
    let agent_id = request.child_agent_id.clone();
    let task_msg = request.task.clone();
    let parent_agent = request.parent_agent_id.clone();
    let parent_execution_id = request.parent_execution_id.clone();

    tokio::spawn(async move {
        // Create batch writer for non-blocking DB writes
        let batch_writer = spawn_batch_writer(state_service.clone(), log_service.clone());

        // Create stream context for event processing
        let stream_ctx = StreamContext::new(
            agent_id.clone(),
            conv_id.clone(),
            session_id.clone(),
            execution_id.clone(),
            event_bus.clone(),
            log_service.clone(),
            state_service.clone(),
            delegation_tx,
        )
        .with_batch_writer(batch_writer);

        let mut response_acc = ResponseAccumulator::new();

        let result = executor
            .execute_stream(&task_msg, &[], |event| {
                if handle.is_stop_requested() {
                    return;
                }

                handle.increment();

                // Process the event (logging, delegation, token tracking)
                let (gateway_event, response_delta) = process_stream_event(&stream_ctx, &event);

                // Accumulate response content
                if let Some(delta) = response_delta {
                    response_acc.append(&delta);
                }

                // Broadcast the gateway event (if not an internal-only event)
                if let Some(event) = gateway_event {
                    broadcast_event(stream_ctx.event_bus.clone(), event);
                }
            })
            .await;

        let accumulated_response = response_acc.into_response();

        match result {
            Ok(()) => {
                handle_execution_success(
                    &conversation_repo,
                    &state_service,
                    &log_service,
                    &event_bus,
                    &delegation_registry,
                    &execution_id,
                    &session_id,
                    &agent_id,
                    &conv_id,
                    &task_msg,
                    &accumulated_response,
                    &parent_agent,
                    &parent_execution_id,
                )
                .await;
            }
            Err(e) => {
                handle_execution_failure(
                    &conversation_repo,
                    &state_service,
                    &log_service,
                    &event_bus,
                    &delegation_registry,
                    &execution_id,
                    &session_id,
                    &agent_id,
                    &conv_id,
                    &parent_execution_id,
                    &task_msg,
                    &accumulated_response,
                    &e.to_string(),
                )
                .await;
            }
        }
    });
}

/// Handle successful execution completion.
async fn handle_execution_success(
    conversation_repo: &ConversationRepository,
    state_service: &StateService<DatabaseManager>,
    log_service: &LogService<DatabaseManager>,
    event_bus: &EventBus,
    delegation_registry: &DelegationRegistry,
    execution_id: &str,
    session_id: &str,
    agent_id: &str,
    conv_id: &str,
    task_msg: &str,
    response: &str,
    parent_agent: &str,
    parent_execution_id: &str,
) {
    // Save conversation messages (subagent tool calls tracked separately for now)
    save_messages(conversation_repo, execution_id, task_msg, response, None);

    // Complete execution and emit events
    // Delegations don't dispatch to connectors (they're internal subagent calls)
    complete_execution(
        state_service,
        log_service,
        event_bus,
        execution_id,
        session_id,
        agent_id,
        conv_id,
        Some(response.to_string()),
        None,
        None,
    )
    .await;

    // Get delegation context before removing (for callback check)
    let delegation_ctx = delegation_registry.get(execution_id);

    // Emit delegation completed with proper conversation IDs for routing
    let parent_conv_id = delegation_ctx
        .as_ref()
        .map(|ctx| ctx.parent_conversation_id.as_str());
    emit_delegation_completed(
        event_bus,
        parent_agent,
        session_id,
        agent_id,
        execution_id,
        parent_conv_id,
        Some(conv_id),
        Some(response.to_string()),
    )
    .await;

    // Check if this was the last delegation and continuation is needed
    match state_service.complete_delegation(session_id) {
        Ok(true) => {
            // Get root execution for continuation
            if let Ok(Some(root_exec)) = state_service.get_root_execution(session_id) {
                event_bus
                    .publish(GatewayEvent::SessionContinuationReady {
                        session_id: session_id.to_string(),
                        root_agent_id: root_exec.agent_id.clone(),
                        root_execution_id: root_exec.id.clone(),
                    })
                    .await;
                tracing::info!(
                    session_id = %session_id,
                    root_execution_id = %root_exec.id,
                    "All delegations complete, continuation ready"
                );
            }
        }
        Ok(false) => {} // More delegations pending
        Err(e) => tracing::warn!("Failed to complete delegation tracking: {}", e),
    }

    // Send callback message to parent if enabled
    handle_delegation_success(
        delegation_ctx.as_ref(),
        conversation_repo,
        event_bus,
        session_id,
        parent_execution_id,
        agent_id,
        conv_id,
        response,
    )
    .await;

    // Remove from delegation registry
    delegation_registry.remove(execution_id);
}

/// Mark a pre-created execution as crashed when spawn fails.
///
/// This is called when the spawn process fails early (e.g., agent not found,
/// executor build error). The execution was created with status QUEUED in
/// `handle_delegation()`, so we need to mark it CRASHED to allow the session
/// to complete properly.
fn crash_spawn_failure(
    state_service: &StateService<DatabaseManager>,
    execution_id: &str,
    error: &str,
) {
    if let Err(e) = state_service.crash_execution(execution_id, error) {
        tracing::warn!(
            execution_id = %execution_id,
            error = %e,
            "Failed to mark spawn failure as crashed"
        );
    } else {
        tracing::info!(
            execution_id = %execution_id,
            error = %error,
            "Marked failed spawn as crashed"
        );
    }
}

/// Handle execution failure.
async fn handle_execution_failure(
    conversation_repo: &ConversationRepository,
    state_service: &StateService<DatabaseManager>,
    log_service: &LogService<DatabaseManager>,
    event_bus: &EventBus,
    delegation_registry: &DelegationRegistry,
    execution_id: &str,
    session_id: &str,
    agent_id: &str,
    conv_id: &str,
    parent_execution_id: &str,
    task_msg: &str,
    response: &str,
    error: &str,
) {
    // Always save messages, even on error — prevents message loss on crash
    save_messages(conversation_repo, execution_id, task_msg, response, None);

    // Crash execution and emit events (don't crash session for subagent)
    crash_execution(
        state_service,
        log_service,
        event_bus,
        execution_id,
        session_id,
        agent_id,
        conv_id,
        error,
        false, // don't crash session for subagent
    )
    .await;

    // Send error callback to parent
    handle_delegation_failure(
        conversation_repo,
        event_bus,
        session_id,
        parent_execution_id,
        agent_id,
        conv_id,
        error,
    )
    .await;

    // Check if this was the last delegation and continuation is needed
    // (even failures count as completed delegations)
    match state_service.complete_delegation(session_id) {
        Ok(true) => {
            if let Ok(Some(root_exec)) = state_service.get_root_execution(session_id) {
                event_bus
                    .publish(GatewayEvent::SessionContinuationReady {
                        session_id: session_id.to_string(),
                        root_agent_id: root_exec.agent_id.clone(),
                        root_execution_id: root_exec.id.clone(),
                    })
                    .await;
                tracing::info!(
                    session_id = %session_id,
                    "All delegations complete (including failed), continuation ready"
                );
            }
        }
        Ok(false) => {}
        Err(e) => tracing::warn!("Failed to complete delegation tracking: {}", e),
    }

    delegation_registry.remove(execution_id);
}
