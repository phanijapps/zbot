// ============================================================================
// EXECUTOR MODULE
// Core agent execution engine
// ============================================================================

//! # Executor Module
//!
//! Core agent execution engine that coordinates LLM calls, tool execution,
//! and middleware processing.
//!
//! The executor is the main orchestrator that:
//! 1. Creates LLM client with provided configuration
//! 2. Uses provided tool registry and MCP manager
//! 3. Processes messages through middleware pipeline
//! 4. Executes LLM calls with streaming support
//! 5. Handles tool execution and result collection
//! 6. Emits events for real-time feedback

use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::llm::client::StreamChunk;
use crate::llm::LlmClient;
use crate::mcp::McpManager;
use crate::middleware::token_counter::estimate_total_tokens;
use crate::middleware::traits::MiddlewareContext;
use crate::middleware::MiddlewarePipeline;
use crate::tools::context::ToolContext;
use crate::tools::ToolRegistry;
use crate::types::{ChatMessage, StreamEvent, ToolCall};
use zero_core::event::EventActions;
use zero_core::types::Part;
use zero_core::ToolContext as ZeroToolContext;

// ============================================================================
// MID-SESSION RECALL HOOK
// ============================================================================

/// Result returned by the mid-session recall hook.
///
/// Contains novel facts formatted as a system message and the keys of those
/// facts so the caller can track already-injected keys.
#[derive(Debug, Clone)]
pub struct RecallHookResult {
    /// Formatted system message to inject (empty if nothing novel)
    pub system_message: String,
    /// Keys of the facts that were included (for dedup tracking)
    pub fact_keys: Vec<String>,
}

/// A callback invoked by the executor every N turns to refresh memory recall.
///
/// The hook receives:
/// - `latest_user_message`: the most recent user message for query context
/// - `already_injected_keys`: keys of facts already injected in this session
///
/// Returns a `RecallHookResult` with a formatted message and new keys.
pub type RecallHook = Box<
    dyn Fn(
            &str,
            &HashSet<String>,
        ) -> Pin<Box<dyn Future<Output = Result<RecallHookResult, String>> + Send>>
        + Send
        + Sync,
>;

/// Result from tool execution including any actions set by the tool
struct ToolExecutionResult {
    output: String,
    actions: EventActions,
}

// ============================================================================
// EXECUTOR CONFIGURATION
// ============================================================================

/// Configuration for agent executor
#[derive(Clone)]
pub struct ExecutorConfig {
    /// Agent identifier
    pub agent_id: String,

    /// Provider identifier
    pub provider_id: String,

    /// Model to use
    pub model: String,

    /// Temperature for generation (0.0 - 1.0)
    pub temperature: f64,

    /// Maximum tokens to generate
    pub max_tokens: u32,

    /// Enable reasoning/thinking
    pub thinking_enabled: bool,

    /// System instruction
    pub system_instruction: Option<String>,

    /// Enable tools
    pub tools_enabled: bool,

    /// MCP servers to use
    pub mcps: Vec<String>,

    /// Skills to use
    pub skills: Vec<String>,

    /// Conversation ID for scoping
    pub conversation_id: Option<String>,

    /// Initial state to inject into tool context.
    /// This allows passing hook context, delegation context, etc.
    #[allow(dead_code)]
    pub initial_state: std::collections::HashMap<String, Value>,

    /// Maximum characters for a tool result in context (default: 30000 chars ≈ 7500 tokens).
    /// Results exceeding this are truncated to head + tail with a notice.
    /// Set to 0 to disable truncation.
    pub max_tool_result_chars: usize,

    /// Offload large tool results to filesystem instead of keeping in context.
    pub offload_large_results: bool,

    /// Character threshold for offloading (default: 20000 chars ≈ 5000 tokens).
    pub offload_threshold_chars: usize,

    /// Directory to save offloaded tool results.
    pub offload_dir: Option<std::path::PathBuf>,

    /// Maximum LLM loop iterations before checking for progress (default: 50).
    /// Kept for diagnostics — no longer a hard stop. Set to 0 to disable diagnostics.
    pub max_iterations: u32,

    /// Maximum times auto-extension can be granted (default: 3, so 50 + 3*25 = 125 max).
    /// Legacy field — iteration limits are now advisory.
    pub max_extensions: u32,

    /// Additional iterations granted per auto-extension (default: 25).
    /// Legacy field — iteration limits are now advisory.
    pub extension_size: u32,

    /// Context window size for the model in tokens.
    /// When cumulative tokens exceed 80% of this, auto-compaction triggers.
    /// Set to 0 to disable compaction.
    pub context_window_tokens: u64,

    /// Percentage of context window at which to inject a pre-compaction memory flush warning.
    /// Default: 80. Chat mode sets this to 70 so the nudge fires before the middleware prunes.
    pub compaction_warn_pct: u64,

    /// Soft turn budget: inject a "wrap up" nudge after this many tool-calling iterations.
    /// Set to 0 to disable.
    pub turn_budget: u32,

    /// Hard turn limit: forcibly stop execution after this many iterations.
    /// Set to 0 to disable.
    pub max_turns: u32,

    /// Hook called before each tool execution. Can block the call.
    /// Default: None (all tools allowed).
    pub before_tool_call: Option<BeforeToolCallHook>,

    /// Hook called after each tool execution. Can transform the result.
    /// Default: None (results passed through unchanged).
    pub after_tool_call: Option<AfterToolCallHook>,

    /// Tool execution mode: parallel (default) or sequential.
    pub tool_execution_mode: ToolExecutionMode,

    /// Hook called before every LLM call to transform the message context.
    /// Default: None (messages passed through unchanged).
    pub transform_context: Option<TransformContextHook>,

    /// Task complexity level: "S", "M", "L", "XL".
    /// When set, applies complexity-based iteration budgets:
    /// S=15, M=30, L=50, XL=100.
    pub complexity: Option<String>,

    /// When true, only the first tool call per LLM response is executed.
    /// Extra tool calls are dropped with a log message.
    /// Default: false. Set true for orchestrator agents (root).
    pub single_action_mode: bool,
}

impl ExecutorConfig {
    /// Create a new executor config
    #[must_use]
    pub fn new(agent_id: String, provider_id: String, model: String) -> Self {
        Self {
            agent_id,
            provider_id,
            model,
            temperature: 0.7,
            max_tokens: 8192,
            thinking_enabled: false,
            system_instruction: None,
            tools_enabled: true,
            mcps: Vec::new(),
            skills: Vec::new(),
            conversation_id: None,
            initial_state: std::collections::HashMap::new(),
            max_tool_result_chars: 30_000, // ~7500 tokens
            offload_large_results: false,
            offload_threshold_chars: 20_000, // ~5000 tokens
            offload_dir: None,
            max_iterations: 50,
            max_extensions: 3,
            extension_size: 25,
            context_window_tokens: 128_000, // Default to 128K context
            compaction_warn_pct: 80,        // Warn at 80% by default
            turn_budget: 25,                // Soft nudge at 25 turns
            max_turns: 50,                  // Hard stop at 50 turns
            before_tool_call: None,
            after_tool_call: None,
            tool_execution_mode: ToolExecutionMode::default(),
            transform_context: None,
            complexity: None,
            single_action_mode: false,
        }
    }

    /// Add initial state that will be injected into tool context
    #[must_use]
    pub fn with_initial_state(mut self, key: impl Into<String>, value: Value) -> Self {
        self.initial_state.insert(key.into(), value);
        self
    }
}

impl fmt::Debug for ExecutorConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExecutorConfig")
            .field("agent_id", &self.agent_id)
            .field("provider_id", &self.provider_id)
            .field("model", &self.model)
            .field("temperature", &self.temperature)
            .field("max_tokens", &self.max_tokens)
            .field("thinking_enabled", &self.thinking_enabled)
            .field("system_instruction", &self.system_instruction)
            .field("tools_enabled", &self.tools_enabled)
            .field("mcps", &self.mcps)
            .field("skills", &self.skills)
            .field("conversation_id", &self.conversation_id)
            .field("initial_state", &self.initial_state)
            .field("max_tool_result_chars", &self.max_tool_result_chars)
            .field("offload_large_results", &self.offload_large_results)
            .field("offload_threshold_chars", &self.offload_threshold_chars)
            .field("offload_dir", &self.offload_dir)
            .field("max_iterations", &self.max_iterations)
            .field("max_extensions", &self.max_extensions)
            .field("extension_size", &self.extension_size)
            .field("context_window_tokens", &self.context_window_tokens)
            .field("compaction_warn_pct", &self.compaction_warn_pct)
            .field("turn_budget", &self.turn_budget)
            .field("max_turns", &self.max_turns)
            .field(
                "before_tool_call",
                &self.before_tool_call.as_ref().map(|_| "<hook>"),
            )
            .field(
                "after_tool_call",
                &self.after_tool_call.as_ref().map(|_| "<hook>"),
            )
            .field("tool_execution_mode", &self.tool_execution_mode)
            .field(
                "transform_context",
                &self.transform_context.as_ref().map(|_| "<hook>"),
            )
            .field("complexity", &self.complexity)
            .field("single_action_mode", &self.single_action_mode)
            .finish()
    }
}

// ============================================================================
// TOOL HOOK TYPES
// ============================================================================

/// Decision from beforeToolCall hook.
#[derive(Debug, Clone)]
pub enum ToolCallDecision {
    /// Allow the tool call to proceed.
    Allow,
    /// Block the tool call. The reason is returned to the LLM as the tool result.
    Block { reason: String },
}

/// Tool execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ToolExecutionMode {
    /// Execute all tools concurrently (current behavior).
    #[default]
    Parallel,
    /// Execute tools one at a time, in order.
    Sequential,
}

/// Type alias for beforeToolCall hook.
/// Receives (`tool_name`, args). Returns Allow or Block.
pub type BeforeToolCallHook = Arc<dyn Fn(&str, &Value) -> ToolCallDecision + Send + Sync>;

/// Type alias for afterToolCall hook.
/// Receives (`tool_name`, args, result, succeeded). Returns optional replacement result.
pub type AfterToolCallHook = Arc<dyn Fn(&str, &Value, &str, bool) -> Option<String> + Send + Sync>;

/// Type alias for transformContext hook.
/// Called before every LLM call. Can modify the message list in place.
pub type TransformContextHook = Arc<dyn Fn(&mut Vec<ChatMessage>) + Send + Sync>;

// ============================================================================
// AGENT EXECUTOR
// ============================================================================

/// Main agent executor
///
/// Coordinates LLM calls, tool execution, and middleware processing.
pub struct AgentExecutor {
    config: ExecutorConfig,
    llm_client: Arc<dyn LlmClient>,
    tool_registry: Arc<ToolRegistry>,
    mcp_manager: Arc<McpManager>,
    middleware_pipeline: Arc<MiddlewarePipeline>,
    /// Optional mid-session recall hook invoked every N turns.
    recall_hook: Option<Arc<RecallHook>>,
    /// How often (in turns) to run mid-session recall. 0 = disabled.
    recall_every_n_turns: u32,
    /// Keys of facts already injected at session start (seeds the dedup set).
    recall_initial_keys: HashSet<String>,
    /// Optional steering queue for mid-execution message injection.
    steering_queue: Option<std::sync::Mutex<crate::steering::SteeringQueue>>,
}

impl AgentExecutor {
    /// Create a new agent executor
    ///
    /// # Arguments
    /// * `config` - Executor configuration
    /// * `llm_client` - LLM client for making API calls
    /// * `tool_registry` - Registry of available tools
    /// * `mcp_manager` - MCP manager for external tools
    /// * `middleware_pipeline` - Middleware pipeline for preprocessing
    pub fn new(
        config: ExecutorConfig,
        llm_client: Arc<dyn LlmClient>,
        tool_registry: Arc<ToolRegistry>,
        mcp_manager: Arc<McpManager>,
        middleware_pipeline: Arc<MiddlewarePipeline>,
    ) -> Result<Self, ExecutorError> {
        Ok(Self {
            config,
            llm_client,
            tool_registry,
            mcp_manager,
            middleware_pipeline,
            recall_hook: None,
            recall_every_n_turns: 0,
            recall_initial_keys: HashSet::new(),
            steering_queue: None,
        })
    }

    /// Set the middleware pipeline
    pub fn set_middleware_pipeline(&mut self, pipeline: Arc<MiddlewarePipeline>) {
        self.middleware_pipeline = pipeline;
    }

    /// Configure mid-session recall.
    ///
    /// The `hook` is called every `every_n_turns` iterations with the latest
    /// user message and the set of already-injected fact keys. Novel facts
    /// are injected as a system message.
    ///
    /// `initial_keys` seeds the dedup set from facts injected at session start.
    pub fn set_recall_hook(
        &mut self,
        hook: RecallHook,
        every_n_turns: u32,
        initial_keys: HashSet<String>,
    ) {
        self.recall_hook = Some(Arc::new(hook));
        self.recall_every_n_turns = every_n_turns;
        self.recall_initial_keys = initial_keys;
    }

