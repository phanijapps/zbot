// ============================================================================
// TRANSPORT TYPES
// Shared types for the transport abstraction layer
// ============================================================================

// ============================================================================
// Transport Configuration
// ============================================================================

export interface TransportConfig {
  /** HTTP base URL for the gateway (e.g., http://localhost:18791) */
  httpUrl: string;
  /** WebSocket URL for the gateway (e.g., ws://localhost:18790) */
  wsUrl: string;
}

// ============================================================================
// Transport Result
// ============================================================================

export interface TransportResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

// ============================================================================
// Agent Types (for HTTP API)
// ============================================================================

export interface AgentResponse {
  id: string;
  name: string;
  displayName: string;
  description: string;
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
  thinkingEnabled: boolean;
  voiceRecordingEnabled: boolean;
  instructions: string;
  mcps: string[];
  skills: string[];
  middleware?: string;
  createdAt?: string;
}

export interface CreateAgentRequest {
  name: string;
  displayName?: string;
  description?: string;
  providerId: string;
  model: string;
  temperature?: number;
  maxTokens?: number;
  instructions?: string;
  mcps?: string[];
  skills?: string[];
}

export interface UpdateAgentRequest {
  name?: string;
  displayName?: string;
  description?: string;
  providerId?: string;
  model?: string;
  temperature?: number;
  maxTokens?: number;
  thinkingEnabled?: boolean;
  voiceRecordingEnabled?: boolean;
  instructions?: string;
  mcps?: string[];
  skills?: string[];
  middleware?: string;
}

// ============================================================================
// Conversation Types
// ============================================================================

export interface ConversationResponse {
  id: string;
  agentId: string;
  title?: string;
  createdAt: string;
  updatedAt: string;
  messageCount: number;
}

export interface MessageResponse {
  id: string;
  role: string;
  content: string;
  timestamp: string;
  metadata?: Record<string, unknown>;
}

/**
 * Extended message response for session-scoped queries.
 * Includes execution metadata (agent_id, delegation_type) for context.
 */
export interface SessionMessage {
  id: string;
  execution_id: string;
  agent_id: string;
  delegation_type: string;
  role: string;
  content: string;
  created_at: string;
  tool_calls?: unknown;
  tool_results?: unknown;
}

/**
 * Scope for session messages query.
 */
export type MessageScope = 'all' | 'root' | 'execution' | 'delegates';

/**
 * Query parameters for session messages endpoint.
 */
export interface SessionMessagesQuery {
  scope?: MessageScope;
  execution_id?: string;
  agent_id?: string;
}

// ============================================================================
// Gateway Status Types
// ============================================================================

export interface HealthResponse {
  status: string;
  version: string;
  uptime: number;
}

export interface StatusResponse {
  status: string;
  websocket_port: number;
  http_port: number;
  active_connections: number;
  active_executions: number;
}

// ============================================================================
// Skill Types
// ============================================================================

export interface SkillResponse {
  id: string;
  name: string;
  displayName: string;
  description: string;
  category: string;
  instructions: string;
  createdAt?: string;
}

export interface CreateSkillRequest {
  name: string;
  displayName?: string;
  description?: string;
  category?: string;
  instructions?: string;
}

export interface UpdateSkillRequest {
  name?: string;
  displayName?: string;
  description?: string;
  category?: string;
  instructions?: string;
}

// ============================================================================
// Provider Types
// ============================================================================

export interface ProviderResponse {
  id?: string;
  name: string;
  description: string;
  apiKey: string;
  baseUrl: string;
  models: string[];
  embeddingModels?: string[];
  defaultModel?: string;
  verified?: boolean;
  isDefault?: boolean;
  createdAt?: string;
  rateLimits?: RateLimits;
  modelConfigs?: Record<string, ModelConfig>;
}

export interface CreateProviderRequest {
  id?: string;
  name: string;
  description: string;
  apiKey: string;
  baseUrl: string;
  models: string[];
  embeddingModels?: string[];
  defaultModel?: string;
  rateLimits?: RateLimits;
  modelConfigs?: Record<string, ModelConfig>;
}

