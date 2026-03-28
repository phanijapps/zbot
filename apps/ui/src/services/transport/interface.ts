// ============================================================================
// TRANSPORT INTERFACE
// Defines the common interface for Tauri and Web transports
// ============================================================================

import type {
  TransportConfig,
  TransportResult,
  AgentResponse,
  CreateAgentRequest,
  UpdateAgentRequest,
  SkillResponse,
  CreateSkillRequest,
  UpdateSkillRequest,
  ProviderResponse,
  CreateProviderRequest,
  UpdateProviderRequest,
  ProviderTestResult,
  HealthResponse,
  StatusResponse,
  EventCallback,
  UnsubscribeFn,
  McpListResponse,
  McpServerConfig,
  CreateMcpRequest,
  McpTestResult,
  ModelRegistryResponse,
  MessageResponse,
  SessionMessage,
  SessionMessagesQuery,
  ToolSettings,
  LogSettings,
  UpdateLogSettingsRequest,
  LogSession,
  SessionDetail,
  LogFilter,
  // V2 types
  SessionWithExecutions,
  SessionFilter,
  DashboardStats,
  // Legacy types
  ExecutionSession,
  ExecutionSessionFilter,
  ExecutionStats,
  // Subscription types
  ConnectionState,
  ConnectionStateCallback,
  GlobalCallback,
  SubscriptionOptions,
  // Bridge worker types
  BridgeWorker,
  // Cron types
  CronJobResponse,
  CreateCronJobRequest,
  UpdateCronJobRequest,
  CronTriggerResult,
  // Memory types
  MemoryFact,
  MemoryFilter,
  MemoryListResponse,
  // Graph types
  GraphStatsResponse,
  GraphEntityListResponse,
  GraphRelationshipListResponse,
  GraphEntityFilter,
  GraphRelationshipFilter,
  GraphNeighborResponse,
  GraphSubgraphResponse,
  GraphNeighborOptions,
  GraphSubgraphOptions,
} from "./types";

// ============================================================================
// Transport Interface
// ============================================================================

export interface Transport {
  /** Get the transport mode */
  readonly mode: "web";

  /** Initialize the transport with configuration */
  initialize(config: TransportConfig): Promise<void>;

  /** Check if the gateway is healthy */
  health(): Promise<TransportResult<HealthResponse>>;

  /** Get gateway status */
  status(): Promise<TransportResult<StatusResponse>>;

  // =========================================================================
  // Agent Operations
  // =========================================================================

  /** List all agents */
  listAgents(): Promise<TransportResult<AgentResponse[]>>;

  /** Get an agent by ID */
  getAgent(id: string): Promise<TransportResult<AgentResponse>>;

  /** Create a new agent */
  createAgent(request: CreateAgentRequest): Promise<TransportResult<AgentResponse>>;

  /** Update an existing agent */
  updateAgent(id: string, request: UpdateAgentRequest): Promise<TransportResult<AgentResponse>>;

  /** Delete an agent */
  deleteAgent(id: string): Promise<TransportResult<void>>;

  // =========================================================================
  // Skill Operations
  // =========================================================================

  /** List all skills */
  listSkills(): Promise<TransportResult<SkillResponse[]>>;

  /** Get a skill by ID */
  getSkill(id: string): Promise<TransportResult<SkillResponse>>;

  /** Create a new skill */
  createSkill(request: CreateSkillRequest): Promise<TransportResult<SkillResponse>>;

  /** Update an existing skill */
  updateSkill(id: string, request: UpdateSkillRequest): Promise<TransportResult<SkillResponse>>;

  /** Delete a skill */
  deleteSkill(id: string): Promise<TransportResult<void>>;

  // =========================================================================
  // Provider Operations
  // =========================================================================

  /** List all providers */
  listProviders(): Promise<TransportResult<ProviderResponse[]>>;

  /** Get a provider by ID */
  getProvider(id: string): Promise<TransportResult<ProviderResponse>>;

  /** Create a new provider */
  createProvider(request: CreateProviderRequest): Promise<TransportResult<ProviderResponse>>;

  /** Update an existing provider */
  updateProvider(id: string, request: UpdateProviderRequest): Promise<TransportResult<ProviderResponse>>;

  /** Delete a provider */
  deleteProvider(id: string): Promise<TransportResult<void>>;

  /** Test a provider connection */
  testProvider(provider: CreateProviderRequest): Promise<TransportResult<ProviderTestResult>>;

  /** Set a provider as the default */
  setDefaultProvider(id: string): Promise<TransportResult<ProviderResponse>>;

  // =========================================================================
  // Model Registry Operations
  // =========================================================================

