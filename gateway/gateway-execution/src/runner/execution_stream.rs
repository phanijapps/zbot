//! # ExecutionStream
//!
//! Per-execution event loop. Consumes an `AgentExecutor` stream,
//! accumulates tool calls, drives lifecycle transitions, and fires
//! post-execution background tasks (distillation, ward indexing).
//!
//! Field list = dependency contract.

use std::collections::HashMap;
use std::sync::Arc;

use agent_runtime::{AgentExecutor, ChatMessage};
use api_logs::LogService;
use execution_state::StateService;
use gateway_database::{ConversationRepository, DatabaseManager};
use gateway_events::EventBus;
use gateway_services::SharedVaultPaths;
use tokio::sync::{mpsc, RwLock};

use crate::delegation::{DelegationRegistry, DelegationRequest};
use crate::handle::ExecutionHandle;
use crate::invoke::micro_recall::MicroRecallContext;
use crate::invoke::working_memory_middleware;
use crate::invoke::{
    broadcast_event, process_stream_event, spawn_batch_writer_with_repo, BatchWriterHandle,
    ResponseAccumulator, StreamContext, ToolCallAccumulator, WorkingMemory,
};
use crate::lifecycle::{
    complete_execution, crash_execution, stop_execution, CompleteExecution, CrashExecution,
    StopExecution,
};

// ============================================================================
// STRUCT
// ============================================================================

/// Per-execution event loop handler.
///
/// Constructed by the caller in `invoke_with_callback`, wrapped in a
/// `tokio::spawn`, and consumed by a single call to [`ExecutionStream::run`].
/// Not long-lived — each invocation creates a fresh instance.
pub struct ExecutionStream {
    pub event_bus: Arc<EventBus>,
    pub state_service: Arc<StateService<DatabaseManager>>,
    pub log_service: Arc<LogService<DatabaseManager>>,
    pub conversation_repo: Arc<ConversationRepository>,
    pub delegation_tx: mpsc::UnboundedSender<DelegationRequest>,
    pub delegation_registry: Arc<DelegationRegistry>,
    pub handles: Arc<RwLock<HashMap<String, ExecutionHandle>>>,
    pub distiller: Option<Arc<crate::distillation::SessionDistiller>>,
    pub kg_episode_repo: Option<Arc<gateway_database::KgEpisodeRepository>>,
    pub graph_storage: Option<Arc<knowledge_graph::GraphStorage>>,
    pub paths: SharedVaultPaths,
    pub memory_repo: Option<Arc<gateway_database::MemoryRepository>>,
    pub connector_registry: Option<Arc<gateway_connectors::ConnectorRegistry>>,
    pub bridge_registry: Option<Arc<gateway_bridge::BridgeRegistry>>,
    pub bridge_outbox: Option<Arc<gateway_bridge::OutboxRepository>>,
}

/// Per-execution identifiers, handle, and message payload.
/// Constructed by callers as part of session setup and passed verbatim to
/// [`ExecutionStream::run`].
pub struct ExecutionContext {
    pub execution_id: String,
    pub session_id: String,
    pub agent_id: String,
    pub conversation_id: String,
    pub handle: ExecutionHandle,
    pub respond_to: Option<Vec<String>>,
    pub thread_id: Option<String>,
    pub message: String,
    pub history: Vec<ChatMessage>,
    pub recommended_skills: Vec<String>,
}

// ============================================================================
// EVENT ACCUMULATOR (stream-local mutable state)
// ============================================================================

/// Per-turn mutable state for the stream event loop.
/// Kept in one struct so event handlers take `&mut EventAccumulator`
/// instead of 10 parameters.
struct EventAccumulator {
    tool_acc: ToolCallAccumulator,
    turn_tool_calls: Vec<serde_json::Value>,
    turn_text: String,
    working_memory: WorkingMemory,
    pending_recall_triggers: Vec<(crate::invoke::micro_recall::MicroRecallTrigger, u32)>,
    current_tool_name: String,
}

/// Borrowed dependencies the stream-event handlers need to observe but not
/// mutate. Constructed once per spawn, passed by reference into each handler.
struct EventHandlerDeps<'a> {
    batch_writer: &'a BatchWriterHandle,
    session_id: &'a str,
    execution_id: &'a str,
    agent_id: &'a str,
    handle: &'a ExecutionHandle,
    kg_episode_repo: Option<&'a Arc<gateway_database::KgEpisodeRepository>>,
    graph_storage: Option<&'a Arc<knowledge_graph::GraphStorage>>,
}