export interface UpdateProviderRequest {
  name?: string;
  description?: string;
  apiKey?: string;
  baseUrl?: string;
  models?: string[];
  embeddingModels?: string[];
  defaultModel?: string;
  rateLimits?: RateLimits;
  modelConfigs?: Record<string, ModelConfig>;
}

/** Get the default model for a provider response. */
export function getProviderDefaultModel(provider: ProviderResponse): string {
  return provider.defaultModel || provider.models[0] || "";
}

export interface ProviderTestResult {
  success: boolean;
  message: string;
  models?: string[];
}

// ============================================================================
// Model Registry Types
// ============================================================================

export interface ModelProfile {
  name: string;
  provider: string;
  capabilities: ModelCapabilities;
  context: ContextWindow;
  embedding?: EmbeddingSpec;
}

export interface ModelCapabilities {
  tools: boolean;
  vision: boolean;
  thinking: boolean;
  embeddings: boolean;
  voice: boolean;
  imageGeneration: boolean;
  videoGeneration: boolean;
}

export interface RateLimits {
  requestsPerMinute: number;
  concurrentRequests: number;
}

export interface ModelConfig {
  capabilities: ModelCapabilities;
  maxInput?: number;
  maxOutput?: number;
  source: "registry" | "discovered" | "user";
}

export interface ContextWindow {
  input: number;
  output: number | null;
}

export interface EmbeddingSpec {
  dimensions: number;
  maxDimensions?: number;
}

/** Full registry response: model ID → profile */
export type ModelRegistryResponse = Record<string, ModelProfile>;

// ============================================================================
// MCP Types
// ============================================================================

export interface McpServerSummary {
  id: string;
  name: string;
  description: string;
  type: string;
  enabled: boolean;
}

export interface McpListResponse {
  servers: McpServerSummary[];
}

export interface CreateMcpRequest {
  type: "stdio" | "http" | "sse" | "streamable-http";
  id?: string;
  name: string;
  description: string;
  // stdio fields
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  // http/sse/streamable-http fields
  url?: string;
  headers?: Record<string, string>;
  enabled?: boolean;
}

export interface McpServerConfig {
  type: "stdio" | "http" | "sse" | "streamable-http";
  id?: string;
  name: string;
  description: string;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
  headers?: Record<string, string>;
  enabled: boolean;
  validated?: boolean;
}

export interface McpTestResult {
  success: boolean;
  message: string;
  tools?: string[];
}

// ============================================================================
// Settings Types
// ============================================================================

export interface ToolSettings {
  /** Enable python tool (run Python scripts) */
  python: boolean;
  /** Enable web_fetch tool (HTTP requests — large responses can cause context explosion) */
  webFetch: boolean;
  /** Enable UI tools (request_input, show_content) */
  uiTools: boolean;
  /** Enable create_agent tool */
  createAgent: boolean;
  /** Enable introspection tools (list_tools, list_mcps) */
  introspection: boolean;
  /** Enable file tools (read, write, edit, glob) as separate tools */
  fileTools: boolean;
  /** Enable heavyweight todos tool (SQLite-like task persistence) */
  todos: boolean;
  /** Offload large tool results to filesystem instead of keeping in context */
  offloadLargeResults: boolean;
  /** Token threshold for offloading (default: 5000 tokens ≈ 20000 chars) */
  offloadThresholdTokens: number;
}

export interface ToolSettingsResponse {
  success: boolean;
  data?: ToolSettings;
  error?: string;
}

/** Log settings for daemon file logging */
export interface LogSettings {
  /** Enable file logging */
  enabled: boolean;
  /** Custom log directory (null = default {data_dir}/logs) */
  directory: string | null;
  /** Log level: trace, debug, info, warn, error */
  level: "trace" | "debug" | "info" | "warn" | "error";
  /** Rotation strategy: daily, hourly, minutely, never */
  rotation: "daily" | "hourly" | "minutely" | "never";
  /** Maximum log files to keep (0 = unlimited) */
  maxFiles: number;
  /** Suppress stdout output (only log to file) */
  suppressStdout: boolean;
}