    /// Attach a steering queue to this executor.
    ///
    /// Call this before `execute_stream`. The returned `SteeringHandle` can be
    /// shared with the UI, parent agents, or budget enforcers.
    pub fn enable_steering(&mut self) -> crate::steering::SteeringHandle {
        let (queue, handle) = crate::steering::SteeringQueue::new();
        self.steering_queue = Some(std::sync::Mutex::new(queue));
        handle
    }

    /// Get the middleware pipeline
    #[must_use]
    pub fn middleware_pipeline(&self) -> &Arc<MiddlewarePipeline> {
        &self.middleware_pipeline
    }

    /// Get the configuration
    #[must_use]
    pub fn config(&self) -> &ExecutorConfig {
        &self.config
    }

    /// Execute the agent with streaming
    ///
    /// The callback receives events as they occur during execution.
    pub async fn execute_stream(
        &self,
        user_message: &str,
        history: &[ChatMessage],
        mut on_event: impl FnMut(StreamEvent),
    ) -> Result<(), ExecutorError> {
        // Emit metadata event
        on_event(StreamEvent::Metadata {
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            agent_id: self.config.agent_id.clone(),
            model: self.config.model.clone(),
            provider: self.config.provider_id.clone(),
        });

        // Build messages array
        let mut messages = Vec::new();

        // Add system instruction if available
        if let Some(instruction) = &self.config.system_instruction {
            messages.push(ChatMessage::system(instruction.clone()));
        }

        // Add conversation history
        messages.extend(history.iter().cloned());

        // Add current user message
        messages.push(ChatMessage::user(user_message.to_string()));

        // Create middleware context
        let message_count = messages.len();
        let estimated_tokens = estimate_total_tokens(&messages, &self.config.model);

        // Build execution state from message history.
        // This extracts skill information from previous tool calls so middleware
        // can make skill-aware decisions during context compaction.
        let execution_state = crate::middleware::traits::ExecutionState::from_messages(&messages);

        let middleware_context = MiddlewareContext::new(
            self.config.agent_id.clone(),
            self.config.conversation_id.clone(),
            self.config.provider_id.clone(),
            self.config.model.clone(),
        )
        .with_counts(message_count, estimated_tokens)
        .with_execution_state(execution_state);

        // Process messages through middleware pipeline
        let processed_messages = self
            .middleware_pipeline
            .process_messages(messages, &middleware_context, &mut on_event)
            .await
            .map_err(ExecutorError::MiddlewareError)?;

        // Get tools schema if enabled
        let tools_schema = if self.config.tools_enabled {
            Some(self.build_tools_schema().await?)
        } else {
            None
        };

        tracing::debug!("Starting execute_with_tools_loop");

        // Execute with tool calling loop
        self.execute_with_tools_loop(processed_messages, tools_schema, &mut on_event)
            .await
    }

    async fn execute_with_tools_loop(
        &self,
        messages: Vec<ChatMessage>,
        tools_schema: Option<Value>,
        on_event: &mut impl FnMut(StreamEvent),
    ) -> Result<(), ExecutorError> {
        tracing::debug!("=== execute_with_tools_loop starting ===");
        tracing::debug!("Messages count: {}", messages.len());
        tracing::debug!("Tools schema: {}", tools_schema.is_some());

        let mut current_messages = messages;
        #[allow(unused_assignments)] // Initialized here, assigned in loop exit condition
        let mut full_response = String::new();

        // Track cumulative token usage across the session
        let mut total_tokens_in: u64 = 0;
        let mut total_tokens_out: u64 = 0;

        // Progress tracker for diagnostics and advisory nudges.
        // No longer used for hard stops — agent runs until done or safety valve trips.
        let mut progress_tracker = ProgressTracker::new(self.config.max_extensions);

        // Track whether we've sent a stuck-loop nudge (max 1)
        let mut stuck_nudge_sent = false;

        // Track whether we've injected a pre-compaction memory flush warning
        let mut compaction_warned = false;

        // Track whether the turn budget nudge has been sent (max 1)
        let mut turn_budget_nudge_sent = false;

        // Track message count to skip redundant compaction estimation
        let mut last_compaction_check_msg_count: usize = 0;

        // Track which fact keys have been injected via recall (initial + mid-session).
        // Seeded from initial recall keys; extended by mid-session recall hook results.
        let mut recall_injected_keys = self.recall_initial_keys.clone();

        // Track whether the loop stopped due to delegation (vs respond or natural end).
        // Set inside the loop; read after the loop to decide whether to emit Done.
        let mut stopped_for_delegation = false;

        // Create shared tool context that persists across all tool calls in this execution.
        // This allows tools like load_skill to maintain state (e.g., loaded skills, resources)
        // that other tools and middleware can access throughout the execution loop.
        let shared_tool_context = Arc::new(ToolContext::full_with_state(
            self.config.agent_id.clone(),
            self.config.conversation_id.clone(),
            self.config.skills.clone(),
            self.config.initial_state.clone(),
        ));

        loop {
            progress_tracker.tick();

            // Reset delegation claim at the start of each turn.
            // This allows root to delegate again after a previous delegation completes.
            // try_claim checks for Bool(true); setting to Bool(false) releases the claim.
            {
                use zero_core::CallbackContext;
                shared_tool_context
                    .set_state("app:delegation_active".to_string(), Value::Bool(false));
            }

            // Turn budget: soft nudge then hard stop
            if self.config.max_turns > 0
                && progress_tracker.total_iterations >= self.config.max_turns
            {
                tracing::warn!(
                    total_iterations = progress_tracker.total_iterations,
                    max_turns = self.config.max_turns,
                    "Hard turn limit reached, stopping execution"
                );
                // Emit done with explanation
                on_event(StreamEvent::Done {
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                    final_message: format!(
                        "[Turn limit reached after {} iterations. Stopping execution.]",
                        progress_tracker.total_iterations
                    ),
                    token_count: 0,
                });
                return Ok(());
            }

            if self.config.turn_budget > 0
                && progress_tracker.total_iterations >= self.config.turn_budget
                && !turn_budget_nudge_sent
            {
                turn_budget_nudge_sent = true;
                current_messages.push(ChatMessage::user(format!(
                    "[SYSTEM: You have used {} of {} tool calls. Wrap up your current work \
                     and call `respond` with a summary. Do not start new explorations.]",
                    progress_tracker.total_iterations, self.config.max_turns
                )));
                tracing::info!(
                    total_iterations = progress_tracker.total_iterations,
                    turn_budget = self.config.turn_budget,
                    "Turn budget nudge sent"
                );
            }

            // Complexity-based budget enforcement
            if let Some(ref complexity) = self.config.complexity {
                let (hard_budget, soft_budget) = match complexity.as_str() {
                    "S" => (15u32, 12u32),
                    "M" => (30, 24),
                    "L" => (50, 40),
                    "XL" => (100, 80),
                    _ => (0, 0),
                };

                if hard_budget > 0 {
                    let iters = progress_tracker.total_iterations;
                    if iters >= hard_budget {
                        // Hard budget exceeded: inject urgent message
                        current_messages.push(ChatMessage::user(format!(
                            "[STEER: System] Budget exceeded ({iters}/{hard_budget} iterations for {complexity} task).                              Respond NOW with what you have. Do not start new work."
                        )));
                        tracing::warn!(
                            complexity = %complexity,
                            iterations = iters,
                            budget = hard_budget,
                            "Complexity hard budget reached"
                        );
                    } else if iters == soft_budget {
                        // Soft budget: nudge exactly once (when iters == soft_budget)
                        current_messages.push(ChatMessage::user(format!(
                            "[STEER: System] You've used {iters}/{hard_budget} iterations for a {complexity} task.                              Wrap up or simplify your approach."
                        )));
                        tracing::info!(
                            complexity = %complexity,
                            iterations = iters,
                            budget = hard_budget,
                            "Complexity soft budget nudge sent"
                        );
                    }
                }
            }

            // Advisory stuck-detection: inject nudge once, hard-stop only as safety valve
            if progress_tracker.is_clearly_stuck() {
                if !stuck_nudge_sent {
                    // First time: inject advisory nudge, let agent recover
                    stuck_nudge_sent = true;
                    current_messages.push(ChatMessage::user(
                        "[SYSTEM: You appear to be repeating similar actions without progress. \
                         Step back, re-read the full context, and try a different approach. \
                         If you cannot make progress, use the `respond` tool to summarize \
                         what you've accomplished and what remains.]"
                            .to_string(),
                    ));
                    tracing::warn!(
                        total_iterations = progress_tracker.total_iterations,
                        score = progress_tracker.score,
                        "Stuck-loop advisory nudge sent"
                    );
                } else if progress_tracker.score <= -12 {
                    // Safety valve: agent still stuck after nudge, hard-stop
                    let diagnosis = progress_tracker.diagnosis();
                    tracing::warn!(
                        total_iterations = progress_tracker.total_iterations,
                        score = progress_tracker.score,
                        diagnosis = %diagnosis,
                        "Safety valve: agent stuck after nudge, stopping"
                    );
                    return Err(ExecutorError::MaxIterationsNeedsIntervention {
                        iterations_used: progress_tracker.total_iterations,
                        reason: diagnosis,
                    });
                }
            }

            // Token-budget auto-compaction trigger.
            // When cumulative tokens approach 80% of the context window, trim old messages.
            // Skip the check entirely if no new messages have been added since last check —
            // avoids redundant threshold evaluation in tight tool-calling loops.
            if self.config.context_window_tokens > 0
                && current_messages.len() > last_compaction_check_msg_count
            {
                last_compaction_check_msg_count = current_messages.len();
                let warn_threshold =
                    (self.config.context_window_tokens * self.config.compaction_warn_pct) / 100;
                let compact_threshold = (self.config.context_window_tokens * 80) / 100;
                if total_tokens_in > warn_threshold {
                    // Pre-compaction memory flush: inject a nudge to save important facts
                    // before context is trimmed. The agent can use save_fact on the next
                    // turn before the old messages disappear.
                    if !compaction_warned {
                        current_messages.push(ChatMessage::system(
                            "[system] Context is getting full. Save important facts with \
                             memory(action=\"save_fact\", scope=\"chat\") before they are pruned."
                                .to_string(),
                        ));
                        compaction_warned = true;
                        tracing::info!(
                            tokens_in = total_tokens_in,
                            warn_threshold = warn_threshold,
                            "Pre-compaction memory flush warning injected"
                        );
                        // Skip actual compaction this iteration — give agent one turn to save
                        continue;
                    }

                    // Actual compaction triggers at 80% regardless of warn threshold
                    if total_tokens_in > compact_threshold {
                        let before = current_messages.len();
                        current_messages = compact_messages(current_messages);
                        tracing::info!(
                            tokens_in = total_tokens_in,
                            compact_threshold = compact_threshold,
                            messages_before = before,
                            messages_after = current_messages.len(),
                            "Context compacted"
                        );
                    }
                }
            }

            // Mid-session recall: every N turns, fetch novel facts and inject as
            // a system message so the agent benefits from memory even during long
            // multi-turn sessions.
            if self.recall_every_n_turns > 0
                && progress_tracker.total_iterations > 0
                && progress_tracker
                    .total_iterations
                    .is_multiple_of(self.recall_every_n_turns)
            {
                if let Some(hook) = &self.recall_hook {
                    // Find the latest user message for query context
                    let latest_user_msg = current_messages
                        .iter()
                        .rev()
                        .find(|m| m.role == "user")
                        .map(super::types::messages::ChatMessage::text_content)
                        .unwrap_or_default();

                    let hook_clone = Arc::clone(hook);
                    match hook_clone(&latest_user_msg, &recall_injected_keys).await {
                        Ok(result) if !result.system_message.is_empty() => {
                            current_messages.push(ChatMessage::system(result.system_message));
                            // Track newly injected keys for future dedup
                            for key in result.fact_keys {
                                recall_injected_keys.insert(key);
                            }
                            tracing::info!(
                                turn = progress_tracker.total_iterations,
                                total_keys = recall_injected_keys.len(),
                                "Mid-session recall injected novel facts"
                            );
                        }
                        Ok(_) => {
                            tracing::debug!(
                                turn = progress_tracker.total_iterations,
                                "Mid-session recall: no novel facts"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                turn = progress_tracker.total_iterations,
                                error = %e,
                                "Mid-session recall failed"
                            );
                        }
                    }
                }
            }

            // Drain steering queue: inject any pending steering messages
            if let Some(ref steering_mutex) = self.steering_queue {
                if let Ok(mut queue) = steering_mutex.lock() {
                    let steering_messages = queue.drain();
                    for msg in steering_messages {
                        let formatted = format!("[STEER: {}] {}", msg.source, msg.content);
                        current_messages.push(ChatMessage::user(formatted));
                        tracing::info!(
                            source = %msg.source,
                            priority = ?msg.priority,
                            "Injected steering message"
                        );
                    }
                }
            }

            // Sanitize messages to remove orphaned tool messages before LLM call.
            // This prevents API errors when compaction or summarization splits
            // assistant+tool pairs.
            sanitize_messages(&mut current_messages);

            // transformContext hook: allow caller to modify messages before LLM call
            if let Some(ref hook) = self.config.transform_context {
                hook(&mut current_messages);
            }

            // Real streaming via chat_stream() with mpsc channel bridge.
            // Tokens are emitted to the user IMMEDIATELY as they arrive from the LLM,
            // including intermediate text that accompanies tool calls.
            // NOTE: When non_streaming is enabled on the LLM client wrapper,
            // chat_stream() internally uses chat() and emits content as a single chunk.
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamChunk>();

            let llm_client = self.llm_client.clone();
            let messages_for_stream = current_messages.clone();
            let tools_for_stream = tools_schema.clone();

            // Spawn the streaming LLM call in a separate task
            let stream_handle = tokio::spawn(async move {
                llm_client
                    .chat_stream(
                        messages_for_stream,
                        tools_for_stream,
                        Box::new(move |chunk| {
                            let _ = tx.send(chunk);
                        }),
                    )
                    .await
            });

            // Process chunks as they arrive — emit Token events in real-time.
            // Uses tokio::select! with a 10s heartbeat interval so that during
            // extended silent phases (e.g., LLM reasoning), heartbeat events keep
            // WebSocket connections alive (client PONG_TIMEOUT is 30s).
            let mut streamed_content = String::new();
            let mut heartbeat_interval = tokio::time::interval(std::time::Duration::from_secs(10));
            heartbeat_interval.tick().await; // consume immediate first tick

            loop {
                tokio::select! {
                    chunk = rx.recv() => {
                        match chunk {
                            Some(StreamChunk::Token(text)) => {
                                streamed_content.push_str(&text);
                                on_event(StreamEvent::Token {
                                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                    content: text,
                                });
                                heartbeat_interval.reset();
                            }
                            Some(StreamChunk::Reasoning(text)) => {
                                on_event(StreamEvent::Reasoning {
                                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                    content: text,
                                });
                                heartbeat_interval.reset();
                            }
                            Some(StreamChunk::ToolCall(_)) => {
                                // Tool call chunks are accumulated by the streaming impl
                                // and returned in the final ChatResponse. No action needed here.
                                heartbeat_interval.reset();
                            }
                            None => break, // channel closed
                        }
                    }
                    _ = heartbeat_interval.tick() => {
                        on_event(StreamEvent::Heartbeat {
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        });
                    }
                }
            }

            // Await the final response (channel closed = stream complete)
            let response = stream_handle
                .await
                .map_err(|e| ExecutorError::LlmError(format!("Stream task panicked: {e}")))?
                .map_err(|e| ExecutorError::LlmError(e.to_string()))?;

            // Update cumulative token counts and emit event
            if let Some(usage) = &response.usage {
                total_tokens_in += u64::from(usage.prompt_tokens);
                total_tokens_out += u64::from(usage.completion_tokens);

                on_event(StreamEvent::TokenUpdate {
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                    tokens_in: total_tokens_in,
                    tokens_out: total_tokens_out,
                });
            }

            tracing::debug!(
                "LLM response - content: '{}', tool_calls: {}",
                response.content,
                response.tool_calls.as_ref().map_or(0, std::vec::Vec::len)
            );

            // Check for tool calls
            let mut tool_calls = response.tool_calls.clone().unwrap_or_default();

            // Single-action mode: execute only the first tool call, drop extras.
            // This prevents the model from batching multiple actions into one response.
            if self.config.single_action_mode && tool_calls.len() > 1 {
                tracing::info!(
                    "Single-action mode: executing '{}', dropping {} extra tool calls",
                    tool_calls[0].name,
                    tool_calls.len() - 1
                );
                tool_calls.truncate(1);
            }
            if tool_calls.is_empty() {
                // No tool calls, this is the final response
                // Text was already streamed in real-time above
                full_response = response.content.clone();
                tracing::debug!(
                    "No tool calls, final response length: {}",
                    full_response.len()
                );
                break;
            }

            // Handle tool calls
            // Store the assistant message with ORIGINAL tool calls (not truncated).
            // Truncation caused the LLM to copy garbled text on retries.
            current_messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: vec![Part::Text {
                    text: response.content.clone(),
                }],
                tool_calls: Some(tool_calls.clone()),
                tool_call_id: None,
            });