/// Handle a `StreamEvent::ToolCallStart` — record the call, update the
/// current tool name, and append to the per-turn tool-call list.
fn handle_tool_call_start(
    acc: &mut EventAccumulator,
    tool_id: &str,
    tool_name: &str,
    args: &serde_json::Value,
) {
    acc.tool_acc
        .start_call(tool_id.to_string(), tool_name.to_string(), args.clone());
    acc.current_tool_name = tool_name.to_string();
    acc.turn_tool_calls.push(serde_json::json!({
        "tool_id": tool_id,
        "tool_name": tool_name,
        "args": args,
    }));
}

/// Handle a `StreamEvent::ToolResult` — flush the pending assistant turn,
/// emit the tool message, update working memory, fire-and-forget graph
/// extraction, and collect micro-recall triggers for post-stream execution.
fn handle_tool_result(
    acc: &mut EventAccumulator,
    deps: &EventHandlerDeps<'_>,
    tool_id: &str,
    result: &str,
    error: Option<&str>,
) {
    acc.tool_acc
        .complete_call(tool_id, result.to_string(), error.map(String::from));

    // Emit the assistant message for this turn (with accumulated tool_calls)
    if !acc.turn_tool_calls.is_empty() {
        let tc_json = serde_json::to_string(&acc.turn_tool_calls).unwrap_or_default();
        let content = if acc.turn_text.is_empty() {
            "[tool calls]".to_string()
        } else {
            std::mem::take(&mut acc.turn_text)
        };
        deps.batch_writer.session_message(
            deps.session_id,
            deps.execution_id,
            "assistant",
            &content,
            Some(&tc_json),
            None,
        );
        acc.turn_tool_calls.clear();
    }

    // Emit tool result message
    let tool_content = match error {
        Some(err) => format!("Error: {}", err),
        None => result.to_string(),
    };
    deps.batch_writer.session_message(
        deps.session_id,
        deps.execution_id,
        "tool",
        &tool_content,
        None,
        Some(tool_id),
    );

    // Update working memory from tool result
    working_memory_middleware::process_tool_result(
        &mut acc.working_memory,
        &acc.current_tool_name,
        result,
        error,
        deps.handle.current_iteration(),
    );

    // Phase 6d: real-time graph extraction from tool output.
    // Non-blocking — fires in a background task so the execution
    // loop never waits.
    if let (Some(ep_repo), Some(graph)) = (deps.kg_episode_repo, deps.graph_storage) {
        let tool_name_cl = acc.current_tool_name.clone();
        let tool_id_cl = tool_id.to_string();
        let result_cl = result.to_string();
        let session_id_cl = deps.session_id.to_string();
        let agent_id_cl = deps.agent_id.to_string();
        let ep_repo_cl = ep_repo.clone();
        let graph_cl = graph.clone();
        tokio::spawn(async move {
            crate::tool_result_extractor::extract_and_persist(
                &tool_name_cl,
                &tool_id_cl,
                &result_cl,
                &session_id_cl,
                &agent_id_cl,
                ep_repo_cl.as_ref(),
                &graph_cl,
            )
            .await;
        });
    }

    // Detect micro-recall triggers (sync) — executed after stream completes
    let triggers = working_memory_middleware::detect_recall_triggers(
        &acc.working_memory,
        &acc.current_tool_name,
        result,
        error,
    );

    let iter = deps.handle.current_iteration();
    for trigger in triggers {
        acc.pending_recall_triggers.push((trigger, iter));
    }
}

// ============================================================================
// IMPL
// ============================================================================