/** Response from log settings API (includes restart warning) */
export interface LogSettingsResponse {
  success: boolean;
  data?: LogSettings & { restartRequired: boolean };
  error?: string;
}

/** Request to update log settings */
export interface UpdateLogSettingsRequest {
  enabled?: boolean;
  directory?: string | null;
  level?: "trace" | "debug" | "info" | "warn" | "error";
  rotation?: "daily" | "hourly" | "minutely" | "never";
  maxFiles?: number;
  suppressStdout?: boolean;
}

/** Orchestrator (root agent) configuration */
export interface OrchestratorConfig {
  /** Provider ID. null = use default provider */
  providerId?: string | null;
  /** Model. null = use provider's default model */
  model?: string | null;
  /** Temperature (0-2). Default: 0.7 */
  temperature: number;
  /** Max output tokens. Default: 16384 */
  maxTokens: number;
  /** Enable extended thinking/reasoning. Default: true */
  thinkingEnabled: boolean;
}

/** Distillation model configuration (provider/model override) */
export interface DistillationConfig {
  /** Provider ID override. null = inherit from orchestrator */
  providerId?: string | null;
  /** Model override. null = inherit from orchestrator */
  model?: string | null;
}

/** Execution settings for controlling agent concurrency */
export interface ExecutionSettings {
  /** Maximum parallel subagents across all sessions (default: 2) */
  maxParallelAgents: number;
  /** Whether the first-time setup wizard has been completed (default: false) */
  setupComplete: boolean;
  /** The user-chosen name for the root agent */
  agentName?: string;
  /** Orchestrator (root agent) configuration */
  orchestrator?: OrchestratorConfig;
  /** Distillation model configuration (inherits from orchestrator by default) */
  distillation?: DistillationConfig;
}

export interface ExecutionSettingsResponse {
  success: boolean;
  data?: ExecutionSettings & { restartRequired: boolean };
  error?: string;
}

/** Setup wizard status check */
export interface SetupStatus {
  setupComplete: boolean;
  hasProviders: boolean;
}

// ============================================================================
// Event Types
// ============================================================================

export interface StreamEvent {
  type: string;
  timestamp: number;
  [key: string]: unknown;
}

export type EventCallback = (event: StreamEvent) => void;
export type UnsubscribeFn = () => void;

// ============================================================================
// Execution Log Types
// ============================================================================

export type LogLevel = "debug" | "info" | "warn" | "error";
export type LogCategory = "session" | "token" | "tool_call" | "tool_result" | "delegation" | "error" | "thinking" | "system" | "response" | "intent";
export type SessionStatus = "running" | "completed" | "error" | "stopped";

/** A single execution log entry (snake_case from API) */
export interface ExecutionLog {
  id: string;
  session_id: string;
  conversation_id: string;
  agent_id: string;
  parent_session_id?: string;
  timestamp: string;
  level: LogLevel;
  category: LogCategory;
  message: string;
  metadata?: Record<string, unknown>;
  duration_ms?: number;
}

/** Session summary (snake_case from API) */
export interface LogSession {
  session_id: string;
  conversation_id: string;
  agent_id: string;
  agent_name: string;
  /** Title derived from the first user message in the session */
  title?: string;
  parent_session_id?: string;
  started_at: string;
  ended_at?: string;
  status: SessionStatus;
  token_count: number;
  tool_call_count: number;
  error_count: number;
  duration_ms?: number;
  child_session_ids: string[];
}

/** Session with all its logs */
export interface SessionDetail {
  session: LogSession;
  logs: ExecutionLog[];
}

/** Filter for querying logs */
export interface LogFilter {
  agent_id?: string;
  level?: LogLevel;
  from_time?: string;
  to_time?: string;
  limit?: number;
  offset?: number;
}

