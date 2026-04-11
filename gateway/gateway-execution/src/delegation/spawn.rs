//! # Delegation Spawning
//!
//! Handles spawning of delegated subagents.

use super::callback::{handle_delegation_failure, handle_delegation_success};
use super::context::{DelegationContext, DelegationRequest};
use super::registry::DelegationRegistry;
use agent_runtime::AgentExecutor;
use api_logs::LogService;
use execution_state::StateService;
use gateway_database::{ConversationRepository, DatabaseManager};
use gateway_events::{EventBus, GatewayEvent};
use gateway_services::{AgentService, McpService, ProviderService, SharedVaultPaths, SkillService};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, OwnedSemaphorePermit, RwLock};

use agent_runtime::ChatMessage;

use crate::handle::ExecutionHandle;
use crate::invoke::{
    broadcast_event, collect_agents_summary, collect_skills_summary, detect_subagent_role,
    process_stream_event, spawn_batch_writer_with_repo, subagent_rules, AgentLoader,
    ExecutorBuilder, ResponseAccumulator, StreamContext, WorkspaceCache,
};
use crate::lifecycle::{
    complete_execution, crash_execution, emit_delegation_completed, emit_delegation_started,
    start_execution,
};
use crate::recall::MemoryRecall;

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
#[allow(clippy::too_many_arguments)]
pub async fn spawn_delegated_agent(
    request: &DelegationRequest,
    event_bus: Arc<EventBus>,
    agent_service: Arc<AgentService>,
    provider_service: Arc<ProviderService>,
    mcp_service: Arc<McpService>,
    skill_service: Arc<SkillService>,
    paths: SharedVaultPaths,
    conversation_repo: Arc<ConversationRepository>,
    handles: Arc<RwLock<HashMap<String, ExecutionHandle>>>,
    delegation_registry: Arc<DelegationRegistry>,
    delegation_tx: mpsc::UnboundedSender<DelegationRequest>,
    log_service: Arc<LogService<DatabaseManager>>,
    state_service: Arc<StateService<DatabaseManager>>,
    workspace_cache: WorkspaceCache,
    delegation_permit: Option<OwnedSemaphorePermit>,
    memory_repo: Option<Arc<gateway_database::MemoryRepository>>,
    embedding_client: Option<Arc<dyn agent_runtime::llm::embedding::EmbeddingClient>>,
    memory_recall: Option<Arc<MemoryRecall>>,
    rate_limiters: Arc<
        std::sync::RwLock<
            std::collections::HashMap<String, Arc<agent_runtime::ProviderRateLimiter>>,
        >,
    >,
) -> Result<String, String> {
    // Create a child session for subagent isolation
    let child_session =
        execution_state::Session::new_child(&request.child_agent_id, &request.session_id);
    let child_session_id = child_session.id.clone();

    if let Err(e) = state_service.create_session_from(&child_session) {
        tracing::warn!("Failed to create child session: {}", e);
    }

    // Generate child conversation ID (legacy, for event routing)
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

    // Link the pre-created execution to its child session (for smart resume)
    if let Err(e) = state_service.set_child_session_id(&execution_id, &child_session_id) {
        tracing::warn!("Failed to set child_session_id on execution: {}", e);
    }

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
    let delegation_context =
        delegation_context.with_child_conversation_id(child_conversation_id.clone());
    let delegation_context = if let Some(ctx) = request.context.clone() {
        delegation_context.with_context(ctx)
    } else {
        delegation_context
    };
    let delegation_context = if let Some(schema) = request.output_schema.clone() {
        delegation_context.with_output_schema(schema)
    } else {
        delegation_context
    };
    delegation_registry.register(&execution_id, delegation_context);

    // Note: pending_delegations is incremented synchronously in handle_delegation (stream.rs).
    // Do NOT increment again here — would double-count and break continuation.

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
    let agent_loader = AgentLoader::new(&agent_service, &provider_service, paths.clone());
    let (mut agent, provider) = match agent_loader
        .load_or_create_specialist(&request.child_agent_id)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            // Mark the pre-created execution as crashed so session can complete
            crash_spawn_failure(&state_service, &execution_id, &e);
            delegation_registry.remove(&execution_id);
            return Err(e);
        }
    };

    // Detect subagent role
    let role = detect_subagent_role(&request.child_agent_id, &request.task);
    tracing::info!(
        child_agent = %request.child_agent_id,
        role = ?role,
        "Subagent role detected"
    );

    // PREPEND rules as the FIRST thing in instructions.
    // Rules must come before agent AGENTS.md, ward context, specs — everything.
    // The agent reads rules first, then context. Rules frame all decisions.
    let rules = subagent_rules(role);
    let original_instructions = std::mem::take(&mut agent.instructions);
    agent.instructions = format!("{}\n\n{}", rules, original_instructions);

    // Skill hints (one line)
    if !request.skills.is_empty() {
        let skill_names = request.skills.join(", ");
        agent.instructions.push_str(&format!(
            "\nRecommended skills: {}. Use load_skill to load any you need.\n",
            skill_names
        ));
    }

    // Inject output contract into child agent instructions when schema is provided
    if let Some(ref schema) = request.output_schema {
        let schema_str = serde_json::to_string_pretty(schema).unwrap_or_default();
        agent.instructions.push_str(&format!(
            "\n\n## Output Contract\nYour response MUST be a JSON object matching this schema:\n```json\n{}\n```\nRespond with ONLY the JSON object. No explanation before or after the JSON.",
            schema_str
        ));
    }

    // Collect available agents and skills for executor state
    let available_agents = collect_agents_summary(&agent_service).await;
    let available_skills = collect_skills_summary(&skill_service).await;

    // Get tool settings
    let settings_service = gateway_services::SettingsService::new(paths.clone());
    let tool_settings = settings_service.get_tool_settings().unwrap_or_default();

    // Look up active ward from parent session
    let session_ward_id = state_service
        .get_session(&request.session_id)
        .ok()
        .flatten()
        .and_then(|s| s.ward_id);

    // Inject ward context so subagent starts with complete knowledge
    if let Some(ref ward_id) = session_ward_id {
        let ward_dir = paths.vault_dir().join("wards").join(ward_id);
        let agents_md_path = ward_dir.join("AGENTS.md");

        if let Ok(agents_md) = std::fs::read_to_string(&agents_md_path) {
            agent
                .instructions
                .push_str(&format!("\n# Ward Context ({})\n{}\n", ward_id, agents_md));
        }

        // Inject core module docs so subagent knows available functions
        let core_docs_path = ward_dir.join("memory-bank").join("core_docs.md");
        if let Ok(core_docs) = std::fs::read_to_string(&core_docs_path) {
            // Only inject if reasonably sized (< 4KB to avoid context bloat)
            if core_docs.len() < 4096 {
                agent
                    .instructions
                    .push_str(&format!("\n# Available Core Modules\n{}\n", core_docs));
            } else {
                agent.instructions.push_str(
                    "\n# Core Modules\nSee memory-bank/core_docs.md for available functions. Read it before writing new code.\n"
                );
            }
        }

        // List active spec PATHS (not content) — agent can cat if needed.
        // Content injection was 8-12KB per delegation — too much context bloat.
        let specs_dir = ward_dir.join("specs");
        if specs_dir.exists() {
            let mut spec_files = Vec::new();
            collect_spec_files(&specs_dir, &specs_dir, &mut spec_files);
            if !spec_files.is_empty() {
                agent.instructions.push_str("\n# Specs\n");
                for rel_path in &spec_files {
                    agent.instructions.push_str(&format!("- {}\n", rel_path));
                }
            }
        }

        tracing::info!(
            child_agent = %request.child_agent_id,
            ward_id = %ward_id,
            "Injected ward context for subagent"
        );
    }

    // Build model registry for capability lookups
    let bundled_models = gateway_templates::Templates::get("models_registry.json")
        .map(|f| f.data.to_vec())
        .unwrap_or_default();
    let model_registry = Arc::new(gateway_services::models::ModelRegistry::load(
        &bundled_models,
        paths.vault_dir(),
    ));

    // Get shared rate limiter for the child's provider
    let provider_id = provider.id.clone().unwrap_or_else(|| provider.name.clone());
    let rate_limiter = {
        let guard = rate_limiters.read().unwrap_or_else(|e| e.into_inner());
        guard.get(&provider_id).cloned()
    };

    // Build executor using ExecutorBuilder
    let mut builder = ExecutorBuilder::new(paths.vault_dir().clone(), tool_settings)
        .with_workspace_cache(workspace_cache)
        .with_model_registry(model_registry)
        .with_delegated(true);

    if let Some(limiter) = rate_limiter {
        builder = builder.with_rate_limiter(limiter);
    }

    // Build fact store for subagent (so save_fact uses DB, not file fallback)
    let fact_store: Option<Arc<dyn zero_core::MemoryFactStore>> =
        memory_repo.as_ref().map(|repo| {
            Arc::new(gateway_database::GatewayMemoryFactStore::new(
                repo.clone(),
                embedding_client.clone(),
            )) as Arc<dyn zero_core::MemoryFactStore>
        });
    if let Some(fs) = fact_store {
        builder = builder.with_fact_store(fs);
    }

    let executor = match builder
        .build(
            &agent,
            &provider,
            &child_conversation_id,
            &request.session_id,
            &available_agents,
            &available_skills,
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

    // Delegation recall: inject relevant knowledge for the child agent
    let initial_history = if let Some(recall) = &memory_recall {
        let ward_id = session_ward_id.as_deref();
        match recall
            .recall_for_delegation(&request.child_agent_id, &request.task, ward_id, 8)
            .await
        {
            Ok(context) if !context.is_empty() => {
                tracing::info!(
                    agent = %request.child_agent_id,
                    context_len = context.len(),
                    "Primed subagent with recalled memory context"
                );
                vec![ChatMessage::system(context)]
            }
            Ok(_) => Vec::new(),
            Err(e) => {
                tracing::warn!(
                    agent = %request.child_agent_id,
                    error = %e,
                    "Delegation recall failed, proceeding without priming"
                );
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    // Create execution handle
    // Complexity-based iteration budget (overrides default if complexity is set)
    let max_iter = match request.complexity.as_deref() {
        Some("S") => request.max_iterations.unwrap_or(15),
        Some("M") => request.max_iterations.unwrap_or(30),
        Some("L") => request.max_iterations.unwrap_or(50),
        Some("XL") => request.max_iterations.unwrap_or(100),
        _ => request.max_iterations.unwrap_or(1000),
    };
    let handle = ExecutionHandle::new(max_iter);
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
        child_session_id,
        child_conversation_id.clone(),
        event_bus,
        conversation_repo,
        delegation_registry,
        delegation_tx,
        log_service,
        state_service,
        paths,
        delegation_permit,
        initial_history,
    );

    tracing::info!(
        parent_agent = %request.parent_agent_id,
        child_agent = %request.child_agent_id,
        child_conversation = %child_conversation_id,
        "Spawned delegated subagent"
    );

    Ok(child_conversation_id)
}

#[allow(clippy::too_many_arguments)]
/// Spawn the async execution task for the delegated agent.
fn spawn_execution_task(
    executor: AgentExecutor,
    handle: ExecutionHandle,
    request: DelegationRequest,
    execution_id: String,
    session_id: String,
    child_session_id: String,
    conv_id: String,
    event_bus: Arc<EventBus>,
    conversation_repo: Arc<ConversationRepository>,
    delegation_registry: Arc<DelegationRegistry>,
    delegation_tx: mpsc::UnboundedSender<DelegationRequest>,
    log_service: Arc<LogService<DatabaseManager>>,
    state_service: Arc<StateService<DatabaseManager>>,
    paths: SharedVaultPaths,
    delegation_permit: Option<OwnedSemaphorePermit>,
    initial_history: Vec<ChatMessage>,
) {
    let agent_id = request.child_agent_id.clone();
    let task_msg = request.task.clone();
    let parent_agent = request.parent_agent_id.clone();
    let parent_execution_id = request.parent_execution_id.clone();

    tokio::spawn(async move {
        // Hold the delegation permit for the duration of the task.
        // When this task completes (or is dropped), the permit is released,
        // allowing another queued delegation to proceed.
        let _delegation_permit = delegation_permit;

        // Create batch writer with conversation repo for session message streaming
        let batch_writer = spawn_batch_writer_with_repo(
            state_service.clone(),
            log_service.clone(),
            Some(conversation_repo.clone()),
        );

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
            paths.vault_dir().clone(),
        )
        .with_batch_writer(batch_writer.clone());

        let mut response_acc = ResponseAccumulator::new();

        // Append task message to child session stream
        batch_writer.session_message(
            &child_session_id,
            &execution_id,
            "user",
            &task_msg,
            None,
            None,
        );

        let child_session_id_inner = child_session_id.clone();
        let execution_id_inner = execution_id.clone();
        let batch_writer_inner = batch_writer.clone();
        let mut turn_tool_calls: Vec<serde_json::Value> = Vec::new();
        let mut turn_text = String::new();

        let result = executor
            .execute_stream(&task_msg, &initial_history, |event| {
                if handle.is_stop_requested() {
                    return;
                }

                handle.increment();

                // Stream messages to child session
                match &event {
                    agent_runtime::StreamEvent::ToolCallStart {
                        tool_id,
                        tool_name,
                        args,
                        ..
                    } => {
                        turn_tool_calls.push(serde_json::json!({
                            "tool_id": tool_id,
                            "tool_name": tool_name,
                            "args": args,
                        }));
                    }
                    agent_runtime::StreamEvent::ToolResult {
                        tool_id,
                        result,
                        error,
                        ..
                    } => {
                        if !turn_tool_calls.is_empty() {
                            let tc_json =
                                serde_json::to_string(&turn_tool_calls).unwrap_or_default();
                            let content = if turn_text.is_empty() {
                                "[tool calls]".to_string()
                            } else {
                                std::mem::take(&mut turn_text)
                            };
                            batch_writer_inner.session_message(
                                &child_session_id_inner,
                                &execution_id_inner,
                                "assistant",
                                &content,
                                Some(&tc_json),
                                None,
                            );
                            turn_tool_calls.clear();
                        }

                        let tool_content = if let Some(err) = error {
                            format!("Error: {}", err)
                        } else {
                            result.clone()
                        };
                        batch_writer_inner.session_message(
                            &child_session_id_inner,
                            &execution_id_inner,
                            "tool",
                            &tool_content,
                            None,
                            Some(tool_id),
                        );
                    }
                    agent_runtime::StreamEvent::Token { content, .. } => {
                        turn_text.push_str(content);
                    }
                    _ => {}
                }

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

        // Emit final assistant response to child session stream
        if !accumulated_response.is_empty() {
            batch_writer.session_message(
                &child_session_id,
                &execution_id,
                "assistant",
                &accumulated_response,
                None,
                None,
            );
        }

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
                    &accumulated_response,
                    &parent_agent,
                    &parent_execution_id,
                )
                .await;
            }
            Err(e) => {
                // Build structured crash report with plan status and ward files
                let crash_report = build_crash_report(
                    &agent_id,
                    &e.to_string(),
                    &conversation_repo,
                    &child_session_id,
                    &state_service,
                    &session_id,
                    &paths,
                );

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
                    &crash_report,
                )
                .await;
            }
        }

        // Mark child session as completed (prevents orphaned "running" sessions)
        if let Err(e) = state_service.complete_session(&child_session_id) {
            tracing::warn!(child_session_id = %child_session_id, "Failed to complete child session: {}", e);
        }
    });
}