  /** Get all known models with capabilities */
  listModels(): Promise<TransportResult<ModelRegistryResponse>>;

  // =========================================================================
  // MCP Operations
  // =========================================================================

  /** List all MCP servers */
  listMcps(): Promise<TransportResult<McpListResponse>>;

  /** Get an MCP server by ID */
  getMcp(id: string): Promise<TransportResult<McpServerConfig>>;

  /** Create a new MCP server */
  createMcp(request: CreateMcpRequest): Promise<TransportResult<McpServerConfig>>;

  /** Update an existing MCP server */
  updateMcp(id: string, request: CreateMcpRequest): Promise<TransportResult<McpServerConfig>>;

  /** Delete an MCP server */
  deleteMcp(id: string): Promise<TransportResult<void>>;

  /** Test an MCP server connection */
  testMcp(id: string): Promise<TransportResult<McpTestResult>>;

  // =========================================================================
  // Conversation Operations
  // =========================================================================

  /** Get messages for an execution (exec-xxx) or conversation (web-xxx) */
  getMessages(id: string): Promise<TransportResult<MessageResponse[]>>;

  /**
   * Get messages for a session with scope filtering.
   *
   * Scopes:
   * - `all`: All messages from all executions
   * - `root`: Only messages from root executions (main chat view)
   * - `execution`: Messages from a specific execution (requires execution_id)
   * - `delegates`: Only messages from delegated executions
   */
  getSessionMessages(
    sessionId: string,
    query?: SessionMessagesQuery
  ): Promise<TransportResult<SessionMessage[]>>;

  // =========================================================================
  // Agent Execution
  // =========================================================================

  /** Execute an agent with a message */
  executeAgent(
    agentId: string,
    conversationId: string,
    message: string,
    sessionId?: string
  ): Promise<TransportResult<{ conversationId: string; sessionId?: string }>>;

  /** Stop an agent execution */
  stopAgent(conversationId: string): Promise<TransportResult<void>>;

  // =========================================================================
  // Settings Operations
  // =========================================================================

  /** Get tool settings */
  getToolSettings(): Promise<TransportResult<ToolSettings>>;

  /** Update tool settings */
  updateToolSettings(settings: ToolSettings): Promise<TransportResult<ToolSettings>>;

  /** Get log settings */
  getLogSettings(): Promise<TransportResult<LogSettings & { restartRequired: boolean }>>;

  /** Update log settings */
  updateLogSettings(settings: UpdateLogSettingsRequest): Promise<TransportResult<LogSettings & { restartRequired: boolean }>>;

  // =========================================================================
  // Execution Log Operations
  // =========================================================================

  /** List all log sessions */
  listLogSessions(filter?: LogFilter): Promise<TransportResult<LogSession[]>>;

  /** Get a session with its logs */
  getLogSession(sessionId: string): Promise<TransportResult<SessionDetail>>;

  /** Delete a log session */
  deleteLogSession(sessionId: string): Promise<TransportResult<void>>;

  /** Cleanup old logs */
  cleanupOldLogs(olderThanDays: number): Promise<TransportResult<{ deletedCount: number }>>;

  // =========================================================================
  // Session Operations (V2 API)
  // =========================================================================

  /** List sessions with their executions (V2 API - for dashboard) */
  listSessionsFull(filter?: SessionFilter): Promise<TransportResult<SessionWithExecutions[]>>;

  /** Get a single session with executions (V2 API) */
  getSessionFull(sessionId: string): Promise<TransportResult<SessionWithExecutions>>;

  /** Get dashboard stats (V2 API - session + execution counts) */
  getDashboardStats(): Promise<TransportResult<DashboardStats>>;

  // =========================================================================
  // Legacy Execution Session Operations (deprecated)
  // =========================================================================

  /** @deprecated Use listSessionsFull() instead */
  listExecutionSessions(filter?: ExecutionSessionFilter): Promise<TransportResult<ExecutionSession[]>>;

  /** @deprecated Use getSessionFull() instead */
  getExecutionSession(sessionId: string): Promise<TransportResult<ExecutionSession>>;

  /** @deprecated Use getDashboardStats() instead */
  getExecutionStats(): Promise<TransportResult<ExecutionStats>>;

  /** Pause an execution session */
  pauseSession(sessionId: string): Promise<TransportResult<void>>;

  /** Resume a paused execution session */
  resumeSession(sessionId: string): Promise<TransportResult<void>>;

  /** Cancel an execution session */
  cancelSession(sessionId: string): Promise<TransportResult<void>>;

  /** End a session (mark as completed) */
  endSession(sessionId: string): Promise<TransportResult<void>>;