// ============================================================================
// Execution State Types (V2 API)
// ============================================================================

/** Session state status (top-level container) - different from LogSession status */
export type SessionStateStatus = "queued" | "running" | "paused" | "completed" | "crashed";

/** Execution status (agent participation) */
export type ExecutionStatus =
  | "queued"
  | "running"
  | "paused"
  | "crashed"
  | "cancelled"
  | "completed";

/** Delegation type */
export type DelegationType = "root" | "sequential" | "parallel";

/** Session - top-level work container (V2 API) */
/** Trigger source for a session */
export type TriggerSource = "web" | "cli" | "cron" | "api" | "connector";

export interface Session {
  id: string;
  status: SessionStateStatus;
  /** Trigger source (web, cli, cron, api, connector) */
  source: TriggerSource;
  root_agent_id: string;
  title?: string;
  created_at: string;
  started_at?: string;
  completed_at?: string;
  total_tokens_in: number;
  total_tokens_out: number;
  metadata?: Record<string, unknown>;
}

/** Agent Execution - agent's participation in a session (V2 API) */
export interface AgentExecution {
  id: string;
  session_id: string;
  agent_id: string;
  parent_execution_id?: string;
  delegation_type: DelegationType;
  task?: string;
  status: ExecutionStatus;
  started_at?: string;
  completed_at?: string;
  tokens_in: number;
  tokens_out: number;
  error?: string;
}

/** Session with all its executions (V2 API response) */
export interface SessionWithExecutions {
  id: string;
  status: SessionStateStatus;
  /** Trigger source (web, cli, cron, api, connector) */
  source: TriggerSource;
  root_agent_id: string;
  title?: string;
  created_at: string;
  started_at?: string;
  completed_at?: string;
  total_tokens_in: number;
  total_tokens_out: number;
  metadata?: Record<string, unknown>;
  executions: AgentExecution[];
  subagent_count: number;
}

/** Filter for querying sessions */
export interface SessionFilter {
  status?: SessionStateStatus;
  root_agent_id?: string;
  from_time?: string;
  to_time?: string;
  limit?: number;
  offset?: number;
}

/** Filter for querying executions */
export interface ExecutionFilter {
  session_id?: string;
  agent_id?: string;
  status?: ExecutionStatus;
  limit?: number;
  offset?: number;
}

/** Dashboard stats (V2 - session + execution counts) */
export interface DashboardStats {
  sessions_queued: number;
  sessions_running: number;
  sessions_paused: number;
  sessions_completed: number;
  sessions_crashed: number;
  executions_queued: number;
  executions_running: number;
  executions_completed: number;
  executions_crashed: number;
  executions_cancelled: number;
  today_sessions: number;
  today_tokens: number;
  /** Sessions count by trigger source (e.g., { web: 5, connector: 2 }) */
  sessions_by_source: Record<TriggerSource, number>;
}

// ============================================================================
// Legacy Types (for backwards compatibility during migration)
// ============================================================================

/** @deprecated Use SessionWithExecutions instead */
export interface ExecutionSession {
  id: string;
  conversation_id: string;
  agent_id: string;
  parent_session_id?: string;
  status: ExecutionStatus;
  created_at: string;
  started_at?: string;
  completed_at?: string;
  tokens_in: number;
  tokens_out: number;
  checkpoint?: string;
  error?: string;
}

/** @deprecated Use SessionFilter instead */
export interface ExecutionSessionFilter {
  agent_id?: string;
  status?: ExecutionStatus;
  limit?: number;
  offset?: number;
}

/** @deprecated Use DashboardStats instead */
export type ExecutionStats = Record<string, number>;

// ============================================================================
// Connection State Types (for subscription-based event routing)
// ============================================================================

/**
 * Connection state for WebSocket transport.
 * Used to track connection lifecycle and show appropriate UI.
 */
export type ConnectionState =
  | { status: "disconnected"; reason?: "user" | "server" | "network" }
  | { status: "connecting" }
  | { status: "connected" }
  | { status: "reconnecting"; attempt: number; maxAttempts?: number }
  | { status: "failed"; error: string };