impl ExecutionStream {
    /// Per-execution entry point. Body is the verbatim contents of the old
    /// `ExecutionRunner::spawn_execution_task` (the inside of the
    /// `tokio::spawn(async move { … })` block), with `self.<field>`
    /// replacing every captured-runner-field access and `ctx.<field>`
    /// replacing every `args.<field>` access.
    pub async fn run(&self, ctx: ExecutionContext, executor: AgentExecutor) -> Result<(), String> {
        let ExecutionContext {
            execution_id,
            session_id,
            agent_id,
            conversation_id,
            handle,
            respond_to,
            thread_id,
            message,
            mut history,
            recommended_skills,
        } = ctx;

        // Create batch writer for non-blocking DB writes (with conversation repo for session messages)
        let batch_writer = spawn_batch_writer_with_repo(
            self.state_service.clone(),
            self.log_service.clone(),
            Some(self.conversation_repo.clone()),
        );

        // Create stream context for event processing
        let stream_ctx = StreamContext::new(
            agent_id.clone(),
            conversation_id.clone(),
            session_id.clone(),
            execution_id.clone(),
            self.event_bus.clone(),
            self.log_service.clone(),
            self.state_service.clone(),
            self.delegation_tx.clone(),
            self.paths.vault_dir().clone(),
        )
        .with_batch_writer(batch_writer.clone())
        .with_recommended_skills(recommended_skills.clone());

        let mut response_acc = ResponseAccumulator::new();

        // Append user message to session stream BEFORE execution
        batch_writer.session_message(&session_id, &execution_id, "user", &message, None, None);

        // Per-turn mutable state — kept in one struct so the event
        // handlers take `&mut EventAccumulator` instead of 10 parameters.
        let mut acc = EventAccumulator {
            tool_acc: ToolCallAccumulator::new(),
            turn_tool_calls: Vec::new(),
            turn_text: String::new(),
            working_memory: WorkingMemory::new(1500),
            pending_recall_triggers: Vec::new(),
            current_tool_name: String::new(),
        };

        // Seed working memory from recalled corrections (system messages)
        for msg in &history {
            if msg.role == "system" {
                let content = msg.text_content();
                if content.contains("Recalled") || content.contains("correction") {
                    for line in content.lines() {
                        let trimmed = line.trim().trim_start_matches("- ");
                        if trimmed.starts_with("[correction]") || trimmed.starts_with("[pattern]") {
                            acc.working_memory.add_correction(trimmed);
                        }
                    }
                }
            }
        }

        // Inject working memory into history if it has content
        if !acc.working_memory.is_empty() {
            history.push(ChatMessage::system(acc.working_memory.format_for_prompt()));
        }

        // Immutable handler deps — constructed once, borrowed into each
        // event handler call.
        let session_id_inner = session_id.clone();
        let execution_id_inner = execution_id.clone();
        let agent_id_inner = agent_id.clone();
        let batch_writer_inner = batch_writer.clone();
        let kg_episode_repo_inner = self.kg_episode_repo.clone();
        let graph_storage_inner = self.graph_storage.clone();

        // Execute with streaming — closure dispatches into free-fn
        // handlers defined at module scope (handle_tool_call_start,
        // handle_tool_result). Keeps the spawn body flat.
        let result = executor
            .execute_stream(&message, &history, |event| {
                if handle.is_stop_requested() {
                    return;
                }

                handle.increment();

                let deps = EventHandlerDeps {
                    batch_writer: &batch_writer_inner,
                    session_id: &session_id_inner,
                    execution_id: &execution_id_inner,
                    agent_id: &agent_id_inner,
                    handle: &handle,
                    kg_episode_repo: kg_episode_repo_inner.as_ref(),
                    graph_storage: graph_storage_inner.as_ref(),
                };

                // Stream messages to session as they happen
                match &event {
                    agent_runtime::StreamEvent::ToolCallStart {
                        tool_id,
                        tool_name,
                        args,
                        ..
                    } => handle_tool_call_start(&mut acc, tool_id, tool_name, args),
                    agent_runtime::StreamEvent::ToolResult {
                        tool_id,
                        result,
                        error,
                        ..
                    } => handle_tool_result(&mut acc, &deps, tool_id, result, error.as_deref()),
                    agent_runtime::StreamEvent::Token { content, .. } => {
                        acc.turn_text.push_str(content);
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

        // Execute micro-recall triggers collected during the stream
        if !acc.pending_recall_triggers.is_empty() {
            let recall_ctx = MicroRecallContext {
                memory_repo: self.memory_repo.clone(),
                graph_storage: self.graph_storage.clone(),
                agent_id: agent_id.clone(),
            };
            for (trigger, iter) in &acc.pending_recall_triggers {
                working_memory_middleware::execute_micro_recall_triggers(
                    &mut acc.working_memory,
                    std::slice::from_ref(trigger),
                    &recall_ctx,
                    *iter,
                )
                .await;
            }
        }

        let accumulated_response = response_acc.into_response();

        tracing::info!(
            execution_id = %execution_id,
            response_len = accumulated_response.len(),
            tool_calls_count = acc.tool_acc.len(),
            "Execution stream completed"
        );

        // Emit any remaining text that wasn't flushed as part of a tool-call turn.
        // If turn_text is empty, the response was already written when the last
        // ToolResult (e.g., from the respond tool) flushed it. Don't write again.
        if !acc.turn_text.is_empty() {
            batch_writer.session_message(
                &session_id,
                &execution_id,
                "assistant",
                &acc.turn_text,
                None,
                None,
            );

            // Log the response for session replay
            let response_log = api_logs::ExecutionLog::new(
                &execution_id,
                &session_id,
                &agent_id,
                api_logs::LogLevel::Info,
                api_logs::LogCategory::Response,
                &accumulated_response,
            );
            batch_writer.log(response_log);
        }

        // Handle completion
        match result {
            Ok(()) => {
                // Check if this execution spawned delegations that are still active.
                // Use session.pending_delegations (set synchronously in handle_delegation)
                // rather than delegation_registry (populated asynchronously by spawn).
                let has_active_delegations = self
                    .state_service
                    .get_session(&session_id)
                    .ok()
                    .flatten()
                    .map(|s| s.has_pending_delegations())
                    .unwrap_or(false);

                if has_active_delegations {
                    // Root paused for delegation — do NOT complete execution.
                    // The continuation callback will handle completion.
                    tracing::info!(
                        session_id = %session_id,
                        "Root paused for delegation — skipping execution completion"
                    );

                    // Request continuation so the session resumes when delegations complete
                    if let Err(e) = self.state_service.request_continuation(&session_id) {
                        tracing::warn!("Failed to request continuation: {}", e);
                    }

                    // Aggregate tokens so UI shows progress
                    if let Err(e) = self.state_service.aggregate_session_tokens(&session_id) {
                        tracing::warn!("Failed to aggregate session tokens: {}", e);
                    }
                } else {
                    // Normal completion — no active delegations
                    complete_execution(CompleteExecution {
                        state_service: &self.state_service,
                        log_service: &self.log_service,
                        event_bus: &self.event_bus,
                        execution_id: &execution_id,
                        session_id: &session_id,
                        agent_id: &agent_id,
                        conversation_id: &conversation_id,
                        response: Some(accumulated_response),
                        connector_registry: self.connector_registry.as_ref(),
                        respond_to: respond_to.as_ref(),
                        thread_id: thread_id.as_deref(),
                        bridge_registry: self.bridge_registry.as_ref(),
                        bridge_outbox: self.bridge_outbox.as_ref(),
                    })
                    .await;
                }

                // Ward AGENTS.md and memory-bank/ are curated manually by agents;
                // the runtime no longer rewrites them post-execution.
                let session_ward = self
                    .state_service
                    .get_session(&session_id)
                    .ok()
                    .flatten()
                    .and_then(|s| s.ward_id);

                // Fire-and-forget session distillation, followed by ward artifact indexing.
                if let Some(distiller) = self.distiller.as_ref() {
                    let distiller = distiller.clone();
                    let sid = session_id.clone();
                    let aid = agent_id.clone();
                    let ward_id_for_indexer = session_ward.clone();
                    let kg_episode_repo_for_indexer = self.kg_episode_repo.clone();
                    let graph_storage_for_indexer = self.graph_storage.clone();
                    let paths_for_indexer = self.paths.clone();
                    tokio::spawn(async move {
                        if let Err(e) = distiller.distill(&sid, &aid).await {
                            tracing::warn!("Session distillation failed: {}", e);
                        }
                        super::core::run_ward_artifact_indexer(
                            &ward_id_for_indexer,
                            &sid,
                            &aid,
                            kg_episode_repo_for_indexer.as_ref(),
                            graph_storage_for_indexer.as_ref(),
                            &paths_for_indexer,
                        )
                        .await;
                    });
                }
            }
            Err(e) => {
                // Crash execution and emit events
                crash_execution(CrashExecution {
                    state_service: &self.state_service,
                    log_service: &self.log_service,
                    event_bus: &self.event_bus,
                    execution_id: &execution_id,
                    session_id: &session_id,
                    agent_id: &agent_id,
                    conversation_id: &conversation_id,
                    error: &e.to_string(),
                    crash_session: true, // crash session for root execution
                })
                .await;

                // Cancel any orphaned delegations for this session
                super::core::cancel_session_delegations(
                    &session_id,
                    &self.delegation_registry,
                    &self.handles,
                    &self.state_service,
                )
                .await;
            }
        }

        // Check if stopped
        if handle.is_stop_requested() {
            stop_execution(StopExecution {
                state_service: &self.state_service,
                log_service: &self.log_service,
                event_bus: &self.event_bus,
                execution_id: &execution_id,
                session_id: &session_id,
                agent_id: &agent_id,
                conversation_id: &conversation_id,
                iteration: handle.current_iteration(),
            })
            .await;
        }

        Ok(())
    }
}