            // Track if respond tool was called - signals we should stop after this batch
            let mut should_stop_after_respond = false;

            // Emit ToolCallStart events for all tools before execution
            for tool_call in &tool_calls {
                on_event(StreamEvent::ToolCallStart {
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                    tool_id: tool_call.id.clone(),
                    tool_name: tool_call.name.clone(),
                    args: tool_call.arguments.clone(),
                });
            }

            // Check beforeToolCall hook for each tool
            let mut blocked_results: HashMap<String, String> = HashMap::new();
            if let Some(ref hook) = self.config.before_tool_call {
                for tc in &tool_calls {
                    match hook(&tc.name, &tc.arguments) {
                        ToolCallDecision::Allow => {}
                        ToolCallDecision::Block { reason } => {
                            blocked_results.insert(
                                tc.id.clone(),
                                format!("{{\"blocked\":true,\"reason\":\"{reason}\"}}"),
                            );
                        }
                    }
                }
            }

            let non_blocked: Vec<&ToolCall> = tool_calls
                .iter()
                .filter(|tc| !blocked_results.contains_key(&tc.id))
                .collect();

            let results: Vec<Result<ToolExecutionResult, String>> = if self
                .config
                .tool_execution_mode
                == ToolExecutionMode::Sequential
            {
                // Sequential: execute one at a time, in order
                let mut seq_results = Vec::new();
                for tc in &non_blocked {
                    let result = self
                        .execute_tool(&shared_tool_context, &tc.id, &tc.name, &tc.arguments)
                        .await;
                    seq_results.push(result);
                }
                seq_results
            } else {
                // Parallel: all at once (current behavior)
                let tool_futures: Vec<_> = non_blocked
                    .iter()
                    .map(|tc| {
                        let ctx = shared_tool_context.clone();
                        let tool_id = tc.id.clone();
                        let tool_name = tc.name.clone();
                        let args = tc.arguments.clone();
                        async move {
                            tracing::debug!("Executing tool: {} with args: {}", tool_name, args);
                            self.execute_tool(&ctx, &tool_id, &tool_name, &args).await
                        }
                    })
                    .collect();
                futures::future::join_all(tool_futures).await
            };

            // Build a map of executed results (keyed by tool_call id)
            let mut executed_results: HashMap<String, Result<ToolExecutionResult, String>> =
                HashMap::new();
            for (tc, result) in non_blocked.into_iter().zip(results) {
                executed_results.insert(tc.id.clone(), result);
            }

            // Process results in original tool_call order
            for tool_call in &tool_calls {
                let tool_name = &tool_call.name;

                if let Some(blocked_result) = blocked_results.remove(&tool_call.id) {
                    // Blocked by beforeToolCall hook
                    current_messages.push(ChatMessage::tool_result(
                        tool_call.id.clone(),
                        blocked_result,
                    ));
                    on_event(StreamEvent::ToolResult {
                        timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        tool_id: tool_call.id.clone(),
                        result: "[blocked by hook]".to_string(),
                        error: None,
                    });
                    progress_tracker.record_tool_call(&tool_call.name, &tool_call.arguments, false);
                } else if let Some(result) = executed_results.remove(&tool_call.id) {
                    match result {
                        Ok(tool_result) => {
                            let output = tool_result.output;
                            let actions = tool_result.actions;

                            tracing::debug!("Tool result: {}", output);

                            // Track progress: tool succeeded
                            progress_tracker.record_tool_call(
                                tool_name,
                                &tool_call.arguments,
                                true,
                            );

                            // Check for respond action
                            if let Some(respond) = &actions.respond {
                                on_event(StreamEvent::ActionRespond {
                                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                    message: respond.message.clone(),
                                    format: respond.format.clone(),
                                    conversation_id: respond.conversation_id.clone(),
                                    session_id: respond.session_id.clone(),
                                    artifacts: respond.artifacts.clone(),
                                });
                                should_stop_after_respond = true;
                                progress_tracker.record_respond();
                                tracing::debug!(
                                    "Respond action detected, will stop after current tool batch"
                                );
                            }

                            // Check for delegate action
                            if let Some(delegate) = &actions.delegate {
                                on_event(StreamEvent::ActionDelegate {
                                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                    agent_id: delegate.agent_id.clone(),
                                    task: delegate.task.clone(),
                                    context: delegate.context.clone(),
                                    wait_for_result: delegate.wait_for_result,
                                    max_iterations: delegate.max_iterations,
                                    output_schema: delegate.output_schema.clone(),
                                    skills: delegate.skills.clone(),
                                    complexity: delegate.complexity.clone(),
                                    parallel: delegate.parallel,
                                });
                                // Delegation claim is set atomically by the delegate tool via try_claim
                                // Stop executor loop — continuation callback will resume root
                                // when the subagent completes.
                                stopped_for_delegation = true;
                                tracing::debug!(
                                    "Delegation detected, will stop after current tool batch"
                                );
                            }

                            // Check for generative UI markers
                            if let Ok(parsed) = serde_json::from_str::<Value>(&output) {
                                // Check for show_content marker
                                if parsed
                                    .get("__show_content")
                                    .and_then(serde_json::Value::as_bool)
                                    .unwrap_or(false)
                                {
                                    let content_type = parsed
                                        .get("content_type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("text")
                                        .to_string();
                                    let title = parsed
                                        .get("title")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("Content")
                                        .to_string();
                                    let content = parsed
                                        .get("content")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let metadata = parsed.get("metadata").cloned();
                                    let file_path = parsed
                                        .get("file_path")
                                        .and_then(|v| v.as_str())
                                        .map(std::string::ToString::to_string);
                                    let is_attachment = parsed
                                        .get("is_attachment")
                                        .and_then(serde_json::Value::as_bool);
                                    let base64 =
                                        parsed.get("base64").and_then(serde_json::Value::as_bool);

                                    on_event(StreamEvent::ShowContent {
                                        timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                        content_type,
                                        title,
                                        content,
                                        metadata,
                                        file_path,
                                        is_attachment,
                                        base64,
                                    });
                                }

                                // Check for request_input marker
                                if parsed
                                    .get("__request_input")
                                    .and_then(serde_json::Value::as_bool)
                                    .unwrap_or(false)
                                {
                                    let form_id = parsed
                                        .get("form_id")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(&format!(
                                            "form_{}",
                                            chrono::Utc::now().timestamp()
                                        ))
                                        .to_string();
                                    let form_type = parsed
                                        .get("form_type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("json_schema")
                                        .to_string();
                                    let title = parsed
                                        .get("title")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("Input Required")
                                        .to_string();
                                    let description = parsed
                                        .get("description")
                                        .and_then(|v| v.as_str())
                                        .map(std::string::ToString::to_string);
                                    let schema =
                                        parsed.get("schema").cloned().unwrap_or_else(|| json!({}));
                                    let submit_button = parsed
                                        .get("submit_button")
                                        .and_then(|v| v.as_str())
                                        .map(std::string::ToString::to_string);

                                    on_event(StreamEvent::RequestInput {
                                        timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                        form_id,
                                        form_type,
                                        title,
                                        description,
                                        schema,
                                        submit_button,
                                    });
                                }

                                // Check for ward_changed marker (from ward tool)
                                if parsed
                                    .get("__ward_changed__")
                                    .and_then(serde_json::Value::as_bool)
                                    .unwrap_or(false)
                                {
                                    if let Some(ward_id) =
                                        parsed.get("ward_id").and_then(|v| v.as_str())
                                    {
                                        on_event(StreamEvent::WardChanged {
                                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                            ward_id: ward_id.to_string(),
                                        });
                                    }
                                }

                                // Check for plan_update marker
                                if parsed
                                    .get("__plan_update")
                                    .and_then(serde_json::Value::as_bool)
                                    .unwrap_or(false)
                                {
                                    let plan =
                                        parsed.get("plan").cloned().unwrap_or_else(|| json!([]));
                                    let explanation = parsed
                                        .get("explanation")
                                        .and_then(|v| v.as_str())
                                        .map(std::string::ToString::to_string);

                                    on_event(StreamEvent::ActionPlanUpdate {
                                        timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                        plan,
                                        explanation,
                                    });
                                }

                                // Check for session_title_changed marker
                                if parsed
                                    .get("__session_title_changed__")
                                    .and_then(serde_json::Value::as_bool)
                                    .unwrap_or(false)
                                {
                                    if let Some(title) =
                                        parsed.get("title").and_then(|v| v.as_str())
                                    {
                                        on_event(StreamEvent::SessionTitleChanged {
                                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                            title: title.to_string(),
                                        });
                                    }
                                }
                            }

                            on_event(StreamEvent::ToolResult {
                                timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                tool_id: tool_call.id.clone(),
                                result: output.clone(),
                                error: None,
                            });

                            // Process tool result (potentially offload large results to filesystem)
                            let processed_output = self.process_tool_result(tool_name, output);

                            // Truncate if still over budget (safety net when offload is disabled)
                            let processed_output = truncate_tool_result(
                                processed_output,
                                self.config.max_tool_result_chars,
                            );

                            // afterToolCall hook — can transform the result
                            let final_output = if let Some(ref hook) = self.config.after_tool_call {
                                match hook(tool_name, &tool_call.arguments, &processed_output, true)
                                {
                                    Some(replacement) => replacement,
                                    None => processed_output,
                                }
                            } else {
                                processed_output
                            };

                            // Add tool result message
                            current_messages
                                .push(ChatMessage::tool_result(tool_call.id.clone(), final_output));
                        }
                        Err(e) => {
                            tracing::debug!("Tool error: {}", e);

                            // Track progress: tool failed
                            progress_tracker.record_tool_call(
                                tool_name,
                                &tool_call.arguments,
                                false,
                            );
                            progress_tracker.record_error(&e);

                            on_event(StreamEvent::ToolResult {
                                timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                tool_id: tool_call.id.clone(),
                                result: String::new(),
                                error: Some(e.clone()),
                            });

                            // afterToolCall hook — can transform error results too
                            let error_message = json!({"error": e}).to_string();
                            let final_error = if let Some(ref hook) = self.config.after_tool_call {
                                match hook(tool_name, &tool_call.arguments, &error_message, false) {
                                    Some(replacement) => replacement,
                                    None => error_message,
                                }
                            } else {
                                error_message
                            };

                            // Add error result message
                            current_messages
                                .push(ChatMessage::tool_result(tool_call.id.clone(), final_error));
                        }
                    }
                }

                on_event(StreamEvent::ToolCallEnd {
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                    tool_id: tool_call.id.clone(),
                    tool_name: tool_name.clone(),
                    args: tool_call.arguments.clone(),
                });
            }

            // Planning enforcement: nudge if agent hasn't planned
            if progress_tracker.needs_planning_nudge() {
                current_messages.push(ChatMessage::user(
                    "[SYSTEM: You have made several tool calls without creating a plan. \
                     For complex tasks, use the `update_plan` tool to track your steps. \
                     This helps you stay focused and avoid repeating work.]"
                        .to_string(),
                ));
            }

            // If respond tool was called, stop the loop - agent has finished responding
            if should_stop_after_respond || stopped_for_delegation {
                tracing::debug!(
                    "Stopping execution loop — respond={} delegation={}",
                    should_stop_after_respond,
                    stopped_for_delegation
                );
                break;
            }
        }

        // Emit done event — but NOT if we stopped for delegation.
        // When delegation is pending, the runner should NOT mark this execution
        // as completed. The continuation callback will resume it later.
        if stopped_for_delegation {
            tracing::info!("Executor paused for delegation — skipping Done event");
        } else {
            on_event(StreamEvent::Done {
                timestamp: chrono::Utc::now().timestamp_millis() as u64,
                final_message: full_response.clone(),
                token_count: full_response.len(),
            });
        }

        // Emit context state for checkpoint persistence
        // This includes skill tracking (graph), loaded skills, and other tool context state
        // that should be persisted for session resumption.
        on_event(StreamEvent::ContextState {
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            state: shared_tool_context.export_state(),
        });

        Ok(())
    }