  /** Cleanup old execution sessions */
  cleanupExecutionSessions(olderThan?: string): Promise<TransportResult<{ deleted: number }>>;

  // =========================================================================
  // Event Streaming (Legacy)
  // =========================================================================

  /**
   * Subscribe to events for a conversation (legacy - client-side filtering).
   * @deprecated Use subscribeConversation() instead for server-side routing
   */
  subscribe(conversationId: string, callback: EventCallback): UnsubscribeFn;

  /** Connect to the event stream */
  connect(): Promise<TransportResult<void>>;

  /** Disconnect from the event stream */
  disconnect(): Promise<void>;

  /** Check if connected to event stream */
  isConnected(): boolean;

  // =========================================================================
  // Subscription API (Server-Side Routing)
  // =========================================================================

  /** Get the current connection state */
  getConnectionState(): ConnectionState;

  /** Subscribe to connection state changes */
  onConnectionStateChange(callback: ConnectionStateCallback): UnsubscribeFn;

  /** Subscribe to conversation events with server-side routing */
  subscribeConversation(
    conversationId: string,
    options: SubscriptionOptions
  ): UnsubscribeFn;

  /** Subscribe to global events (stats updates, notifications) */
  onGlobalEvent(callback: GlobalCallback): UnsubscribeFn;

  /** Manual reconnect - resets attempt counter and tries again */
  reconnect(): Promise<void>;

  // =========================================================================
  // Bridge Worker Operations
  // =========================================================================

  /** List all connected bridge workers */
  listBridgeWorkers(): Promise<TransportResult<BridgeWorker[]>>;

  // =========================================================================
  // Cron Job Operations
  // =========================================================================

  /** List all cron jobs */
  listCronJobs(): Promise<TransportResult<CronJobResponse[]>>;

  /** Get a cron job by ID */
  getCronJob(id: string): Promise<TransportResult<CronJobResponse>>;

  /** Create a new cron job */
  createCronJob(request: CreateCronJobRequest): Promise<TransportResult<CronJobResponse>>;

  /** Update an existing cron job */
  updateCronJob(id: string, request: UpdateCronJobRequest): Promise<TransportResult<CronJobResponse>>;

  /** Delete a cron job */
  deleteCronJob(id: string): Promise<TransportResult<void>>;

  /** Manually trigger a cron job */
  triggerCronJob(id: string): Promise<TransportResult<CronTriggerResult>>;

  /** Enable a cron job */
  enableCronJob(id: string): Promise<TransportResult<CronJobResponse>>;

  /** Disable a cron job */
  disableCronJob(id: string): Promise<TransportResult<CronJobResponse>>;

  // =========================================================================
  // Memory Operations
  // =========================================================================

  /** List ALL memory facts across all agents (with optional filter) */
  listAllMemory(filter?: MemoryFilter): Promise<TransportResult<MemoryListResponse>>;

  /** List memory facts for an agent */
  listMemory(agentId: string, filter?: MemoryFilter): Promise<TransportResult<MemoryListResponse>>;

  /** Search memory facts for an agent */
  searchMemory(agentId: string, query: string, filter?: MemoryFilter): Promise<TransportResult<MemoryListResponse>>;

  /** Get a single memory fact */
  getMemory(agentId: string, factId: string): Promise<TransportResult<MemoryFact>>;

  /** Delete a memory fact */
  deleteMemory(agentId: string, factId: string): Promise<TransportResult<void>>;

  // =========================================================================
  // Knowledge Graph Operations
  // =========================================================================

  /** Get graph statistics for an agent */
  getGraphStats(agentId: string): Promise<TransportResult<GraphStatsResponse>>;

  /** List entities for an agent with optional filter */
  getGraphEntities(
    agentId: string,
    filter?: GraphEntityFilter
  ): Promise<TransportResult<GraphEntityListResponse>>;

  /** List relationships for an agent with optional filter */
  getGraphRelationships(
    agentId: string,
    filter?: GraphRelationshipFilter
  ): Promise<TransportResult<GraphRelationshipListResponse>>;

  /** Search entities by name */
  searchGraphEntities(
    agentId: string,
    query: string,
    limit?: number
  ): Promise<TransportResult<GraphEntityListResponse>>;

  /** Get neighbors of an entity */
  getEntityNeighbors(
    agentId: string,
    entityId: string,
    options?: GraphNeighborOptions
  ): Promise<TransportResult<GraphNeighborResponse>>;

  /** Get subgraph around an entity */
  getEntitySubgraph(
    agentId: string,
    entityId: string,
    options?: GraphSubgraphOptions
  ): Promise<TransportResult<GraphSubgraphResponse>>;
}
