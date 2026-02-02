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
  MessageResponse,
  SessionMessage,
  SessionMessagesQuery,
  ToolSettings,
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
} from "./types";

// ============================================================================
// Transport Interface
// ============================================================================

export interface Transport {
  /** Get the transport mode (tauri or web) */
  readonly mode: "tauri" | "web";

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
  // Event Streaming
  // =========================================================================

  /** Subscribe to events for a conversation */
  subscribe(conversationId: string, callback: EventCallback): UnsubscribeFn;

  /** Connect to the event stream */
  connect(): Promise<TransportResult<void>>;

  /** Disconnect from the event stream */
  disconnect(): Promise<void>;

  /** Check if connected to event stream */
  isConnected(): boolean;
}