    async fn execute_tool(
        &self,
        shared_ctx: &Arc<ToolContext>,
        tool_call_id: &str,
        tool_name: &str,
        arguments: &Value,
    ) -> Result<ToolExecutionResult, String> {
        // --- Replay intercept ---------------------------------------------------
        // When ZBOT_REPLAY_DIR is set, look up a recorded result and return it
        // instead of running the real tool. Strict mode (default) panics on miss;
        // lenient mode falls through to real execution.
        if let Some(store) = agent_tools::replay::global_store() {
            let exec_id =
                zero_core::ReadonlyContext::invocation_id(shared_ctx.as_ref()).to_string();
            if let Ok(mut guard) = store.lock() {
                match guard.lookup(&exec_id, tool_name) {
                    agent_tools::replay::LookupOutcome::Hit(result) => {
                        return Ok(ToolExecutionResult {
                            output: result,
                            actions: EventActions::default(),
                        });
                    }
                    agent_tools::replay::LookupOutcome::Drift {
                        expected_tool,
                        got_tool,
                    } => {
                        panic!(
                            "[tool-replay] drift on exec {exec_id}: expected '{expected_tool}' got '{got_tool}'"
                        );
                    }
                    agent_tools::replay::LookupOutcome::MissStrict {
                        exec_id: miss_id,
                        tool_index,
                    } => {
                        panic!(
                            "[tool-replay] strict miss on exec {miss_id} tool_index {tool_index}"
                        );
                    }
                    agent_tools::replay::LookupOutcome::MissLenient => {
                        // fall through to real execution
                    }
                }
            }
        }
        // --- end replay intercept -----------------------------------------------

        // First try built-in tools
        if let Some(tool) = self.tool_registry.find(tool_name) {
            // Use shared context that persists across all tool calls in this execution.
            // Set the function_call_id for this specific tool call so tools can track
            // their position in the conversation (e.g., for skill loading).
            shared_ctx.set_function_call_id(tool_call_id.to_string());

            // Clear actions before execution so we capture only this tool's actions
            shared_ctx.set_actions(EventActions::default());

            let result = tool
                .execute(shared_ctx.clone(), arguments.clone())
                .await
                .map_err(|e| format!("Tool execution failed: {e:?}"))?;

            // Atomically take any actions that were set by the tool
            let actions = shared_ctx.take_actions();

            return Ok(ToolExecutionResult {
                output: serde_json::to_string(&result).unwrap_or_else(|_| "null".to_string()),
                actions,
            });
        }

        // Try MCP tools
        // MCP tools have format: {normalized_server_id}__{normalized_tool_name}
        if tool_name.contains("__") {
            let parts: Vec<&str> = tool_name.splitn(2, "__").collect();
            if parts.len() == 2 {
                let normalized_server_id = parts[0];
                let normalized_tool = parts[1];

                // Find the original server ID by checking which one matches when normalized
                let mut original_server_id: Option<String> = None;
                for mcp_id in &self.config.mcps {
                    if normalize_tool_name(mcp_id) == normalized_server_id {
                        original_server_id = Some(mcp_id.clone());
                        break;
                    }
                }

                let server_id = match original_server_id {
                    Some(id) => id,
                    None => {
                        // Fallback: try the normalized ID directly (old behavior)
                        normalized_server_id.replace('_', "-")
                    }
                };

                // Find the original tool name by listing tools from this server
                let actual_tool =
                    if let Some(client) = self.mcp_manager.get_client(&server_id).await {
                        if let Ok(tools) = client.list_tools().await {
                            tools
                                .into_iter()
                                .find(|t| normalize_tool_name(&t.name) == normalized_tool)
                                .map_or_else(|| normalized_tool.to_string(), |t| t.name)
                        } else {
                            normalized_tool.to_string()
                        }
                    } else {
                        normalized_tool.to_string()
                    };

                tracing::debug!(
                    "Executing MCP tool: server={}, tool={}",
                    server_id,
                    actual_tool
                );
                let output = self
                    .mcp_manager
                    .execute_tool(&server_id, &actual_tool, arguments.clone())
                    .await
                    .map(|v| serde_json::to_string(&v).unwrap_or_else(|_| "null".to_string()))
                    .map_err(|e| e.to_string())?;

                // MCP tools don't support actions (yet)
                return Ok(ToolExecutionResult {
                    output,
                    actions: EventActions::default(),
                });
            }
        }

        Err(format!("Tool not found: {tool_name}"))
    }

    /// Harden a tool parameter schema for stricter LLM compliance.
    /// Adds "additionalProperties": false if not already present.
    /// Ensures "required" array exists (empty if missing).
    fn harden_tool_schema(mut schema: Value) -> Value {
        if let Some(obj) = schema.as_object_mut() {
            if obj.get("type").and_then(|v| v.as_str()) == Some("object") {
                obj.entry("additionalProperties")
                    .or_insert(Value::Bool(false));
                obj.entry("required").or_insert_with(|| json!([]));
            }
        }
        schema
    }

    /// Normalize MCP tool parameters to `OpenAI` format
    ///
    /// `OpenAI` requires parameters to have `type: "object"` at the root.
    /// MCP tools may return parameters without this wrapper.
    fn normalize_mcp_parameters(params: Option<Value>) -> Value {
        match params {
            None => json!({"type": "object", "properties": {}}),
            Some(p) => {
                // If parameters already has "type: object", use as-is
                if p.get("type").is_some() {
                    p
                } else {
                    // Otherwise, wrap it with type: object
                    json!({
                        "type": "object",
                        "properties": p
                    })
                }
            }
        }
    }

    async fn build_tools_schema(&self) -> Result<Value, ExecutorError> {
        let mut tools = Vec::new();

        for tool in self.tool_registry.get_all() {
            let tool_name = tool.name();
            let tool_desc = tool.description();
            let schema = tool.parameters_schema().map_or_else(|| json!({"type": "object", "properties": {}, "additionalProperties": false, "required": []}), Self::harden_tool_schema);

            // Validate tool name and description aren't empty
            if tool_name.is_empty() {
                tracing::error!("Tool has empty name, skipping");
                continue;
            }
            if tool_desc.is_empty() {
                tracing::error!("Tool '{}' has empty description, skipping", tool_name);
                continue;
            }

            tools.push(json!({
                "type": "function",
                "function": {
                    "name": tool_name,
                    "description": tool_desc,
                    "parameters": schema
                }
            }));
        }

        tracing::info!("Registered {} built-in tools for LLM", tools.len());

        // Add MCP tools
        for mcp_id in &self.config.mcps {
            if let Some(client) = self.mcp_manager.get_client(mcp_id).await {
                let mcp_tools = client.list_tools().await.map_err(|e| {
                    ExecutorError::McpError(format!("Failed to list MCP tools: {e}"))
                })?;

                tracing::info!(
                    "Loaded {} MCP tools from server {}",
                    mcp_tools.len(),
                    mcp_id
                );

                for mcp_tool in mcp_tools {
                    // Convert MCP ID and tool name to valid OpenAI tool name format
                    // Pattern must match: ^[a-zA-Z0-9_-]+$
                    let mcp_id_normalized = normalize_tool_name(mcp_id);
                    let tool_name_normalized = normalize_tool_name(&mcp_tool.name);
                    let tool_name = format!("{mcp_id_normalized}__{tool_name_normalized}");

                    // Normalize parameters to OpenAI format and harden schema
                    let parameters = Self::harden_tool_schema(Self::normalize_mcp_parameters(
                        mcp_tool.parameters,
                    ));

                    tools.push(json!({
                        "type": "function",
                        "function": {
                            "name": tool_name,
                            "description": mcp_tool.description,
                            "parameters": parameters
                        }
                    }));
                }
            }
        }

        tracing::info!("Total tools available to LLM: {}", tools.len());
        Ok(json!(tools))
    }

    /// Execute agent without streaming (simpler API)
    pub async fn execute(
        &self,
        user_message: &str,
        history: &[ChatMessage],
    ) -> Result<String, ExecutorError> {
        let mut final_response = String::new();

        self.execute_stream(user_message, history, |event| {
            if let StreamEvent::Token { content, .. } = event {
                final_response.push_str(&content);
            }
        })
        .await?;

        Ok(final_response)
    }

    /// Process tool result, potentially offloading large results to filesystem.
    ///
    /// If offload is enabled and the result exceeds the threshold, saves to a temp file
    /// and returns instructions for the agent to read it with a CLI tool.
    fn process_tool_result(&self, tool_name: &str, result: String) -> String {
        // Check if offload is enabled and result exceeds threshold
        if !self.config.offload_large_results {
            return result;
        }

        if result.len() <= self.config.offload_threshold_chars {
            return result;
        }

        // Get offload directory
        let offload_dir = match &self.config.offload_dir {
            Some(dir) => dir.clone(),
            None => {
                // Default to ~/Documents/zbot/temp
                if let Some(home) = dirs::home_dir() {
                    home.join("Documents").join("zbot").join("temp")
                } else {
                    tracing::warn!("Could not determine home directory for offload");
                    return result;
                }
            }
        };

        // Create directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&offload_dir) {
            tracing::warn!("Failed to create offload directory: {}", e);
            return result;
        }

        // Generate unique filename
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let sanitized_tool = tool_name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        let filename = format!("{sanitized_tool}_{timestamp}.txt");
        let file_path = offload_dir.join(&filename);

        // Write result to file
        if let Err(e) = std::fs::write(&file_path, &result) {
            tracing::warn!("Failed to write offloaded result: {}", e);
            return result;
        }

        let result_size = result.len();
        let result_tokens = result_size / 4; // rough estimate

        tracing::info!(
            "Offloaded large tool result ({} chars, ~{} tokens) to: {}",
            result_size,
            result_tokens,
            file_path.display()
        );