/**
 * Subscription error codes from the server.
 */
export type SubscriptionErrorCode =
  | "NOT_FOUND"
  | "LIMIT_EXCEEDED"
  | "SERVER_ERROR";

/**
 * Subscription error message from server.
 */
export interface SubscriptionErrorMessage {
  type: "subscription_error";
  conversation_id: string;
  code: SubscriptionErrorCode;
  message: string;
}

/**
 * Conversation event with session/execution identifiers for routing and filtering.
 *
 * - `session_id`: Top-level session ID (primary routing key)
 * - `execution_id`: Specific execution ID (for filtering root vs subagent)
 * - `conversation_id`: Legacy field for backward compatibility
 */
export interface ConversationEvent extends StreamEvent {
  /** Session ID for subscription routing */
  session_id: string;
  /** Execution ID for filtering (root vs subagent) */
  execution_id: string;
  /** Legacy conversation ID for backward compatibility */
  conversation_id?: string;
  /** Sequence number for ordering events */
  seq?: number;
}

/**
 * Global event (stats updates, notifications).
 */
export interface GlobalEvent extends StreamEvent {
  type: "stats_update" | "session_notification";
}

/**
 * Callback for conversation-specific events.
 */
export type ConversationCallback = (event: ConversationEvent) => void;

/**
 * Callback for global events.
 */
export type GlobalCallback = (event: GlobalEvent) => void;

/**
 * Callback for connection state changes.
 */
export type ConnectionStateCallback = (state: ConnectionState) => void;

/**
 * Subscription scope for event filtering.
 *
 * - `all`: All events (backward compatible, includes subagent events)
 * - `session`: Root execution events + delegation lifecycle markers only
 * - `execution:{id}`: All events for a specific execution
 */
export type SubscriptionScope = "all" | "session" | `execution:${string}`;

/**
 * Options for subscribing to conversation events.
 */
export interface SubscriptionOptions {
  /** Called when an event is received for this conversation */
  onEvent: ConversationCallback;
  /** Called when a subscription error occurs */
  onError?: (error: SubscriptionErrorMessage) => void;
  /** Called when subscription is confirmed with current sequence */
  onConfirmed?: (seq: number, rootExecutionIds?: string[]) => void;
  /** Subscription scope - defaults to "all" for backward compatibility */
  scope?: SubscriptionScope;
}

// ============================================================================
// Plugin Types
// ============================================================================

/** Plugin status from GET /api/plugins */
export interface PluginInfo {
  id: string;
  name: string;
  version: string;
  description: string;
  state: "running" | "stopped" | "failed" | "starting";
  auto_restart: boolean;
  enabled: boolean;
  error?: string;
}

export interface PluginsResponse {
  plugins: PluginInfo[];
  total: number;
}

// ============================================================================
// Bridge Worker Types
// ============================================================================

/** Capability declared by a bridge worker */
export interface BridgeWorkerCapability {
  name: string;
  description?: string;
  schema?: Record<string, unknown>;
}

/** Resource declared by a bridge worker */
export interface BridgeWorkerResource {
  name: string;
  description?: string;
}

/** Connected bridge worker (read-only, from GET /api/bridge/workers) */
export interface BridgeWorker {
  adapter_id: string;
  capabilities: BridgeWorkerCapability[];
  resources: BridgeWorkerResource[];
  connected_at: string;
}

// ============================================================================
// Cron Job Types
// ============================================================================

/** Cron job configuration */
export interface CronJobResponse {
  id: string;
  name: string;
  schedule: string;
  agent_id: string;
  message: string;
  respond_to: string[];
  enabled: boolean;
  timezone?: string;
  metadata?: Record<string, unknown>;
  last_run?: string;
  next_run?: string;
  created_at?: string;
  updated_at?: string;
}

