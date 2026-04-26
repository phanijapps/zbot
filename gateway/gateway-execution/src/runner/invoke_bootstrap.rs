//! # InvokeBootstrap
//!
//! Per-session pre-execution setup. Returns a [`SetupResult`] that
//! contains everything [`crate::runner::execution_stream::ExecutionStream`]
//! needs to drive the agent loop.
//!
//! Field list = dependency contract. The [`InvokeBootstrap::setup`] body is
//! the verbatim first half of the old `invoke_with_callback` (pre-extraction
//! lines 634–845 of core.rs), ending immediately before the
//! `ExecutionStream` assembly. Helper methods (`create_executor`,
//! `run_intent_analysis`, `emit_error`, `emit_intent_fallback_complete`,
//! `get_rate_limiter`) are implemented here directly because they operate
//! exclusively on the bootstrap's own field set.

use std::collections::HashMap;
use std::sync::Arc;

use agent_runtime::{AgentExecutor, ChatMessage};
use api_logs::LogService;
use arc_swap::ArcSwapOption;
use execution_state::StateService;
use gateway_database::{ConversationRepository, DatabaseManager};
use gateway_events::{EventBus, GatewayEvent};
use gateway_services::{
    AgentService, McpService, ModelRegistry, ProviderService, SharedVaultPaths, SkillService,
};
use tokio::sync::RwLock;

use crate::config::ExecutionConfig;
use crate::handle::ExecutionHandle;
use crate::invoke::{
    collect_agents_summary, collect_skills_summary, AgentLoader, ExecutorBuilder, WorkspaceCache,
};
use crate::lifecycle::{emit_agent_started, get_or_create_session, start_execution};
use crate::middleware::intent_analysis::{
    analyze_intent, format_intent_injection, index_resources,
};

// ============================================================================
// STRUCTS
// ============================================================================

/// All dependencies required to run the per-session setup phase of
/// `invoke_with_callback`. Built once in `ExecutionRunner::with_config` and
/// stored as a field so the runner delegates the bootstrap work here.
pub(super) struct InvokeBootstrap {
    pub agent_service: Arc<AgentService>,
    pub provider_service: Arc<ProviderService>,
    pub mcp_service: Arc<McpService>,
    pub skill_service: Arc<SkillService>,
    pub state_service: Arc<StateService<DatabaseManager>>,
    pub log_service: Arc<LogService<DatabaseManager>>,
    pub conversation_repo: Arc<ConversationRepository>,
    pub paths: SharedVaultPaths,
    pub memory_repo: Option<Arc<gateway_database::MemoryRepository>>,
    pub memory_recall: Option<Arc<crate::recall::MemoryRecall>>,
    pub embedding_client: Option<Arc<dyn agent_runtime::llm::embedding::EmbeddingClient>>,
    pub model_registry: Arc<ArcSwapOption<ModelRegistry>>,
    pub rate_limiters: Arc<
        std::sync::RwLock<
            std::collections::HashMap<String, Arc<agent_runtime::ProviderRateLimiter>>,
        >,
    >,
    pub connector_registry: Option<Arc<gateway_connectors::ConnectorRegistry>>,
    pub bridge_registry: Option<Arc<gateway_bridge::BridgeRegistry>>,
    pub bridge_outbox: Option<Arc<gateway_bridge::OutboxRepository>>,
    pub graph_storage: Option<Arc<knowledge_graph::GraphStorage>>,
    pub ingestion_adapter: Option<Arc<dyn agent_tools::IngestionAccess>>,
    pub goal_adapter: Option<Arc<dyn agent_tools::GoalAccess>>,
    pub event_bus: Arc<EventBus>,
    pub handles: Arc<RwLock<HashMap<String, ExecutionHandle>>>,
    pub workspace_cache: WorkspaceCache,
}

/// Output of [`InvokeBootstrap::begin_setup`]. Carries the state that phase 2
/// ([`InvokeBootstrap::finish_setup`]) needs and that the caller needs to pass
/// to the `on_session_ready` callback.
///
/// The caller fires the callback after receiving this value and BEFORE calling
/// [`InvokeBootstrap::finish_setup`], so the subscriber is registered before
/// `AgentStarted`, `IntentAnalysisStarted`, and `IntentAnalysisComplete` fire.
pub(super) struct PartialSetup {
    pub session_id: String,
    pub execution_id: String,
    pub handle: ExecutionHandle,
    /// Ward ID resolved during phase 1; forwarded to phase 2 for executor
    /// construction and placeholder-spec injection.
    pub ward_id: Option<String>,
}