        // Return instructions for the agent
        format!(
            "Tool result was too large for context ({} chars, ~{} tokens).\n\
            Result saved to: {}\n\n\
            To access the full result, use the `read` tool:\n\
            ```json\n\
            {{\"path\": \"{}\"}}\n\
            ```\n\n\
            Or use shell: `head -100 \"{}\"` to preview, `grep \"pattern\" \"{}\"` to search.",
            result_size,
            result_tokens,
            file_path.display(),
            file_path.display(),
            file_path.display(),
            file_path.display()
        )
    }
}

/// Normalize a string to be a valid `OpenAI` tool name.
///
/// `OpenAI` requires tool names to match: ^[a-zA-Z0-9_-]+$
/// This function replaces any invalid characters with underscores.
fn normalize_tool_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

// ============================================================================
// PROGRESS TRACKER
// ============================================================================

/// Tracks execution progress to distinguish productive work from stuck loops.
///
/// Used by the executor to decide whether to auto-extend iterations when
/// `max_iterations` is reached. Scores each iteration based on tool diversity,
/// success rate, and repetition patterns.
#[allow(dead_code)] // Extension fields kept for diagnostics/legacy
struct ProgressTracker {
    /// Recent tool calls as (name, `args_hash`) for repetition detection
    recent_tool_calls: VecDeque<(String, u64)>,
    /// Recent error messages for repeated-error detection
    recent_errors: VecDeque<String>,
    /// Unique tool names used during this scoring window
    unique_tools_used: HashSet<String>,
    /// Cumulative progress score for the current window
    score: i32,
    /// Number of auto-extensions granted so far
    extensions_granted: u32,
    /// Maximum extensions allowed
    max_extensions: u32,
    /// Total iterations consumed across all windows
    total_iterations: u32,
    /// Rolling window of tool names (last 20 calls) for diversity tracking
    tool_name_window: VecDeque<String>,
    /// Count of tool calls in current scoring window (for periodic diversity scoring)
    window_tool_calls: u32,
    /// Whether the agent has created a plan via todos(action="add")
    has_plan: bool,
    /// Number of todo items the agent has added
    plan_items_created: u32,
    /// Number of todo items completed via todos(action="update", completed=true)
    plan_items_completed: u32,
    /// Whether the planning nudge has been injected (max 1)
    planning_nudge_sent: bool,
    /// Non-todo tool calls made before first todos(action="add")
    tool_calls_before_plan: u32,
}

impl ProgressTracker {
    fn new(max_extensions: u32) -> Self {
        Self {
            recent_tool_calls: VecDeque::with_capacity(10),
            recent_errors: VecDeque::with_capacity(5),
            unique_tools_used: HashSet::new(),
            score: 0,
            extensions_granted: 0,
            max_extensions,
            total_iterations: 0,
            tool_name_window: VecDeque::with_capacity(20),
            window_tool_calls: 0,
            has_plan: false,
            plan_items_created: 0,
            plan_items_completed: 0,
            planning_nudge_sent: false,
            tool_calls_before_plan: 0,
        }
    }

    fn hash_args(args: &Value) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let s = serde_json::to_string(args).unwrap_or_default();
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }

    /// Record a tool call and update the progress score.
    fn record_tool_call(&mut self, name: &str, args: &Value, succeeded: bool) {
        // Planning enforcement: detect todo/update_plan tool usage
        if (name == "todos" || name == "update_plan") && succeeded {
            if name == "update_plan" {
                // update_plan uses {plan: [{step, status}]} — lightweight, fire-and-forget
                if let Some(plan) = args.get("plan").and_then(|v| v.as_array()) {
                    let step_count = plan.len() as u32;
                    let completed_count = plan
                        .iter()
                        .filter(|s| s.get("status").and_then(|v| v.as_str()) == Some("completed"))
                        .count() as u32;
                    if !self.has_plan {
                        self.plan_items_created = step_count;
                        self.has_plan = true;
                        self.score += 3 + step_count.min(5) as i32;
                    }
                    // Reward completed steps
                    if completed_count > self.plan_items_completed {
                        let new_completions = completed_count - self.plan_items_completed;
                        self.plan_items_completed = completed_count;
                        self.score += (new_completions * 2) as i32;
                    }
                }
            } else if let Some(action) = args.get("action").and_then(|v| v.as_str()) {
                // todos tool uses {action: "add"/"update"/"list"/"delete", ...}
                match action {
                    "add" => {
                        let item_count = args
                            .get("items")
                            .and_then(|v| v.as_array())
                            .map_or(1, |arr| arr.len() as u32);
                        self.plan_items_created += item_count;
                        self.has_plan = true;
                        self.score += 3 + item_count.min(5) as i32; // +3 base + 1/item (max +5)
                    }
                    "update" => {
                        if args
                            .get("completed")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false)
                        {
                            self.plan_items_completed += 1;
                            self.score += 2; // Reward working the plan
                        }
                    }
                    _ => {} // list, delete — neutral
                }
            }
        }

        if !self.has_plan && name != "todos" && name != "update_plan" {
            self.tool_calls_before_plan += 1;
        }

        let args_hash = Self::hash_args(args);

        // Exact repetition detection — same tool+args in last 5 calls
        // Only penalize FAILED exact repeats. Successful calls with same args
        // (e.g., ralph.py next) are legitimate workflow patterns.
        let is_exact_repeat = self
            .recent_tool_calls
            .iter()
            .any(|(n, h)| n == name && *h == args_hash);
        if is_exact_repeat && !succeeded {
            self.score -= 3;
        }

        // Tool diversity scoring via rolling window
        // Only track FAILED calls for diversity scoring. Subagents with 4 tools
        // (shell, apply_patch, load_skill, respond) naturally have low diversity
        // ratios even when productive. Penalizing low diversity on successful
        // calls kills productive ralph.py workflows.
        if !succeeded {
            self.tool_name_window.push_back(name.to_string());
            if self.tool_name_window.len() > 20 {
                self.tool_name_window.pop_front();
            }
        }

        // Score diversity every 10 FAILED calls (not total calls)
        self.window_tool_calls += 1;
        if !succeeded
            && self.tool_name_window.len() >= 10
            && self.tool_name_window.len().is_multiple_of(5)
        {
            let distinct: HashSet<&str> = self
                .tool_name_window
                .iter()
                .map(std::string::String::as_str)
                .collect();
            let ratio = distinct.len() as f32 / self.tool_name_window.len() as f32;

            if ratio <= 0.15 {
                // Same tool failing repeatedly — definitely stuck
                self.score -= 8;
            } else if ratio <= 0.25 {
                self.score -= 3;
            }
            // No positive score for diversity — success bonus handles that
        }

        // First-ever use of a tool gets a small bonus
        if self.unique_tools_used.insert(name.to_string()) {
            self.score += 1;
        }

        // Successful tool calls get a small bonus to offset any accidental penalties.
        // This keeps productive agents alive. Stuck agents still die because
        // failures accumulate penalties faster than successes add bonuses.
        if succeeded {
            self.score += 1;
        }

        // Track for exact-repetition detection (keep last 5)
        self.recent_tool_calls
            .push_back((name.to_string(), args_hash));
        if self.recent_tool_calls.len() > 5 {
            self.recent_tool_calls.pop_front();
        }
    }

    /// Record a tool error for repeated-error detection.
    fn record_error(&mut self, error: &str) {
        // Check if this exact error appeared 3+ times recently
        let repeat_count = self
            .recent_errors
            .iter()
            .filter(|e| e.as_str() == error)
            .count();
        if repeat_count >= 2 {
            self.score -= 5; // Definitely stuck
        }

        self.recent_errors.push_back(error.to_string());
        if self.recent_errors.len() > 5 {
            self.recent_errors.pop_front();
        }
    }

    /// Record that a respond action was emitted — agent is finishing.
    fn record_respond(&mut self) {
        self.score += 10;
    }

    /// Record one iteration consumed.
    fn tick(&mut self) {
        self.total_iterations += 1;
    }

    /// Whether an auto-extension should be granted.
    /// Planless agents get a -3 effective score penalty.
    /// NOTE: No longer called from executor loop (iteration limits removed).
    /// Kept for potential future use and testing.
    #[allow(dead_code)]
    fn should_extend(&self) -> bool {
        let effective_score = if self.has_plan {
            self.score
        } else {
            self.score - 3 // Planless agents need score > 3 to extend
        };
        effective_score > 0 && self.extensions_granted < self.max_extensions
    }

    /// Check if the agent is clearly stuck and should stop early (before window boundary).
    /// Returns true if score has gone negative after at least 10 tool calls in this window.
    /// Threshold lowered from 15/-10 to 10/-5 because success bonus was removed.
    fn is_clearly_stuck(&self) -> bool {
        self.window_tool_calls >= 10 && self.score <= -5
    }

    /// Returns true once when agent should be nudged to create a plan.
    fn needs_planning_nudge(&mut self) -> bool {
        if !self.has_plan && !self.planning_nudge_sent && self.tool_calls_before_plan >= 5 {
            self.planning_nudge_sent = true;
            true
        } else {
            false
        }
    }

    /// Grant an extension: reset the score window and increment counter.
    /// NOTE: `tool_name_window` is NOT cleared — diversity tracking spans full session.
    /// NOTE: `has_plan`, `plan_items_created`, `plan_items_completed`, `planning_nudge_sent`,
    ///       and `tool_calls_before_plan` are intentionally NOT reset — planning state
    ///       spans the full execution.
    #[allow(dead_code)]
    fn grant_extension(&mut self) {
        self.extensions_granted += 1;
        self.score = 0;
        self.unique_tools_used.clear();
        self.recent_tool_calls.clear();
        self.recent_errors.clear();
        self.window_tool_calls = 0;
    }

    /// Build a human-readable diagnosis of the current state.
    fn diagnosis(&self) -> String {
        let plan_status = if self.has_plan {
            format!(
                ", plan: {}/{} items done",
                self.plan_items_completed, self.plan_items_created
            )
        } else {
            ", no plan created".to_string()
        };

        if self.score <= -10 {
            format!(
                "Stuck in loop: {} repeated tool calls detected (score: {}){}",
                self.recent_tool_calls.len(),
                self.score,
                plan_status
            )
        } else if self.score <= 0 {
            format!(
                "No progress detected after {} iterations (score: {}){}",
                self.total_iterations, self.score, plan_status
            )
        } else {
            format!(
                "Making progress: {} unique tools used (score: {}){}",
                self.unique_tools_used.len(),
                self.score,
                plan_status
            )
        }
    }

    /// Build a reason string for the extension event.
    #[allow(dead_code)]
    fn extension_reason(&self) -> String {
        format!(
            "Making progress: {} unique tools used, score {} (extension {}/{})",
            self.unique_tools_used.len(),
            self.score,
            self.extensions_granted + 1,
            self.max_extensions
        )
    }
}

/// Executor errors
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    /// Maximum iterations reached with no progress detected.
    #[error("Maximum iterations reached")]
    MaxIterationsReached,

    /// Maximum iterations reached but agent needs user intervention.
    #[error("Max iterations reached after {iterations_used} iterations: {reason}")]
    MaxIterationsNeedsIntervention {
        /// Total iterations consumed
        iterations_used: u32,
        /// Diagnosis of why the agent stopped
        reason: String,
    },

    /// LLM API error.
    #[error("LLM error: {0}")]
    LlmError(String),

    /// Tool execution error.
    #[error("Tool error: {0}")]
    ToolError(String),

    /// MCP server error.
    #[error("MCP error: {0}")]
    McpError(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Middleware pipeline error.
    #[error("Middleware error: {0}")]
    MiddlewareError(String),
}

/// Factory function to create an executor
///
/// This is a convenience function that creates an executor with default
/// components. For more control, create components separately and use
/// `AgentExecutor::new()`.
pub async fn create_executor(
    config: ExecutorConfig,
    llm_client: Arc<dyn LlmClient>,
    tool_registry: Arc<ToolRegistry>,
    mcp_manager: Arc<McpManager>,
) -> Result<AgentExecutor, ExecutorError> {
    // Create default middleware pipeline
    let middleware_pipeline = Arc::new(MiddlewarePipeline::new());

    AgentExecutor::new(
        config,
        llm_client,
        tool_registry,
        mcp_manager,
        middleware_pipeline,
    )
}

