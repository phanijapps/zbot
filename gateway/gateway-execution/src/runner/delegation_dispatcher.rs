//! # DelegationDispatcher
//!
//! Long-lived per-session queue for spawning subagents. Within a session,
//! delegations run sequentially. Across sessions, they interleave up to the
//! configured semaphore cap.
//!
//! ## Architecture
//!
//! `DelegationDispatcher` is a 3-field struct: it holds only the concurrency
//! semaphore, the inbound request channel, and a [`DelegationSpawner`]. Everything
//! else (the 21 deps that `spawn_delegated_agent` needs) lives inside the
//! `RunnerDelegationInvoker` companion, which is constructed by
//! `ExecutionRunner::make_delegation_invoker()` and injected at wire-up time.
//!
//! This keeps the dispatcher testable with a `StubSessionInvoker` (one trait
//! method per stub) while keeping the production path complete.
//!
//! ## Queue semantics
//!
//! - Sequential (`parallel: false`): only one delegation per session runs at a
//!   time; extras are queued and dispatched in order as each finishes.
//! - Parallel (`parallel: true`): skip the per-session queue and go straight
//!   to the global semaphore.
//! - Global cap: the `delegation_semaphore` gates total concurrent subagents
//!   regardless of session. The permit is acquired here and passed to the
//!   invoker so it holds it for the duration of the child execution.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use api_logs::LogService;
use async_trait::async_trait;
use execution_state::StateService;
use gateway_events::EventBus;
use gateway_services::{AgentService, McpService, ProviderService, SharedVaultPaths};
use tokio::sync::{mpsc, OwnedSemaphorePermit, RwLock, Semaphore};
use tokio::task::JoinHandle;
use zero_stores_sqlite::{ConversationRepository, DatabaseManager};

use crate::delegation::{spawn_delegated_agent, DelegationRegistry, DelegationRequest};
use crate::handle::ExecutionHandle;
use crate::invoke::WorkspaceCache;
use crate::runner::session_invoker::DelegationSpawner;

/// Dispatcher that enforces per-session sequential ordering and global
/// concurrency cap for subagent delegations.
pub struct DelegationDispatcher {
    pub delegation_rx: mpsc::UnboundedReceiver<DelegationRequest>,
    pub delegation_semaphore: Arc<Semaphore>,
    pub invoker: Arc<dyn DelegationSpawner>,
}

impl DelegationDispatcher {
    /// Start the dispatcher loop in a background task.
    ///
    /// Returns the `JoinHandle` — callers that only need fire-and-forget
    /// can drop it; tests hold it to `.await` shutdown.
    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(self.run())
    }

    async fn run(mut self) {
        // Per-session tracking: only one delegation active per session at a time.
        let mut active_sessions: HashSet<String> = HashSet::new();
        let mut queued: HashMap<String, VecDeque<DelegationRequest>> = HashMap::new();

        // Completion notification channel: each spawned task sends its session_id
        // here when it finishes so the next queued request can be dispatched.
        let (done_tx, mut done_rx) = mpsc::unbounded_channel::<String>();

        // `rx_open` tracks whether the inbound request channel is still open.
        // When it closes the dispatcher drains in-flight work then exits.
        let mut rx_open = true;

        loop {
            tokio::select! {
                msg = self.delegation_rx.recv(), if rx_open => {
                    match msg {
                        Some(request) => {
                            let session_id = request.session_id.clone();

                            if request.parallel {
                                // Parallel: skip per-session queue, go straight to global semaphore.
                                tracing::info!(
                                    session_id = %session_id,
                                    child_agent = %request.child_agent_id,
                                    "Parallel delegation — bypassing per-session queue"
                                );
                                self.spawn_with_notification(request, done_tx.clone());
                            } else if active_sessions.contains(&session_id) {
                                // Sequential: queue behind the active delegation for this session.
                                tracing::info!(
                                    session_id = %session_id,
                                    agent = %request.child_agent_id,
                                    queued = queued.get(&session_id).map(|q| q.len()).unwrap_or(0),
                                    "Queuing delegation (active delegation in progress)"
                                );
                                queued.entry(session_id).or_default().push_back(request);
                            } else {
                                // Sequential: no active delegation, spawn immediately.
                                tracing::info!(
                                    session_id = %session_id,
                                    parent_agent = %request.parent_agent_id,
                                    child_agent = %request.child_agent_id,
                                    "Processing delegation request"
                                );
                                active_sessions.insert(session_id.clone());
                                self.spawn_with_notification(request, done_tx.clone());
                            }
                        }
                        None => {
                            // Inbound channel closed — stop accepting new requests.
                            rx_open = false;
                            tracing::info!("DelegationDispatcher: request channel closed, draining in-flight work");
                            // If nothing is in-flight, exit immediately.
                            if active_sessions.is_empty() && queued.is_empty() {
                                break;
                            }
                        }
                    }
                }
                Some(completed_session) = done_rx.recv() => {
                    active_sessions.remove(&completed_session);

                    // Pop the next queued request for this session (if any).
                    if let Some(queue) = queued.get_mut(&completed_session) {
                        if let Some(next) = queue.pop_front() {
                            tracing::info!(
                                session_id = %completed_session,
                                agent = %next.child_agent_id,
                                remaining = queue.len(),
                                "Dequeuing next delegation"
                            );
                            active_sessions.insert(completed_session.clone());
                            self.spawn_with_notification(next, done_tx.clone());
                        }
                        if queued
                            .get(&completed_session)
                            .map(|q| q.is_empty())
                            .unwrap_or(true)
                        {
                            queued.remove(&completed_session);
                        }
                    }

                    // If the inbound channel closed and all work is drained, exit.
                    if !rx_open && active_sessions.is_empty() && queued.is_empty() {
                        break;
                    }
                }
                else => break,
            }
        }
    }

    /// Acquire the global semaphore permit, call the invoker's
    /// `spawn_delegation`, then signal the run-loop via `done_tx` so the
    /// next queued request for the same session can be dispatched.
    ///
    /// The permit is passed *into* the invoker so it's held for the
    /// duration of the child execution (not just the spawn call).
    fn spawn_with_notification(
        &self,
        request: DelegationRequest,
        done_tx: mpsc::UnboundedSender<String>,
    ) {
        let session_id = request.session_id.clone();
        let semaphore = self.delegation_semaphore.clone();
        let invoker = self.invoker.clone();

        tokio::spawn(async move {
            let permit = semaphore.acquire_owned().await.ok();

            let child_agent_id = request.child_agent_id.clone();
            let result = invoker.spawn_delegation(request, permit).await;

            if let Err(e) = &result {
                tracing::error!(
                    session_id = %session_id,
                    agent = %child_agent_id,
                    error = %e,
                    "Delegation failed"
                );
            }

            // Notify the run-loop that this session's delegation is done.
            let _ = done_tx.send(session_id);
        });
    }
}