/// Output of [`InvokeBootstrap::finish_setup`]. Contains everything that lives
/// across the seam between bootstrap and stream execution.
pub(super) struct SetupResult {
    pub session_id: String,
    pub execution_id: String,
    pub executor: AgentExecutor,
    pub handle: ExecutionHandle,
    pub history: Vec<ChatMessage>,
    pub recommended_skills: Vec<String>,
}

// ============================================================================
// PRIVATE CONTEXT TYPES (mirrors the same structs in core.rs)
// ============================================================================

/// Borrowed inputs for [`InvokeBootstrap::create_executor`].
struct CreateExecutorArgs<'a> {
    agent: &'a gateway_services::agents::Agent,
    provider: &'a gateway_services::providers::Provider,
    config: &'a ExecutionConfig,
    session_id: &'a str,
    ward_id: Option<&'a str>,
    is_root: bool,
    user_message: Option<&'a str>,
    execution_id: &'a str,
}

/// Borrowed inputs for [`InvokeBootstrap::run_intent_analysis`].
struct IntentAnalysisCtx<'a> {
    agent: &'a gateway_services::agents::Agent,
    provider: &'a gateway_services::providers::Provider,
    config: &'a ExecutionConfig,
    session_id: &'a str,
    execution_id: &'a str,
    is_root: bool,
    user_message: Option<&'a str>,
    fact_store: Option<&'a Arc<dyn zero_core::MemoryFactStore>>,
}

/// Return type of [`InvokeBootstrap::run_intent_analysis`].
struct IntentOutcome {
    recommended_skills: Vec<String>,
    instructions_injection: String,
}

// ============================================================================
// IMPL
// ============================================================================

impl InvokeBootstrap {
    /// Phase 1: create or resume the session, persist routing, start the
    /// execution record, and store the handle. Returns BEFORE any agent or
    /// intent events fire so the caller can register subscribers first.
    ///
    /// # Ordering contract
    ///
    /// ```text
    /// begin_setup  [get_or_create_session, persist_routing,
    ///               start_execution, store_handle]
    /// → on_session_ready CALLBACK (caller fires this)
    /// → finish_setup [emit_agent_started, load_agent, run_intent_analysis,
    ///                 inject_placeholder, build executor]
    /// → tokio::spawn
    /// ```
    pub(super) async fn begin_setup(
        &self,
        config: &mut ExecutionConfig,
    ) -> Result<PartialSetup, String> {
        let handle = ExecutionHandle::new(config.max_iterations);

        // Get or create session and execution
        let session_setup = get_or_create_session(
            &self.state_service,
            &config.agent_id,
            config.session_id.as_deref(),
            config.source,
        );
        let session_id = session_setup.session_id;
        let execution_id = session_setup.execution_id;
        let ward_id = session_setup.ward_id;

        // If session has a persisted mode, use it (overrides invoke mode)
        if let Ok(Some(session)) = self.state_service.get_session(&session_id) {
            if let Some(ref persisted_mode) = session.mode {
                config.mode = Some(persisted_mode.clone());
            }
        }

        // Persist routing fields on the session (thread_id, connector_id, respond_to)
        if config.thread_id.is_some()
            || config.connector_id.is_some()
            || config.respond_to.is_some()
        {
            if let Err(e) = self.state_service.update_session_routing(
                &session_id,
                config.thread_id.as_deref(),
                config.connector_id.as_deref(),
                config.respond_to.as_ref(),
            ) {
                tracing::warn!("Failed to persist session routing: {}", e);
            }
        }

        // Start execution and log
        start_execution(
            &self.state_service,
            &self.log_service,
            &execution_id,
            &session_id,
            &config.agent_id,
            None,
        );

        // Store handle
        {
            let mut handles = self.handles.write().await;
            handles.insert(config.conversation_id.clone(), handle.clone());
        }

        Ok(PartialSetup {
            session_id,
            execution_id,
            handle,
            ward_id,
        })
    }