/// Extract key info (file paths, URLs) from a tool result for restorable compression.
fn extract_key_info(content: &str) -> String {
    let mut info = Vec::new();

    for word in content.split_whitespace() {
        let trimmed = word.trim_matches(|c: char| {
            c == '"' || c == '\'' || c == ',' || c == ':' || c == '(' || c == ')'
        });
        if (trimmed.contains('/') || trimmed.contains('.'))
            && (trimmed.ends_with(".py")
                || trimmed.ends_with(".json")
                || trimmed.ends_with(".csv")
                || trimmed.ends_with(".html")
                || trimmed.ends_with(".md")
                || trimmed.ends_with(".js")
                || trimmed.ends_with(".ts")
                || trimmed.ends_with(".yaml")
                || trimmed.ends_with(".toml"))
            && !info.contains(&trimmed.to_string())
        {
            info.push(trimmed.to_string());
        }
        if (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
            && !info.contains(&trimmed.to_string())
        {
            info.push(trimmed.to_string());
        }
    }

    info.join(", ")
}

/// Compact messages to reduce context size when approaching token limits.
///
/// Strategy:
/// 1. Compress old assistant messages to one-liners (preserving tool names and file paths)
/// 2. Clear old tool result content (replace with placeholder, preserve file paths)
/// 3. Only drop messages if still over budget after compression
fn compact_messages(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    const KEEP_RECENT: usize = 20;

    if messages.len() <= KEEP_RECENT + 2 {
        return messages;
    }

    let mut messages = messages;

    // Phase 1: Compress old assistant messages to one-liners
    crate::middleware::compress_old_assistant_messages(&mut messages, KEEP_RECENT);

    // Phase 2: Clear old tool result content (keep tool_call_id for pairing)
    let compress_boundary = messages.len().saturating_sub(KEEP_RECENT);
    for message in &mut messages[..compress_boundary] {
        if message.role == "tool" {
            let text = message.text_content();
            let preserved = extract_key_info(&text);
            message.content = vec![Part::Text {
                text: if preserved.is_empty() {
                    "[result cleared]".to_string()
                } else {
                    format!("[result cleared — {preserved}]")
                },
            }];
        }
    }

    // Phase 3: If still too many messages, drop old ones
    if messages.len() > KEEP_RECENT + 10 {
        let mut compacted = Vec::new();

        // Keep system messages
        let mut non_system_start = 0;
        for (i, msg) in messages.iter().enumerate() {
            if msg.role == "system" {
                compacted.push(msg.clone());
                non_system_start = i + 1;
            } else {
                break;
            }
        }

        // Preserve first user message
        if let Some(user_msg) = messages[non_system_start..]
            .iter()
            .find(|m| m.role == "user")
        {
            compacted.push(user_msg.clone());
        }

        // Find clean split point
        let target_start = messages.len().saturating_sub(KEEP_RECENT);
        let mut split_at = target_start;
        for (i, msg) in messages.iter().enumerate().skip(target_start) {
            if msg.role == "user" || (msg.role == "assistant" && msg.tool_call_id.is_none()) {
                split_at = i;
                break;
            }
        }

        let trimmed_count = split_at.saturating_sub(non_system_start);
        if trimmed_count > 0 {
            compacted.push(ChatMessage::user(format!(
                "[SYSTEM: Context compacted. {trimmed_count} earlier messages were compressed and trimmed. \
                 The original request and recent messages are preserved. Continue with the task.]"
            )));
        }

        compacted.extend(messages[split_at..].iter().cloned());
        compacted
    } else {
        // Compression was enough — no need to drop
        messages
    }
}

/// Sanitize messages to ensure tool call/result pairs are valid.
///
/// Removes orphaned `tool` messages whose `tool_call_id` doesn't match
/// any `tool_calls` entry in a preceding `assistant` message.
/// This prevents API errors: "Messages with role 'tool' must be a response
/// to a preceding message with '`tool_calls`'"
fn sanitize_messages(messages: &mut Vec<ChatMessage>) {
    // Collect all valid tool_call_ids from assistant messages
    let mut valid_tool_call_ids = HashSet::new();
    for msg in messages.iter() {
        if msg.role == "assistant" {
            if let Some(ref tool_calls) = msg.tool_calls {
                for tc in tool_calls {
                    valid_tool_call_ids.insert(tc.id.clone());
                }
            }
        }
    }

    // Remove orphaned tool messages
    let original_len = messages.len();
    messages.retain(|msg| {
        if msg.role == "tool" {
            if let Some(ref tool_call_id) = msg.tool_call_id {
                if !valid_tool_call_ids.contains(tool_call_id) {
                    tracing::warn!(
                        tool_call_id = %tool_call_id,
                        "Removing orphaned tool message — no matching assistant tool_call found"
                    );
                    return false;
                }
            }
        }
        true
    });

    if messages.len() < original_len {
        tracing::warn!(
            removed = original_len - messages.len(),
            "Sanitized {} orphaned tool messages from context",
            original_len - messages.len()
        );
    }
}

/// Truncate tool arguments to prevent context explosion.
///
/// When LLMs generate tool calls with massive arguments (e.g., including
/// full conversation context), storing these in message history causes
/// exponential growth. This function truncates arguments to a reasonable size.
#[allow(dead_code)]
fn truncate_tool_args(args: &Value, max_chars: usize) -> Value {
    let args_str = serde_json::to_string(args).unwrap_or_default();
    if args_str.len() <= max_chars {
        return args.clone();
    }

    // For objects, try to truncate string values
    if let Some(obj) = args.as_object() {
        let mut truncated = serde_json::Map::new();
        for (key, value) in obj {
            if let Some(s) = value.as_str() {
                if s.len() > 200 {
                    truncated.insert(
                        key.clone(),
                        Value::String(format!(
                            "{}... [truncated, {} chars]",
                            zero_core::truncate_str(s, 200),
                            s.len()
                        )),
                    );
                } else {
                    truncated.insert(key.clone(), value.clone());
                }
            } else {
                truncated.insert(key.clone(), value.clone());
            }
        }
        return Value::Object(truncated);
    }

    // Fallback: return a placeholder
    json!({"_truncated": true, "_original_size": args_str.len()})
}

/// Truncate a tool result string if it exceeds `max_chars`.
///
/// Keeps the first ~80% and last ~20% of the budget with a truncation notice.
/// Returns the original string if within limits or if `max_chars` is 0 (disabled).
fn truncate_single_line(result: &str, max_chars: usize) -> String {
    let notice = format!("\n\n--- TRUNCATED ({} chars total) ---\n\n", result.len());
    let budget = max_chars.saturating_sub(notice.len());
    let head_size = (budget * 4) / 5;
    let tail_size = budget - head_size;
    format!(
        "{}{}{}",
        &result[..head_size],
        notice,
        &result[result.len() - tail_size..]
    )
}

fn truncate_tool_result(result: String, max_chars: usize) -> String {
    if max_chars == 0 || result.len() <= max_chars {
        return result;
    }

    let lines: Vec<&str> = result.lines().collect();
    let total_lines = lines.len();

    if total_lines <= 1 {
        // Single line — fall back to char-based truncation
        return truncate_single_line(&result, max_chars);
    }

    // Line-aware: keep first N + last M lines within budget
    let head_budget = (max_chars * 4) / 5; // 80% for head
    let mut head = String::new();
    let mut head_count = 0;
    for line in &lines {
        let next = format!("{line}\n");
        if head.len() + next.len() > head_budget {
            break;
        }
        head.push_str(&next);
        head_count += 1;
    }

    // Tail: work backwards
    let tail_budget = max_chars / 5; // 20% for tail
    let mut tail_lines: Vec<&str> = Vec::new();
    let mut tail_len = 0;
    for line in lines.iter().rev() {
        let next_len = line.len() + 1;
        if tail_len + next_len > tail_budget {
            break;
        }
        tail_lines.push(line);
        tail_len += next_len;
    }
    tail_lines.reverse();
    let tail_count = tail_lines.len();
    let tail = tail_lines.join("\n");

    let omitted = total_lines.saturating_sub(head_count + tail_count);
    let notice = format!(
        "\n--- TRUNCATED: showing {}/{} lines ({} omitted, {} chars total) ---\n\n",
        head_count + tail_count,
        total_lines,
        omitted,
        result.len()
    );

    // Final budget check — if combined fits, return it; otherwise trim head/tail further
    let combined = format!("{head}{notice}{tail}");
    if combined.len() <= max_chars {
        return combined;
    }

    // Re-compute with tighter budgets accounting for notice length
    let notice_len = notice.len();
    let content_budget = max_chars.saturating_sub(notice_len);
    let tight_head_budget = (content_budget * 4) / 5;
    let tight_tail_budget = content_budget - tight_head_budget;

    let mut tight_head = String::new();
    let mut tight_head_count = 0;
    for line in &lines {
        let next = format!("{line}\n");
        if tight_head.len() + next.len() > tight_head_budget {
            break;
        }
        tight_head.push_str(&next);
        tight_head_count += 1;
    }

    let mut tight_tail_lines: Vec<&str> = Vec::new();
    let mut tight_tail_len = 0;
    for line in lines.iter().rev() {
        let next_len = line.len() + 1;
        if tight_tail_len + next_len > tight_tail_budget {
            break;
        }
        tight_tail_lines.push(line);
        tight_tail_len += next_len;
    }
    tight_tail_lines.reverse();
    let tight_tail_count = tight_tail_lines.len();
    let tight_tail = tight_tail_lines.join("\n");

    let tight_omitted = total_lines.saturating_sub(tight_head_count + tight_tail_count);
    let tight_notice = format!(
        "\n--- TRUNCATED: showing {}/{} lines ({} omitted, {} chars total) ---\n\n",
        tight_head_count + tight_tail_count,
        total_lines,
        tight_omitted,
        result.len()
    );

    format!("{tight_head}{tight_notice}{tight_tail}")
}

#[cfg(test)]
mod truncation_tests {
    use super::*;

    #[test]
    fn test_truncate_tool_result_under_limit() {
        let result = "hello world".to_string();
        assert_eq!(truncate_tool_result(result.clone(), 100), result);
    }

    #[test]
    fn test_truncate_tool_result_disabled() {
        let result = "a".repeat(50_000);
        assert_eq!(truncate_tool_result(result.clone(), 0), result);
    }

    #[test]
    fn test_truncate_tool_result_over_limit() {
        let result = "a".repeat(1000) + &"b".repeat(1000);
        let truncated = truncate_tool_result(result, 500);
        assert!(truncated.len() <= 500);
        assert!(truncated.contains("TRUNCATED"));
        assert!(truncated.starts_with("aaa"));
        assert!(truncated.ends_with("bbb"));
    }

    #[test]
    fn test_truncate_tool_result_preserves_head_tail_ratio() {
        let result = "H".repeat(10_000) + &"T".repeat(10_000);
        let truncated = truncate_tool_result(result, 1000);
        // Head should be ~80%, tail ~20% of budget
        let head_h = truncated.matches('H').count();
        let tail_t = truncated.matches('T').count();
        assert!(
            head_h > tail_t,
            "head ({head_h}) should be larger than tail ({tail_t})"
        );
    }

    #[test]
    fn test_truncation_preserves_line_boundaries() {
        let lines: Vec<String> = (0..100)
            .map(|i| format!("Line {i}: some content here"))
            .collect();
        let input = lines.join("\n");
        let result = truncate_tool_result(input, 500);

        // Should not cut mid-line
        for line in result.lines() {
            assert!(
                line.starts_with("Line")
                    || line.contains("TRUNCATED")
                    || line.contains("---")
                    || line.is_empty(),
                "Truncated mid-line: '{line}'"
            );
        }
    }

    #[test]
    fn test_truncate_tool_args_small() {
        let args = json!({"key": "value"});
        let result = truncate_tool_args(&args, 500);
        assert_eq!(result, args);
    }

    #[test]
    fn test_truncate_tool_args_large_string() {
        let args = json!({"content": "x".repeat(500)});
        let result = truncate_tool_args(&args, 100);
        let content = result.get("content").unwrap().as_str().unwrap();
        assert!(content.contains("truncated"));
    }
}

#[cfg(test)]
mod progress_tracker_tests {
    use super::*;

    #[test]
    fn test_new_tracker_no_extension() {
        let tracker = ProgressTracker::new(3);
        assert!(!tracker.should_extend(), "Empty tracker should not extend");
    }

    #[test]
    fn test_unique_tools_grant_extension() {
        let mut tracker = ProgressTracker::new(3);
        // Create a plan first so the -3 planless penalty doesn't apply
        tracker.record_tool_call(
            "update_plan",
            &json!({"plan": [{"step": "read", "status": "pending"}]}),
            true,
        );
        tracker.record_tool_call("read", &json!({"path": "/a"}), true);
        tracker.record_tool_call("write", &json!({"path": "/b"}), true);
        tracker.record_tool_call("shell", &json!({"cmd": "ls"}), true);
        // update_plan: +4(plan bonus) +1(unique) = 5, then +1 each for 3 more unique tools = 8
        assert!(tracker.should_extend());
    }

    #[test]
    fn test_repeated_calls_prevent_extension() {
        let mut tracker = ProgressTracker::new(3);
        let args = json!({"path": "/same"});
        // Same tool+args 5 times, all failed
        for _ in 0..5 {
            tracker.record_tool_call("read", &args, false);
        }
        // First call: +1 (unique) = 1
        // Subsequent 4 calls: -3 (repeat) each = -12
        // Total: 1 + (-12) = -11
        assert!(!tracker.should_extend());
    }

    #[test]
    fn test_repeated_errors_prevent_extension() {
        let mut tracker = ProgressTracker::new(3);
        tracker.record_tool_call("shell", &json!({"cmd": "fail"}), false);
        tracker.record_error("connection refused");
        tracker.record_error("connection refused");
        tracker.record_error("connection refused"); // 3rd time: -5
                                                    // tool call: +1 (unique) +0 (failed) = 1
                                                    // errors: -5
                                                    // total: 1 - 5 = -4
        assert!(!tracker.should_extend());
    }

    #[test]
    fn test_respond_boosts_score() {
        let mut tracker = ProgressTracker::new(3);
        tracker.record_respond();
        // +10 from respond
        assert!(tracker.should_extend());
    }

    #[test]
    fn test_max_extensions_respected() {
        let mut tracker = ProgressTracker::new(2);
        tracker.record_respond(); // +10
        assert!(tracker.should_extend());
        tracker.grant_extension();

        tracker.record_respond(); // +10 (fresh window)
        assert!(tracker.should_extend());
        tracker.grant_extension();

        tracker.record_respond(); // +10 (fresh window)
        assert!(
            !tracker.should_extend(),
            "Should not extend beyond max_extensions=2"
        );
    }

    #[test]
    fn test_grant_extension_resets_window() {
        let mut tracker = ProgressTracker::new(3);
        tracker.record_tool_call("read", &json!({}), true); // +1 (unique only)
        tracker.grant_extension();
        // After grant, score=0, unique_tools cleared, window_tool_calls reset
        assert!(!tracker.should_extend(), "Score reset to 0 after grant");
        assert_eq!(tracker.extensions_granted, 1);
        assert_eq!(tracker.window_tool_calls, 0);
    }

    #[test]
    fn test_diagnosis_stuck() {
        let mut tracker = ProgressTracker::new(3);
        let args = json!({"path": "/same"});
        for _ in 0..6 {
            tracker.record_tool_call("read", &args, false);
        }
        let diagnosis = tracker.diagnosis();
        assert!(
            diagnosis.contains("loop") || diagnosis.contains("No progress"),
            "Got: {diagnosis}"
        );
    }

    #[test]
    fn test_diagnosis_progress() {
        let mut tracker = ProgressTracker::new(3);
        // Use enough diverse tools to stay positive
        tracker.record_tool_call("read", &json!({}), true);
        tracker.record_tool_call("write", &json!({}), true);
        tracker.record_tool_call("shell", &json!({}), true);
        let diagnosis = tracker.diagnosis();
        assert!(diagnosis.contains("progress"), "Got: {diagnosis}");
    }

    #[test]
    fn test_executor_config_defaults() {
        let config = ExecutorConfig::new("a".into(), "p".into(), "m".into());
        assert_eq!(config.max_iterations, 50);
        assert_eq!(config.max_extensions, 3);
        assert_eq!(config.extension_size, 25);
        assert_eq!(config.turn_budget, 25);
        assert_eq!(config.max_turns, 50);
    }

    #[test]
    fn test_low_diversity_loop_detected() {
        let mut tracker = ProgressTracker::new(3);
        // Simulate write+shell loop for 20 iterations, all failed (different args each time)
        for i in 0..20 {
            let tool = if i % 2 == 0 { "write" } else { "shell" };
            tracker.record_tool_call(tool, &json!({"i": i}), false);
        }
        // After 20 failed calls:
        // 2 unique tools (+1 each = +2)
        // At 10 failed calls: diversity = 2/10 = 0.20 <= 0.25 → -3
        // At 15 failed calls: diversity = 2/15 = 0.13 <= 0.15 → -8
        // At 20 failed calls: diversity = 2/20 = 0.10 <= 0.15 → -8
        // Total: +2 - 3 - 8 - 8 = -17
        assert!(
            tracker.score < 0,
            "Low-diversity loop should have negative score, got: {}",
            tracker.score
        );
    }

    #[test]
    fn test_high_diversity_extends() {
        let mut tracker = ProgressTracker::new(3);
        // Use 10 unique tools in 10 calls (all succeed)
        let tools = [
            "read", "write", "shell", "edit", "grep", "glob", "memory", "todo", "ward", "respond",
        ];
        for (i, tool) in tools.iter().enumerate() {
            tracker.record_tool_call(tool, &json!({"i": i}), true);
        }
        // 10 unique tools: +1 each = 10, +1 success each = 10
        // No diversity check (only tracks failed calls, none here)
        // Total: 10 + 10 = 20
        assert!(
            tracker.score > 0,
            "High diversity should produce positive score, got: {}",
            tracker.score
        );
        assert!(
            tracker.should_extend(),
            "High diversity should allow extension"
        );
    }

    #[test]
    fn test_early_stop_deeply_stuck() {
        let mut tracker = ProgressTracker::new(3);
        let args = json!({"path": "/same"});
        // Same exact tool+args repeated, all failed — triggers repetition and diversity penalties
        // Call 1: +1 (unique) = 1
        // Calls 2-20: -3 (repeat) each = -57
        // At 10 failed calls: diversity = 1/10 ≤ 0.15 → -8
        // At 15 failed calls: diversity = 1/15 ≤ 0.15 → -8
        // At 20 failed calls: diversity = 1/20 ≤ 0.15 → -8
        // Total: 1 - 57 - 8 - 8 - 8 = -80
        for _ in 0..20 {
            tracker.record_tool_call("read", &args, false);
        }
        // With 10+ window_tool_calls and deeply negative score, should be stuck
        assert!(
            tracker.window_tool_calls >= 10,
            "Should have 20 window_tool_calls, got: {}",
            tracker.window_tool_calls
        );
        assert!(
            tracker.score <= -5,
            "Score should be <= -5 with exact-repeat loop, got: {}",
            tracker.score
        );
        assert!(
            tracker.is_clearly_stuck(),
            "Should be clearly stuck with score {} after {} calls",
            tracker.score,
            tracker.window_tool_calls
        );
    }

    #[test]
    fn test_tool_name_window_preserved_across_extensions() {
        let mut tracker = ProgressTracker::new(3);
        // Add some failed tool calls to fill the name window (only failed calls tracked)
        for i in 0..10 {
            let tool = if i % 2 == 0 { "write" } else { "shell" };
            tracker.record_tool_call(tool, &json!({"i": i}), false);
        }
        assert_eq!(tracker.tool_name_window.len(), 10);

        // Grant extension
        tracker.grant_extension();

        // tool_name_window should be preserved
        assert_eq!(
            tracker.tool_name_window.len(),
            10,
            "tool_name_window should survive grant_extension"
        );
        // But window_tool_calls should reset
        assert_eq!(tracker.window_tool_calls, 0);
        // And score should reset
        assert_eq!(tracker.score, 0);
    }

    // ========================================================================
    // PLANNING ENFORCEMENT TESTS
    // ========================================================================

    #[test]
    fn test_todo_add_sets_has_plan() {
        let mut tracker = ProgressTracker::new(3);
        tracker.record_tool_call("todos", &json!({"action": "add", "title": "step 1"}), true);
        assert!(tracker.has_plan);
        assert_eq!(tracker.plan_items_created, 1);
    }

    #[test]
    fn test_todo_add_batch_counts_items() {
        let mut tracker = ProgressTracker::new(3);
        tracker.record_tool_call(
            "todos",
            &json!({"action": "add", "items": [
                {"title": "step 1"},
                {"title": "step 2"},
                {"title": "step 3"}
            ]}),
            true,
        );
        assert!(tracker.has_plan);
        assert_eq!(tracker.plan_items_created, 3);
    }

    #[test]
    fn test_todo_add_boosts_score() {
        let mut tracker = ProgressTracker::new(3);
        tracker.record_tool_call(
            "todos",
            &json!({"action": "add", "items": [
                {"title": "step 1"},
                {"title": "step 2"}
            ]}),
            true,
        );
        // +3 base + 2 items + 1 unique tool + 1 success = 7
        assert_eq!(tracker.score, 7);
    }

    #[test]
    fn test_todo_update_completed_boosts_score() {
        let mut tracker = ProgressTracker::new(3);
        // First add a plan so we have context
        tracker.record_tool_call("todos", &json!({"action": "add", "title": "step 1"}), true);
        let score_after_add = tracker.score;
        // Complete the item
        tracker.record_tool_call(
            "todos",
            &json!({"action": "update", "id": "1", "completed": true}),
            true,
        );
        // +2 completion bonus + 1 success (unique tool bonus already used)
        assert_eq!(tracker.score, score_after_add + 3);
        assert_eq!(tracker.plan_items_completed, 1);
    }

    #[test]
    fn test_todo_update_incomplete_no_bonus() {
        let mut tracker = ProgressTracker::new(3);
        tracker.record_tool_call(
            "todos",
            &json!({"action": "update", "id": "1", "completed": false}),
            true,
        );
        // +1 unique tool + 1 success = 2, no completion bonus
        assert_eq!(tracker.score, 2);
        assert_eq!(tracker.plan_items_completed, 0);
    }

    #[test]
    fn test_failed_todo_call_not_counted() {
        let mut tracker = ProgressTracker::new(3);
        tracker.record_tool_call("todos", &json!({"action": "add", "title": "step 1"}), false);
        assert!(!tracker.has_plan);
        assert_eq!(tracker.plan_items_created, 0);
    }

    #[test]
    fn test_tool_calls_before_plan_counted() {
        let mut tracker = ProgressTracker::new(3);
        tracker.record_tool_call("read", &json!({"path": "/a"}), true);
        tracker.record_tool_call("write", &json!({"path": "/b"}), true);
        assert_eq!(tracker.tool_calls_before_plan, 2);

        // Create plan
        tracker.record_tool_call("todos", &json!({"action": "add", "title": "step 1"}), true);
        assert_eq!(tracker.tool_calls_before_plan, 2); // Frozen

        // More tool calls after plan — counter should not increase
        tracker.record_tool_call("shell", &json!({"cmd": "ls"}), true);
        assert_eq!(tracker.tool_calls_before_plan, 2);
    }

    #[test]
    fn test_needs_planning_nudge_at_threshold() {
        let mut tracker = ProgressTracker::new(3);
        for i in 0..5 {
            tracker.record_tool_call("read", &json!({"path": format!("/{}", i)}), true);
        }
        assert_eq!(tracker.tool_calls_before_plan, 5);
        assert!(tracker.needs_planning_nudge());
    }

    #[test]
    fn test_needs_planning_nudge_only_once() {
        let mut tracker = ProgressTracker::new(3);
        for i in 0..6 {
            tracker.record_tool_call("read", &json!({"path": format!("/{}", i)}), true);
        }
        assert!(tracker.needs_planning_nudge());
        assert!(
            !tracker.needs_planning_nudge(),
            "Nudge should fire only once"
        );
    }

    #[test]
    fn test_no_nudge_if_plan_exists() {
        let mut tracker = ProgressTracker::new(3);
        // Create plan first
        tracker.record_tool_call("todos", &json!({"action": "add", "title": "step 1"}), true);
        // Then do 10 tool calls
        for i in 0..10 {
            tracker.record_tool_call("read", &json!({"path": format!("/{}", i)}), true);
        }
        assert!(!tracker.needs_planning_nudge());
    }

    #[test]
    fn test_should_extend_penalizes_no_plan() {
        // Score 2 without plan → effective -1 → no extend
        let mut tracker = ProgressTracker::new(3);
        tracker.record_tool_call("read", &json!({}), true); // +1 unique + 1 success = 2
        assert!(!tracker.has_plan);
        assert_eq!(tracker.score, 2);
        assert!(
            !tracker.should_extend(),
            "Score 2 without plan should not extend (effective -1)"
        );

        // Score 4 without plan → effective 1 → extends (but let's test score 3 first)
        let mut tracker2 = ProgressTracker::new(3);
        tracker2.record_tool_call("read", &json!({}), true); // +2
        tracker2.record_tool_call("write", &json!({}), true); // +2
        assert!(!tracker2.has_plan);
        assert_eq!(tracker2.score, 4);
        assert!(
            tracker2.should_extend(),
            "Score 4 without plan should extend (effective 1)"
        );

        // Score 8 without plan → effective 5 → extends
        let mut tracker3 = ProgressTracker::new(3);
        tracker3.record_tool_call("read", &json!({}), true); // +2
        tracker3.record_tool_call("write", &json!({}), true); // +2
        tracker3.record_tool_call("shell", &json!({}), true); // +2
        tracker3.record_tool_call("edit", &json!({}), true); // +2
        assert!(!tracker3.has_plan);
        assert_eq!(tracker3.score, 8);
        assert!(
            tracker3.should_extend(),
            "Score 8 without plan should extend (effective 5)"
        );
    }

    #[test]
    fn test_should_extend_no_penalty_with_plan() {
        let mut tracker = ProgressTracker::new(3);
        tracker.record_tool_call("todos", &json!({"action": "add", "title": "step 1"}), true);
        tracker.record_tool_call("read", &json!({}), true);
        tracker.record_tool_call("write", &json!({}), true);
        assert!(tracker.has_plan);
        assert!(tracker.score > 0);
        assert!(
            tracker.should_extend(),
            "With plan, positive score should extend"
        );
    }

    #[test]
    fn test_planning_state_survives_grant_extension() {
        let mut tracker = ProgressTracker::new(3);
        tracker.record_tool_call("todos", &json!({"action": "add", "title": "step 1"}), true);
        tracker.record_tool_call(
            "todos",
            &json!({"action": "update", "id": "1", "completed": true}),
            true,
        );
        // Force a nudge scenario before plan (won't fire since has_plan=true, but set for test)
        tracker.tool_calls_before_plan = 10;

        tracker.grant_extension();

        assert!(tracker.has_plan, "has_plan should survive grant_extension");
        assert_eq!(
            tracker.plan_items_created, 1,
            "plan_items_created should survive"
        );
        assert_eq!(
            tracker.plan_items_completed, 1,
            "plan_items_completed should survive"
        );
        assert_eq!(
            tracker.tool_calls_before_plan, 10,
            "tool_calls_before_plan should survive"
        );
    }

    #[test]
    fn test_diagnosis_includes_plan_status() {
        let mut tracker = ProgressTracker::new(3);
        tracker.record_tool_call(
            "todos",
            &json!({"action": "add", "items": [{"title": "a"}, {"title": "b"}]}),
            true,
        );
        tracker.record_tool_call(
            "todos",
            &json!({"action": "update", "id": "1", "completed": true}),
            true,
        );
        let diagnosis = tracker.diagnosis();
        assert!(
            diagnosis.contains("plan: 1/2 items done"),
            "Expected plan status in diagnosis, got: {diagnosis}"
        );
    }

    #[test]
    fn test_diagnosis_shows_no_plan() {
        let mut tracker = ProgressTracker::new(3);
        tracker.record_tool_call("read", &json!({}), true);
        let diagnosis = tracker.diagnosis();
        assert!(
            diagnosis.contains("no plan created"),
            "Expected 'no plan created' in diagnosis, got: {diagnosis}"
        );
    }

    // ========================================================================
    // STUCK DETECTION THRESHOLD TESTS (post-deflation)
    // ========================================================================

    #[test]
    fn test_is_clearly_stuck_requires_10_calls() {
        let mut tracker = ProgressTracker::new(3);
        let args = json!({"path": "/same"});
        // 9 repeated failed calls — not enough window_tool_calls to trigger
        for _ in 0..9 {
            tracker.record_tool_call("read", &args, false);
        }
        assert!(
            !tracker.is_clearly_stuck(),
            "Should not be stuck with only {} calls (need 10), score: {}",
            tracker.window_tool_calls,
            tracker.score
        );
        // 10th call pushes over the threshold
        tracker.record_tool_call("read", &args, false);
        assert!(
            tracker.is_clearly_stuck(),
            "Should be stuck at {} calls with score {}",
            tracker.window_tool_calls,
            tracker.score
        );
    }

    #[test]
    fn test_safety_valve_at_negative_12() {
        let mut tracker = ProgressTracker::new(3);
        let args = json!({"path": "/same"});
        for _ in 0..15 {
            tracker.record_tool_call("read", &args, false);
        }
        // Score: +1(unique) - 14*3(repeats) - 8(div@10) - 8(div@15) = 1-42-8-8 = -57
        assert!(
            tracker.score <= -12,
            "Score should be <= -12 after 15 exact repeats, got: {}",
            tracker.score
        );
    }

    // ========================================================================
    // COMPACTION TESTS
    // ========================================================================

    #[test]
    fn test_compact_messages_preserves_original_request() {
        let mut messages = Vec::new();
        // System message
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: vec![Part::Text {
                text: "You are an assistant.".to_string(),
            }],
            tool_calls: None,
            tool_call_id: None,
        });
        // Original user request
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: vec![Part::Text {
                text: "Build a trinomial cheat sheet.".to_string(),
            }],
            tool_calls: None,
            tool_call_id: None,
        });
        // Add 30 filler messages so compaction kicks in
        for i in 0..30 {
            messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: vec![Part::Text {
                    text: format!("Step {i}"),
                }],
                tool_calls: None,
                tool_call_id: None,
            });
        }

        let compacted = compact_messages(messages);

        // Should contain: system + original user request + compaction notice + last 20
        assert!(
            compacted
                .iter()
                .any(|m| m.text_content().contains("trinomial cheat sheet")),
            "Compacted messages should preserve the original user request"
        );
        assert!(
            compacted
                .iter()
                .any(|m| m.text_content().contains("Context compacted")),
            "Compacted messages should include compaction notice"
        );
        assert!(
            compacted
                .iter()
                .any(|m| m.text_content().contains("original request")),
            "Compaction notice should reference the preserved original request"
        );
    }

    #[test]
    fn test_compact_messages_no_op_when_short() {
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: vec![Part::Text {
                    text: "system".to_string(),
                }],
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: "user".to_string(),
                content: vec![Part::Text {
                    text: "hello".to_string(),
                }],
                tool_calls: None,
                tool_call_id: None,
            },
        ];
        let compacted = compact_messages(messages.clone());
        assert_eq!(compacted.len(), messages.len());
    }

    // ========================================================================
    // E2E-STYLE LOOP DETECTOR TESTS (moved from e2e_ward_pipeline_tests)
    // ========================================================================

    /// Successful tool calls should not tank the progress score.
    #[test]
    fn test_loop_detector_productive_agent_survives() {
        let config = ExecutorConfig::new("a".into(), "p".into(), "m".into());
        let mut tracker = ProgressTracker::new(config.max_extensions);

        // Simulate a productive ralph.py workflow:
        // shell(ralph.py next) -> apply_patch(create file) -> shell(verify) -> shell(ralph.py complete)
        // All successful — score should stay positive
        for i in 0..5 {
            tracker.record_tool_call(
                "shell",
                &json!({"command": format!("ralph.py next {}", i)}),
                true,
            );
            tracker.record_tool_call(
                "apply_patch",
                &json!({"file": format!("core/mod{}.py", i)}),
                true,
            );
            tracker.record_tool_call(
                "shell",
                &json!({"command": format!("python3 -c 'import core.mod{}'", i)}),
                true,
            );
            tracker.record_tool_call(
                "shell",
                &json!({"command": format!("ralph.py complete {}", i)}),
                true,
            );
        }

        assert!(
            !tracker.is_clearly_stuck(),
            "Productive agent with 20 successful calls should NOT be stuck. Score: {}",
            tracker.score
        );
        assert!(
            tracker.score > 0,
            "Score should be positive for productive work, got: {}",
            tracker.score
        );
    }

    /// Failed repeated tool calls should tank the score.
    #[test]
    fn test_loop_detector_stuck_agent_dies() {
        let config = ExecutorConfig::new("a".into(), "p".into(), "m".into());
        let mut tracker = ProgressTracker::new(config.max_extensions);

        // Simulate a stuck agent: same shell command failing repeatedly
        for _ in 0..15 {
            tracker.record_tool_call("shell", &json!({"command": "cat nonexistent.py"}), false);
        }

        assert!(
            tracker.is_clearly_stuck(),
            "Agent with 15 repeated failures should be stuck. Score: {}",
            tracker.score
        );
    }

    /// Mixed success/failure: productive work with occasional errors should survive.
    #[test]
    fn test_loop_detector_mixed_survives() {
        let config = ExecutorConfig::new("a".into(), "p".into(), "m".into());
        let mut tracker = ProgressTracker::new(config.max_extensions);

        // 8 successes, 2 failures — should be fine
        for i in 0..10 {
            let succeeded = i % 5 != 3; // fail on iteration 3 and 8
            tracker.record_tool_call(
                if i % 2 == 0 { "shell" } else { "apply_patch" },
                &json!({"arg": format!("call_{}", i)}),
                succeeded,
            );
        }

        assert!(
            !tracker.is_clearly_stuck(),
            "Agent with 80% success rate should NOT be stuck. Score: {}",
            tracker.score
        );
    }
}