/** Request to create a new cron job */
export interface CreateCronJobRequest {
  id: string;
  name: string;
  schedule: string;
  agent_id: string;
  message: string;
  respond_to?: string[];
  enabled?: boolean;
  timezone?: string;
  metadata?: Record<string, unknown>;
}

/** Request to update a cron job */
export interface UpdateCronJobRequest {
  name?: string;
  schedule?: string;
  agent_id?: string;
  message?: string;
  respond_to?: string[];
  enabled?: boolean;
  timezone?: string;
  metadata?: Record<string, unknown>;
}

/** Result of triggering a cron job manually */
export interface CronTriggerResult {
  success: boolean;
  session_id?: string;
  execution_id?: string;
  message: string;
}

// ============================================================================
// Memory Types
// ============================================================================

/** Memory fact scope - determines visibility of the fact */
export type MemoryScope = "agent" | "shared" | "ward";

/** Memory fact category - type of information stored */
export type MemoryCategory =
  | "preference"
  | "decision"
  | "pattern"
  | "entity"
  | "instruction"
  | "correction"
  | "user"
  | "domain"
  | "strategy"
  | "skill"
  | "agent"
  | "ward";

/** A memory fact stored in the agent's memory system */
export interface MemoryFact {
  id: string;
  agent_id: string;
  scope: MemoryScope;
  category: MemoryCategory;
  key: string;
  content: string;
  confidence: number;
  mention_count: number;
  source_summary?: string;
  contradicted_by?: string;
  created_at: string;
  updated_at: string;
  /** Pinned facts can't be overwritten by distillation. User-authored facts are pinned. */
  pinned?: boolean;
}

/** Filter options for listing memory facts */
export interface MemoryFilter {
  /** Optional agent filter - when provided, only that agent's memories are returned */
  agent_id?: string;
  category?: MemoryCategory;
  scope?: MemoryScope;
  limit?: number;
  offset?: number;
}

/** Response for memory list operations */
export interface MemoryListResponse {
  facts: MemoryFact[];
  total: number;
}

// ============================================================================
// Knowledge Graph Types
// ============================================================================

/** Graph statistics response */
export interface GraphStatsResponse {
  entity_count: number;
  relationship_count: number;
  entity_types: Record<string, number>;
  relationship_types: Record<string, number>;
  most_connected_entities: Array<[string, number]>;
}

/** Graph entity */
export interface GraphEntity {
  id: string;
  agent_id: string;
  entity_type: string;
  name: string;
  properties: Record<string, unknown>;
  mention_count: number;
  first_seen_at: string;
  last_seen_at: string;
}

/** Graph relationship */
export interface GraphRelationship {
  id: string;
  agent_id: string;
  source_entity_id: string;
  target_entity_id: string;
  relationship_type: string;
  mention_count: number;
}

/** Entity list response */
export interface GraphEntityListResponse {
  entities: GraphEntity[];
  total: number;
}

/** Relationship list response */
export interface GraphRelationshipListResponse {
  relationships: GraphRelationship[];
  total: number;
}

/** Filter for entity queries */
export interface GraphEntityFilter {
  entity_type?: string;
  limit?: number;
  offset?: number;
}

/** Filter for relationship queries */
export interface GraphRelationshipFilter {
  relationship_type?: string;
  limit?: number;
  offset?: number;
}

/** Neighbor entry in neighbor response */
export interface GraphNeighborEntry {
  entity: GraphEntity;
  relationship: GraphRelationship;
  direction: "incoming" | "outgoing";
}

/** Neighbor response */
export interface GraphNeighborResponse {
  entity_id: string;
  neighbors: GraphNeighborEntry[];
}

/** Subgraph response */
export interface GraphSubgraphResponse {
  entities: GraphEntity[];
  relationships: GraphRelationship[];
  center: string;
  max_hops: number;
}

/** Options for neighbor queries */
export interface GraphNeighborOptions {
  direction?: "incoming" | "outgoing" | "both";
  limit?: number;
}

/** Options for subgraph queries */
export interface GraphSubgraphOptions {
  max_hops?: number;
}