    /// Phase 2: emit `AgentStarted`, load the agent, run intent analysis,
    /// inject placeholder specs, and build the executor. Receives the
    /// [`PartialSetup`] produced by [`Self::begin_setup`].
    ///
    /// The caller MUST fire the `on_session_ready` callback between
    /// `begin_setup` and `finish_setup` so all events emitted here are
    /// visible to the subscriber.
    pub(super) async fn finish_setup(
        &self,
        config: &ExecutionConfig,
        message: &str,
        partial: PartialSetup,
    ) -> Result<SetupResult, String> {
        let PartialSetup {
            session_id,
            execution_id,
            handle,
            ward_id,
        } = partial;

        // Emit start event — subscriber is already registered at this point.
        emit_agent_started(
            &self.event_bus,
            &config.agent_id,
            &config.conversation_id,
            &session_id,
            &execution_id,
        )
        .await;

        // Load agent configuration (or create default for "root" agent)
        let settings_for_loader = gateway_services::SettingsService::new(self.paths.clone());
        let agent_loader = AgentLoader::new(
            &self.agent_service,
            &self.provider_service,
            self.paths.clone(),
        )
        .with_settings(&settings_for_loader)
        .with_chat_mode(config.is_chat_mode());
        let (agent, provider) = match agent_loader.load_or_create_root(&config.agent_id).await {
            Ok(result) => result,
            Err(e) => {
                self.emit_error(&config.conversation_id, &config.agent_id, &e)
                    .await;
                return Err(e);
            }
        };

        // Load full session conversation (all messages including tool calls/results)
        let mut history: Vec<ChatMessage> = self
            .conversation_repo
            .get_session_conversation(&session_id, 200)
            .map(|messages| {
                self.conversation_repo
                    .session_messages_to_chat_format(&messages)
            })
            .unwrap_or_default();

        // Graph-powered recall for first message — inject remembered facts, episodes, and
        // entity context before the agent sees the user's message.
        // Runs in BOTH chat and research modes (Phase 7): only the pipeline depth is
        // gated on mode; memory must reach every session. Chat mode uses a smaller budget
        // to keep latency low.
        if let Some(recall) = &self.memory_recall {
            let top_k = if config.is_chat_mode() { 5 } else { 10 };
            match recall
                .recall_unified(&config.agent_id, message, ward_id.as_deref(), &[], top_k)
                .await
            {
                Ok(items) if !items.is_empty() => {
                    let formatted = crate::recall::format_scored_items(&items);
                    if !formatted.is_empty() {
                        history.insert(0, ChatMessage::system(formatted));
                    }
                    tracing::info!(
                        agent_id = %config.agent_id,
                        count = items.len(),
                        "Recalled unified context for first message"
                    );
                }
                Ok(_) => {
                    tracing::debug!(
                        "First-message unified recall returned empty — no relevant items"
                    );
                }
                Err(e) => {
                    // Surface the failure so the agent can drill manually instead
                    // of assuming memory was silently empty. Empty results (Ok case
                    // above) stay quiet — only genuine errors are reported.
                    tracing::warn!("First-message unified recall failed: {}", e);
                    history.insert(
                        0,
                        ChatMessage::system(crate::recall::format_recall_failure_message(&e)),
                    );
                }
            }
        }

        // Create executor (restore ward_id from existing session if available)
        let (executor, recommended_skills) = match self
            .create_executor(CreateExecutorArgs {
                agent: &agent,
                provider: &provider,
                config,
                session_id: &session_id,
                ward_id: ward_id.as_deref(),
                is_root: true,
                user_message: Some(message),
                execution_id: &execution_id,
            })
            .await
        {
            Ok(result) => result,
            Err(e) => {
                self.emit_error(&config.conversation_id, &config.agent_id, &e)
                    .await;
                return Err(e);
            }
        };

        // Inject mandatory first action for graph tasks with placeholder specs
        if let Some(ref wid) = ward_id {
            let specs_dir = self.paths.vault_dir().join("wards").join(wid).join("specs");
            if specs_dir.exists() {
                let has_placeholders = std::fs::read_dir(&specs_dir)
                    .ok()
                    .map(|entries| {
                        entries
                            .filter_map(|e| e.ok())
                            .filter(|e| e.path().is_dir())
                            .any(|topic_dir| {
                                std::fs::read_dir(topic_dir.path())
                                    .ok()
                                    .map(|files| {
                                        files.filter_map(|f| f.ok()).any(|f| {
                                            std::fs::read_to_string(f.path())
                                                .ok()
                                                .map(|c| c.contains("Status: placeholder"))
                                                .unwrap_or(false)
                                        })
                                    })
                                    .unwrap_or(false)
                            })
                    })
                    .unwrap_or(false);

                if has_placeholders {
                    history.push(ChatMessage::system(
                        "[MANDATORY FIRST ACTION] Placeholder specs found in the ward's specs/ folder. \
                         You MUST delegate to a planning subagent as your first action. \
                         Follow the pipeline in your planning shard: delegate to data-analyst with max_iterations=40 \
                         to fill the specs and analyze core/. Do NOT load skills, create plans, or write code yourself.".to_string()
                    ));
                    tracing::info!(ward = %wid, "Injected mandatory planning action for graph task");
                }
            }
        }

        Ok(SetupResult {
            session_id,
            execution_id,
            executor,
            handle,
            history,
            recommended_skills,
        })
    }