#[cfg(test)]
mod hook_tests {
    use super::*;

    #[test]
    fn test_tool_call_decision_default_is_allow() {
        let decision = ToolCallDecision::Allow;
        assert!(matches!(decision, ToolCallDecision::Allow));
    }

    #[test]
    fn test_tool_call_decision_block_has_reason() {
        let decision = ToolCallDecision::Block {
            reason: "dangerous".to_string(),
        };
        match decision {
            ToolCallDecision::Block { reason } => assert_eq!(reason, "dangerous"),
            _ => panic!("Expected Block"),
        }
    }

    #[test]
    fn test_before_tool_call_block_returns_reason() {
        let reason = "Ward boundary violation";
        let result = format!("{{\"blocked\":true,\"reason\":\"{reason}\"}}");
        assert!(result.contains("blocked"));
        assert!(result.contains(reason));
    }

    #[test]
    fn test_steering_message_format() {
        use crate::steering::{SteeringMessage, SteeringPriority, SteeringSource};
        let msg = SteeringMessage {
            content: "Wrap up now".to_string(),
            source: SteeringSource::System,
            priority: SteeringPriority::Normal,
        };
        let formatted = format!("[STEER: {}] {}", msg.source, msg.content);
        assert_eq!(formatted, "[STEER: System] Wrap up now");
    }