// ============================================================================
// RunnerDelegationInvoker
// ============================================================================

/// Companion to `ExecutionRunner` that holds the subset of runner fields
/// needed to call `spawn_delegated_agent`, implementing [`DelegationSpawner`]
/// so `DelegationDispatcher` remains decoupled from the concrete runner type.
///
/// Constructed via [`ExecutionRunner::make_delegation_invoker`] inside
/// `with_config` — before the runner is wrapped in `Arc` — so each field
/// gets a clone of the runner's shared handles rather than ownership.
///
/// The full delegation pipeline runs inside `spawn_delegation`:
/// child session creation, delegation registry lifecycle, event emission,
/// subagent rules + ward context + recall priming, executor build + run,
/// success/failure callbacks, and continuation trigger.
pub(crate) struct RunnerDelegationInvoker {
    pub(crate) event_bus: Arc<EventBus>,
    pub(crate) agent_service: Arc<AgentService>,
    pub(crate) provider_service: Arc<ProviderService>,
    pub(crate) mcp_service: Arc<McpService>,
    pub(crate) skill_service: Arc<gateway_services::SkillService>,
    pub(crate) paths: SharedVaultPaths,
    pub(crate) conversation_repo: Arc<ConversationRepository>,
    pub(crate) handles: Arc<RwLock<HashMap<String, ExecutionHandle>>>,
    pub(crate) delegation_registry: Arc<DelegationRegistry>,
    pub(crate) delegation_tx: mpsc::UnboundedSender<DelegationRequest>,
    pub(crate) log_service: Arc<LogService<DatabaseManager>>,
    pub(crate) state_service: Arc<StateService<DatabaseManager>>,
    pub(crate) workspace_cache: WorkspaceCache,
    pub(crate) memory_repo: Option<Arc<zero_stores_sqlite::MemoryRepository>>,
    pub(crate) memory_store: Option<Arc<dyn zero_stores::MemoryFactStore>>,
    pub(crate) embedding_client: Option<Arc<dyn agent_runtime::llm::embedding::EmbeddingClient>>,
    pub(crate) memory_recall: Option<Arc<crate::recall::MemoryRecall>>,
    pub(crate) rate_limiters: Arc<
        std::sync::RwLock<
            std::collections::HashMap<String, Arc<agent_runtime::ProviderRateLimiter>>,
        >,
    >,
    pub(crate) graph_storage: Option<Arc<zero_stores_sqlite::kg::storage::GraphStorage>>,
    pub(crate) ingestion_adapter: Option<Arc<dyn agent_tools::IngestionAccess>>,
    pub(crate) goal_adapter: Option<Arc<dyn agent_tools::GoalAccess>>,
}

#[async_trait]
impl DelegationSpawner for RunnerDelegationInvoker {
    async fn spawn_delegation(
        &self,
        request: DelegationRequest,
        permit: Option<OwnedSemaphorePermit>,
    ) -> Result<(), String> {
        spawn_delegated_agent(
            &request,
            self.event_bus.clone(),
            self.agent_service.clone(),
            self.provider_service.clone(),
            self.mcp_service.clone(),
            self.skill_service.clone(),
            self.paths.clone(),
            self.conversation_repo.clone(),
            self.handles.clone(),
            self.delegation_registry.clone(),
            self.delegation_tx.clone(),
            self.log_service.clone(),
            self.state_service.clone(),
            self.workspace_cache.clone(),
            permit,
            self.memory_repo.clone(),
            self.memory_store.clone(),
            self.embedding_client.clone(),
            self.memory_recall.clone(),
            self.rate_limiters.clone(),
            self.graph_storage.clone(),
            self.ingestion_adapter.clone(),
            self.goal_adapter.clone(),
        )
        .await
        .map(|_| ())
    }
}