    // =========================================================================
    // HELPER METHODS (verbatim from ExecutionRunner, operating on bootstrap fields)
    // =========================================================================

    /// Build an [`AgentExecutor`] from the given args. Mirrors the same-named
    /// method on `ExecutionRunner`.
    async fn create_executor(
        &self,
        args: CreateExecutorArgs<'_>,
    ) -> Result<(AgentExecutor, Vec<String>), String> {
        let CreateExecutorArgs {
            agent,
            provider,
            config,
            session_id,
            ward_id,
            is_root,
            user_message,
            execution_id,
        } = args;

        // Collect available agents and skills for executor state
        let available_agents = collect_agents_summary(&self.agent_service).await;
        let available_skills = collect_skills_summary(&self.skill_service).await;

        // Get tool settings
        let settings_service = gateway_services::SettingsService::new(self.paths.clone());
        let tool_settings = settings_service.get_tool_settings().unwrap_or_default();

        // Build hook context if present
        let hook_context = config
            .hook_context
            .as_ref()
            .and_then(|ctx| serde_json::to_value(ctx).ok());

        // Build fact store from memory repo + embedding client (if available)
        let fact_store: Option<Arc<dyn zero_core::MemoryFactStore>> =
            self.memory_repo.as_ref().map(|repo| {
                Arc::new(gateway_database::GatewayMemoryFactStore::new(
                    repo.clone(),
                    self.embedding_client.clone(),
                )) as Arc<dyn zero_core::MemoryFactStore>
            });
        // Clone for resource indexing (before fact_store is moved into builder)
        let fact_store_for_indexing = fact_store.clone();

        // Build connector resource provider (HTTP + bridge composite)
        let http_provider: Option<Arc<dyn zero_core::ConnectorResourceProvider>> =
            self.connector_registry.as_ref().map(|registry| {
                Arc::new(crate::resource_provider::GatewayResourceProvider::new(
                    registry.clone(),
                )) as Arc<dyn zero_core::ConnectorResourceProvider>
            });
        let bridge_provider: Option<Arc<dyn zero_core::ConnectorResourceProvider>> = self
            .bridge_registry
            .as_ref()
            .zip(self.bridge_outbox.as_ref())
            .map(|(reg, outbox)| {
                Arc::new(gateway_bridge::BridgeResourceProvider::new(
                    reg.clone(),
                    outbox.clone(),
                )) as Arc<dyn zero_core::ConnectorResourceProvider>
            });
        let connector_provider: Option<Arc<dyn zero_core::ConnectorResourceProvider>> =
            if http_provider.is_some() || bridge_provider.is_some() {
                Some(
                    Arc::new(crate::composite_provider::CompositeResourceProvider::new(
                        http_provider,
                        bridge_provider,
                    )) as Arc<dyn zero_core::ConnectorResourceProvider>,
                )
            } else {
                None
            };

        // Get or create shared rate limiter for this provider
        let rate_limiter = self.get_rate_limiter(provider);
        tracing::debug!(provider = %provider.name, "Using shared rate limiter for provider");

        // Use ExecutorBuilder to create the executor
        let mut builder = ExecutorBuilder::new(self.paths.vault_dir().clone(), tool_settings)
            .with_workspace_cache(self.workspace_cache.clone())
            .with_rate_limiter(rate_limiter)
            .with_chat_mode(config.is_chat_mode());
        if let Some(registry) = self.model_registry.load_full() {
            builder = builder.with_model_registry(registry);
        }
        if let Some(fs) = fact_store {
            builder = builder.with_fact_store(fs);
        }
        if let Some(cp) = connector_provider {
            builder = builder.with_connector_provider(cp);
        }
        if let Some(ref gs) = self.graph_storage {
            builder = builder.with_graph_storage(gs.clone());
        }
        if let Some(ref a) = self.ingestion_adapter {
            builder = builder.with_ingestion_adapter(a.clone());
        }
        if let Some(ref a) = self.goal_adapter {
            builder = builder.with_goal_adapter(a.clone());
        }

        // Intent analysis for root agent first turns only.
        // Note: execution_logs stores execution_id in the session_id column,
        // so we query by execution_id to find prior intent logs.
        let mut agent_for_build = agent.clone();
        let mut recommended_skills: Vec<String> = Vec::new();
        let outcome = self
            .run_intent_analysis(IntentAnalysisCtx {
                agent,
                provider,
                config,
                session_id,
                execution_id,
                is_root,
                user_message,
                fact_store: fact_store_for_indexing.as_ref(),
            })
            .await;
        if let Some(out) = outcome {
            recommended_skills = out.recommended_skills;
            agent_for_build
                .instructions
                .push_str(&out.instructions_injection);
        }

        // Flag if placeholder specs exist — delegate tool uses this to block ad-hoc delegations
        if is_root {
            if let Some(wid) = ward_id {
                let specs_dir = self.paths.vault_dir().join("wards").join(wid).join("specs");
                if specs_dir.exists() {
                    let has_placeholders = std::fs::read_dir(&specs_dir)
                        .ok()
                        .map(|entries| entries.filter_map(|e| e.ok()).any(|e| e.path().is_dir()))
                        .unwrap_or(false);
                    if has_placeholders {
                        builder = builder.with_initial_state(
                            "app:has_placeholder_specs",
                            serde_json::Value::Bool(true),
                        );
                    }
                }
            }
        }

        let mut executor = builder
            .build(
                &agent_for_build,
                provider,
                &config.conversation_id,
                session_id,
                &available_agents,
                &available_skills,
                hook_context.as_ref(),
                &self.mcp_service,
                ward_id,
            )
            .await?;

        super::core::attach_mid_session_recall_hook(
            &mut executor,
            self.memory_recall.as_ref(),
            &agent.id,
            ward_id,
        );

        Ok((executor, recommended_skills))
    }