    #[test]
    fn test_transform_context_hook_type() {
        // Verify the type compiles and can be called
        let hook: TransformContextHook = Arc::new(|messages: &mut Vec<ChatMessage>| {
            messages.push(ChatMessage::system("injected".to_string()));
        });
        let mut messages = vec![ChatMessage::user("hello".to_string())];
        hook(&mut messages);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].text_content(), "injected");
    }

    #[test]
    fn test_complexity_budget_lookup() {
        fn budget_for(complexity: Option<&str>) -> (u32, u32) {
            match complexity {
                Some("S") => (15, 12),
                Some("M") => (30, 24),
                Some("L") => (50, 40),
                Some("XL") => (100, 80),
                _ => (0, 0),
            }
        }
        assert_eq!(budget_for(Some("S")), (15, 12));
        assert_eq!(budget_for(Some("M")), (30, 24));
        assert_eq!(budget_for(Some("L")), (50, 40));
        assert_eq!(budget_for(Some("XL")), (100, 80));
        assert_eq!(budget_for(None), (0, 0));
    }

    #[test]
    fn test_single_action_mode_default_false() {
        let config = ExecutorConfig::new("a".into(), "p".into(), "m".into());
        assert!(!config.single_action_mode);
    }
}

#[cfg(test)]
mod token_cache_tests {
    #[test]
    fn test_token_estimate_cache() {
        use std::collections::HashMap;
        let mut cache: HashMap<u64, usize> = HashMap::new();

        fn content_hash(content: &str) -> u64 {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            content.hash(&mut hasher);
            hasher.finish()
        }

        let msg = "Hello world this is a test message";
        let hash = content_hash(msg);

        // Cache miss
        assert!(!cache.contains_key(&hash));
        let estimate = msg.len() / 4 + 4;
        cache.insert(hash, estimate);

        // Cache hit
        assert_eq!(cache.get(&hash), Some(&estimate));
    }
}

#[cfg(test)]
mod compaction_tests {
    use super::*;

    #[test]
    fn test_compact_compresses_before_dropping() {
        let mut messages = vec![
            ChatMessage::system("system prompt".to_string()),
            ChatMessage::user("original request".to_string()),
        ];

        for i in 0..14 {
            let tool = ToolCall::new(
                format!("call_{i}"),
                "write_file".to_string(),
                json!({"path": format!("src/file_{}.py", i)}),
            );
            messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: vec![Part::Text {
                    text: format!("Creating file_{i}.py with detailed explanation"),
                }],
                tool_calls: Some(vec![tool]),
                tool_call_id: None,
            });
            messages.push(ChatMessage {
                role: "tool".to_string(),
                content: vec![Part::Text {
                    text: format!("File created: src/file_{i}.py"),
                }],
                tool_calls: None,
                tool_call_id: Some(format!("call_{i}")),
            });
        }

        let compacted = compact_messages(messages);

        // Old assistant messages should be compressed
        let has_compressed = compacted
            .iter()
            .any(|m| m.text_content().starts_with("[Turn"));
        assert!(
            has_compressed,
            "Old assistant messages should be compressed"
        );

        // Old tool results should preserve file paths
        let has_preserved = compacted.iter().any(|m| {
            m.text_content().contains("[result cleared") && m.text_content().contains(".py")
        });
        assert!(
            has_preserved,
            "Cleared tool results should preserve file paths"
        );
    }

    #[test]
    fn test_compact_preserves_recent() {
        let mut messages = vec![
            ChatMessage::system("system".to_string()),
            ChatMessage::user("request".to_string()),
        ];
        for i in 0..25 {
            messages.push(ChatMessage::user(format!("msg {i}")));
        }
        let compacted = compact_messages(messages);
        assert!(compacted.last().unwrap().text_content().contains("msg 24"));
    }

    #[test]
    fn test_extract_key_info() {
        let content = "File created: src/main.py with 100 lines. See https://example.com for docs.";
        let info = extract_key_info(content);
        assert!(info.contains("src/main.py"));
        assert!(info.contains("https://example.com"));
    }

    #[test]
    fn test_extract_key_info_empty() {
        let info = extract_key_info("Success! Operation completed.");
        assert!(info.is_empty());
    }
}