/// Handle successful execution completion.
#[allow(clippy::too_many_arguments)]
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
    response: &str,
    parent_agent: &str,
    parent_execution_id: &str,
) {
    // Messages already streamed to child session during execution

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
        None, // No thread_id for delegations (internal subagent calls)
        None, // No bridge dispatch for delegations
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
#[allow(clippy::too_many_arguments)]
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
    error: &str,
) {
    // Messages already streamed to child session during execution

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

/// Build a structured crash report with plan status and ward file listing.
///
/// When a subagent fails, this provides the parent agent with actionable
/// intelligence about what was accomplished before the crash, enabling
/// better retry strategies.
fn build_crash_report(
    agent_id: &str,
    error: &str,
    conversation_repo: &ConversationRepository,
    child_session_id: &str,
    state_service: &StateService<DatabaseManager>,
    parent_session_id: &str,
    paths: &SharedVaultPaths,
) -> String {
    let mut report = format!("DELEGATION FAILED: {}\n\nERROR: {}\n", agent_id, error);

    // Try to extract plan status from child session messages.
    // Plan updates appear as tool results containing JSON with `__plan_update: true`.
    let mut found_plan = false;
    if let Ok(messages) = conversation_repo.get_session_conversation(child_session_id, 200) {
        // Scan tool-result messages for plan updates (last one is most recent)
        let plan_messages: Vec<_> = messages
            .iter()
            .filter(|m| m.content.contains("__plan_update"))
            .collect();

        if let Some(last_plan_msg) = plan_messages.last() {
            if let Ok(plan_data) = serde_json::from_str::<serde_json::Value>(&last_plan_msg.content)
            {
                if let Some(steps) = plan_data.get("plan").and_then(|p| p.as_array()) {
                    let completed: Vec<_> = steps
                        .iter()
                        .filter(|s| s.get("status").and_then(|v| v.as_str()) == Some("completed"))
                        .filter_map(|s| s.get("step").and_then(|v| v.as_str()))
                        .collect();
                    let pending: Vec<_> = steps
                        .iter()
                        .filter(|s| s.get("status").and_then(|v| v.as_str()) != Some("completed"))
                        .filter_map(|s| s.get("step").and_then(|v| v.as_str()))
                        .collect();

                    found_plan = true;
                    if !completed.is_empty() {
                        report.push_str("\nCOMPLETED STEPS:\n");
                        for s in &completed {
                            report.push_str(&format!("  [done] {}\n", s));
                        }
                    }
                    if !pending.is_empty() {
                        report.push_str("\nREMAINING STEPS:\n");
                        for s in &pending {
                            report.push_str(&format!("  [todo] {}\n", s));
                        }
                    }
                }
            }
        }
    }

    if !found_plan {
        report.push_str("\nPARTIAL WORK COMPLETED:\nNo plan was created\n");
    }

    // Check ralph.py tasks.json status if available in the ward
    if let Ok(Some(session)) = state_service.get_session(parent_session_id) {
        if let Some(ward_id) = &session.ward_id {
            let ward_dir = paths.ward_dir(ward_id);
            let ralph = ward_dir.join("ralph.py");
            if ralph.exists() {
                // Find any tasks.json files in specs/
                let specs_dir = ward_dir.join("specs");
                if specs_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&specs_dir) {
                        for entry in entries.flatten() {
                            let tasks_json = entry.path().join("tasks.json");
                            if tasks_json.exists() {
                                // Run ralph.py status to get task completion state
                                if let Ok(output) = std::process::Command::new("python3")
                                    .arg(&ralph)
                                    .arg("status")
                                    .arg(&tasks_json)
                                    .current_dir(&ward_dir)
                                    .output()
                                {
                                    let status = String::from_utf8_lossy(&output.stdout);
                                    if !status.trim().is_empty() {
                                        let rel_path = tasks_json
                                            .strip_prefix(&ward_dir)
                                            .map(|p| p.display().to_string())
                                            .unwrap_or_else(|_| tasks_json.display().to_string());
                                        report.push_str(&format!(
                                            "\nTASK RUNNER STATUS ({}):\n  {}\n\
                                             \nTO RESUME: Re-delegate with \"Continue processing {}\" — ralph.py tracks completion state.\n",
                                            rel_path, status.trim(), rel_path
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // List files in the ward (if one is active for this session)
    if let Ok(Some(session)) = state_service.get_session(parent_session_id) {
        if let Some(ward_id) = &session.ward_id {
            let ward_dir = paths.ward_dir(ward_id);
            if ward_dir.exists() {
                if let Ok(entries) = walkdir_simple(&ward_dir) {
                    if !entries.is_empty() {
                        report.push_str("\nFILES IN WARD:\n");
                        for entry in entries.iter().take(20) {
                            report.push_str(&format!("  {}\n", entry));
                        }
                        if entries.len() > 20 {
                            report.push_str(&format!(
                                "  ... and {} more files\n",
                                entries.len() - 20
                            ));
                        }
                    }
                }
            }
        }
    }

    report.push_str(
        "\nSUGGESTION: Break remaining work into smaller, focused tasks. \
         Existing files can be reused.\n",
    );
    report
}

/// Recursively collect .md spec file paths relative to specs_root.
fn collect_spec_files(dir: &std::path::Path, specs_root: &std::path::Path, out: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip archive directory
                if path.file_name().map(|n| n == "archive").unwrap_or(false) {
                    continue;
                }
                collect_spec_files(&path, specs_root, out);
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                if let Ok(rel) = path.strip_prefix(specs_root) {
                    out.push(format!("specs/{}", rel.display()));
                }
            }
        }
    }
}

/// Simple recursive directory listing that skips hidden files and common noise.
fn walkdir_simple(dir: &Path) -> std::io::Result<Vec<String>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = path
            .strip_prefix(dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        if name.starts_with('.') || name.contains("__pycache__") {
            continue;
        }
        if path.is_file() {
            files.push(name);
        } else if path.is_dir() {
            if let Ok(sub_files) = walkdir_simple(&path) {
                for sf in sub_files {
                    files.push(format!("{}/{}", name, sf));
                }
            }
        }
    }
    files.sort();
    Ok(files)
}