    /// Run the intent-analysis sub-pipeline. Mirrors the same-named method on
    /// `ExecutionRunner`.
    async fn run_intent_analysis(&self, ctx: IntentAnalysisCtx<'_>) -> Option<IntentOutcome> {
        let IntentAnalysisCtx {
            agent,
            provider,
            config,
            session_id,
            execution_id,
            is_root,
            user_message,
            fact_store,
        } = ctx;

        // Guard: non-root or chat-mode — never run intent analysis.
        if !is_root || config.is_chat_mode() {
            return None;
        }

        // Already analyzed (e.g. continuation turn): emit Skipped so the
        // UI renders a block, then return.
        if self.log_service.has_intent_log(execution_id) {
            self.event_bus
                .publish(gateway_events::GatewayEvent::IntentAnalysisSkipped {
                    session_id: session_id.to_string(),
                    execution_id: execution_id.to_string(),
                })
                .await;
            tracing::debug!("Intent analysis skipped (already analyzed for this execution)");
            return None;
        }

        let fs = fact_store?;
        let msg = user_message?;

        // Index resources (fast DB upsert — no LLM call). Runs before
        // analyze_intent so the analyzer has the latest capability index.
        index_resources(
            fs.as_ref(),
            &self.skill_service,
            &self.agent_service,
            &self.paths,
        )
        .await;
        tracing::info!("Resource indexing complete (skills, agents, wards)");

        // Emit started event so UI can show "Analyzing..."
        self.event_bus
            .publish(gateway_events::GatewayEvent::IntentAnalysisStarted {
                session_id: session_id.to_string(),
                execution_id: execution_id.to_string(),
            })
            .await;

        // Build temporary LLM client for analysis.
        let llm_config = agent_runtime::LlmConfig::new(
            provider.base_url.clone(),
            provider.api_key.clone(),
            agent.model.clone(),
            provider.id.clone().unwrap_or_else(|| provider.name.clone()),
        )
        .with_max_tokens(2048); // Intent analysis JSON is 1-2KB — keep max_tokens low for speed

        let raw_client = match agent_runtime::OpenAiClient::new(llm_config) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to create LLM client for intent analysis: {}", e);
                self.emit_intent_fallback_complete(
                    session_id,
                    execution_id,
                    "LLM client creation failed — using scratch ward",
                    "Intent analysis unavailable (no LLM client)",
                )
                .await;
                return None;
            }
        };

        let retrying = agent_runtime::RetryingLlmClient::new(
            std::sync::Arc::new(raw_client),
            agent_runtime::RetryPolicy::default(),
        );
        let system_prompt =
            crate::middleware::intent_analysis::load_intent_analysis_prompt(&self.paths);

        let analysis = match analyze_intent(
            &retrying,
            msg,
            fs.as_ref(),
            self.memory_recall.as_ref().map(|r| r.as_ref()),
            &system_prompt,
        )
        .await
        {
            Ok(a) => a,
            Err(e) => {
                tracing::warn!("Intent analysis failed (non-fatal): {}", e);
                self.emit_intent_fallback_complete(
                    session_id,
                    execution_id,
                    "Intent analysis failed — using scratch ward",
                    "Intent analysis unavailable",
                )
                .await;
                return None;
            }
        };

        tracing::info!(
            primary_intent = %analysis.primary_intent,
            approach = %analysis.execution_strategy.approach,
            "Intent analysis succeeded"
        );

        // Emit IntentAnalysisComplete event with the real analysis.
        self.event_bus
            .publish(GatewayEvent::IntentAnalysisComplete {
                session_id: session_id.to_string(),
                execution_id: execution_id.to_string(),
                primary_intent: analysis.primary_intent.clone(),
                hidden_intents: analysis.hidden_intents.clone(),
                recommended_skills: analysis.recommended_skills.clone(),
                recommended_agents: analysis.recommended_agents.clone(),
                ward_recommendation: serde_json::to_value(&analysis.ward_recommendation)
                    .unwrap_or_default(),
                execution_strategy: serde_json::to_value(&analysis.execution_strategy)
                    .unwrap_or_default(),
            })
            .await;

        // Phase 2b: populate session ctx with the intent-analyzer's
        // decision + verbatim user prompt. Subagents spawned later can
        // fetch these via memory(get_fact, key="ctx.<sid>.intent") without
        // re-reading the original message.
        let ward = analysis.ward_recommendation.ward_name.as_str();
        let intent_json = serde_json::to_value(&analysis).unwrap_or(serde_json::Value::Null);
        crate::session_ctx::writer::intent_snapshot(fs, session_id, ward, &intent_json, msg).await;

        // Log for session replay.
        if let Ok(meta) = serde_json::to_value(&analysis) {
            let log_entry = api_logs::ExecutionLog::new(
                execution_id,
                session_id,
                &config.agent_id,
                api_logs::LogLevel::Info,
                api_logs::LogCategory::Intent,
                format!("Intent: {}", analysis.primary_intent),
            )
            .with_metadata(meta);
            let _ = self.log_service.log(log_entry);
        }

        // Collect spec guidance from recommended skills' ward_setup.
        let spec_guidance = {
            let mut guidances = Vec::new();
            for skill_name in &analysis.recommended_skills {
                if let Ok(Some(ws)) = self.skill_service.get_ward_setup(skill_name).await {
                    if let Some(ref g) = ws.spec_guidance {
                        guidances.push(g.clone());
                    }
                }
            }
            if guidances.is_empty() {
                None
            } else {
                Some(guidances.join("\n\n"))
            }
        };

        Some(IntentOutcome {
            recommended_skills: analysis.recommended_skills.clone(),
            instructions_injection: format_intent_injection(
                &analysis,
                spec_guidance.as_deref(),
                Some(msg),
            ),
        })
    }

    /// Emit the fallback `IntentAnalysisComplete` event used when the LLM
    /// client can't be built or the analysis call fails.
    async fn emit_intent_fallback_complete(
        &self,
        session_id: &str,
        execution_id: &str,
        ward_reason: &str,
        strategy_explanation: &str,
    ) {
        self.event_bus
            .publish(GatewayEvent::IntentAnalysisComplete {
                session_id: session_id.to_string(),
                execution_id: execution_id.to_string(),
                primary_intent: "general".to_string(),
                hidden_intents: vec![],
                recommended_skills: vec![],
                recommended_agents: vec![],
                ward_recommendation: serde_json::json!({
                    "action": "create_new",
                    "ward_name": "scratch",
                    "subdirectory": null,
                    "reason": ward_reason,
                }),
                execution_strategy: serde_json::json!({
                    "approach": "simple",
                    "explanation": strategy_explanation,
                }),
            })
            .await;
    }

    /// Emit an error event on the conversation.
    async fn emit_error(&self, conversation_id: &str, agent_id: &str, message: &str) {
        self.event_bus
            .publish(GatewayEvent::Error {
                agent_id: Some(agent_id.to_string()),
                session_id: None,
                execution_id: None,
                message: message.to_string(),
                conversation_id: Some(conversation_id.to_string()),
            })
            .await;
    }

    /// Get or create a shared rate limiter for a provider.
    fn get_rate_limiter(
        &self,
        provider: &gateway_services::providers::Provider,
    ) -> Arc<agent_runtime::ProviderRateLimiter> {
        let provider_id = provider.id.clone().unwrap_or_else(|| provider.name.clone());
        let rate_limits = provider.effective_rate_limits();

        // Check if exists (fast path — read lock)
        if let Ok(guard) = self.rate_limiters.read() {
            if let Some(limiter) = guard.get(&provider_id) {
                return limiter.clone();
            }
        }

        // Create new limiter and insert (write lock)
        let limiter = Arc::new(agent_runtime::ProviderRateLimiter::new(
            rate_limits.concurrent_requests,
            rate_limits.requests_per_minute,
        ));

        if let Ok(mut guard) = self.rate_limiters.write() {
            // Use entry API to avoid overwriting if another thread raced us
            guard.entry(provider_id).or_insert_with(|| limiter.clone());
        }

        limiter
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    use api_logs::LogService;
    use arc_swap::ArcSwapOption;
    use execution_state::StateService;
    use gateway_database::{ConversationRepository, DatabaseManager};
    use gateway_events::EventBus;
    use gateway_services::VaultPaths;
    use tokio::sync::RwLock;

    #[test]
    fn invoke_bootstrap_constructs_with_minimum_required_deps() {
        // Compile-as-assertion: locks in the field list as the dependency
        // contract. End-to-end coverage lives in the e2e suite (Tasks 7+8).
        #[allow(deprecated)]
        let dir = tempfile::tempdir().unwrap();
        #[allow(deprecated)]
        let path = dir.into_path();
        let paths = Arc::new(VaultPaths::new(path));
        let db = Arc::new(DatabaseManager::new(paths.clone()).unwrap());
        let handles: Arc<RwLock<HashMap<String, ExecutionHandle>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let _ = InvokeBootstrap {
            agent_service: Arc::new(gateway_services::AgentService::new(paths.agents_dir())),
            provider_service: Arc::new(gateway_services::ProviderService::new(paths.clone())),
            mcp_service: Arc::new(gateway_services::McpService::new(paths.clone())),
            skill_service: Arc::new(gateway_services::SkillService::new(paths.skills_dir())),
            state_service: Arc::new(StateService::new(db.clone())),
            log_service: Arc::new(LogService::new(db.clone())),
            conversation_repo: Arc::new(ConversationRepository::new(db)),
            paths,
            memory_repo: None,
            memory_recall: None,
            embedding_client: None,
            model_registry: Arc::new(ArcSwapOption::empty()),
            rate_limiters: Arc::new(std::sync::RwLock::new(HashMap::new())),
            connector_registry: None,
            bridge_registry: None,
            bridge_outbox: None,
            graph_storage: None,
            ingestion_adapter: None,
            goal_adapter: None,
            event_bus: Arc::new(EventBus::new()),
            handles,
            workspace_cache: crate::invoke::new_workspace_cache(),
        };
    }
}
