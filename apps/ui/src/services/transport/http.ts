// ============================================================================
// HTTP TRANSPORT
// Web-based transport using fetch and WebSocket directly
// ============================================================================

import type { Transport } from "./interface";
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
  StreamEvent,
  McpListResponse,
  McpServerConfig,
  CreateMcpRequest,
  McpTestResult,
  MessageResponse,
  SessionMessage,
  SessionMessagesQuery,
  ToolSettings,
  ToolSettingsResponse,
  LogSession,
  SessionDetail,
  LogFilter,
  // V2 types
  SessionWithExecutions,
  SessionFilter,
  DashboardStats,
  // Legacy types (for backwards compatibility)
  ExecutionSession,
  ExecutionSessionFilter,
  ExecutionStats,
  // Subscription types
  ConnectionState,
  ConnectionStateCallback,
  ConversationCallback,
  ConversationEvent,
  GlobalCallback,
  GlobalEvent,
  SubscriptionErrorMessage,
  SubscriptionOptions,
} from "./types";

// ============================================================================
// HTTP Transport Implementation
// ============================================================================

// Internal subscription state for a conversation
interface SubscriptionState {
  callbacks: Set<ConversationCallback>;
  errorCallbacks: Map<ConversationCallback, (error: SubscriptionErrorMessage) => void>;
  confirmedCallbacks: Map<ConversationCallback, (seq: number) => void>;
  confirmed: boolean;
  lastSeq: number;
}

export class HttpTransport implements Transport {
  readonly mode = "web" as const;

  private config: TransportConfig | null = null;
  private ws: WebSocket | null = null;
  private eventCallbacks: Map<string, Set<EventCallback>> = new Map();
  private globalCallbacks: Set<EventCallback> = new Set();
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectDelay = 1000;

  // ─────────────────────────────────────────────────────────────────────────
  // Subscription state (server-side routing)
  // ─────────────────────────────────────────────────────────────────────────
  private conversationSubscriptions: Map<string, SubscriptionState> = new Map();
  private pendingSubscriptions: Set<string> = new Set(); // Queued when WS not ready
  private globalEventCallbacks: Set<GlobalCallback> = new Set();
  private connectionStateCallbacks: Set<ConnectionStateCallback> = new Set();
  private connectionState: ConnectionState = { status: "disconnected" };

  // Heartbeat
  private pingInterval: ReturnType<typeof setInterval> | null = null;
  private lastPong: number = Date.now();
  private readonly PING_INTERVAL = 15000;
  private readonly PONG_TIMEOUT = 30000;

  // Browser event handlers (stored for cleanup)
  private visibilityHandler: (() => void) | null = null;
  private onlineHandler: (() => void) | null = null;

  // =========================================================================
  // Initialization
  // =========================================================================

  async initialize(config: TransportConfig): Promise<void> {
    this.config = config;
  }

  // =========================================================================
  // Health & Status
  // =========================================================================

  async health(): Promise<TransportResult<HealthResponse>> {
    return this.get<HealthResponse>("/api/health");
  }

  async status(): Promise<TransportResult<StatusResponse>> {
    return this.get<StatusResponse>("/api/status");
  }

  // =========================================================================
  // Agent Operations
  // =========================================================================

  async listAgents(): Promise<TransportResult<AgentResponse[]>> {
    return this.get<AgentResponse[]>("/api/agents");
  }

  async getAgent(id: string): Promise<TransportResult<AgentResponse>> {
    return this.get<AgentResponse>(`/api/agents/${encodeURIComponent(id)}`);
  }

  async createAgent(request: CreateAgentRequest): Promise<TransportResult<AgentResponse>> {
    return this.post<AgentResponse>("/api/agents", request);
  }

  async updateAgent(id: string, request: UpdateAgentRequest): Promise<TransportResult<AgentResponse>> {
    return this.put<AgentResponse>(`/api/agents/${encodeURIComponent(id)}`, request);
  }

  async deleteAgent(id: string): Promise<TransportResult<void>> {
    return this.delete(`/api/agents/${encodeURIComponent(id)}`);
  }

  // =========================================================================
  // Skill Operations
  // =========================================================================

  async listSkills(): Promise<TransportResult<SkillResponse[]>> {
    return this.get<SkillResponse[]>("/api/skills");
  }

  async getSkill(id: string): Promise<TransportResult<SkillResponse>> {
    return this.get<SkillResponse>(`/api/skills/${encodeURIComponent(id)}`);
  }

  async createSkill(request: CreateSkillRequest): Promise<TransportResult<SkillResponse>> {
    return this.post<SkillResponse>("/api/skills", request);
  }

  async updateSkill(id: string, request: UpdateSkillRequest): Promise<TransportResult<SkillResponse>> {
    return this.put<SkillResponse>(`/api/skills/${encodeURIComponent(id)}`, request);
  }

  async deleteSkill(id: string): Promise<TransportResult<void>> {
    return this.delete(`/api/skills/${encodeURIComponent(id)}`);
  }

  // =========================================================================
  // Provider Operations
  // =========================================================================

  async listProviders(): Promise<TransportResult<ProviderResponse[]>> {
    return this.get<ProviderResponse[]>("/api/providers");
  }

  async getProvider(id: string): Promise<TransportResult<ProviderResponse>> {
    return this.get<ProviderResponse>(`/api/providers/${encodeURIComponent(id)}`);
  }

  async createProvider(request: CreateProviderRequest): Promise<TransportResult<ProviderResponse>> {
    return this.post<ProviderResponse>("/api/providers", request);
  }

  async updateProvider(id: string, request: UpdateProviderRequest): Promise<TransportResult<ProviderResponse>> {
    return this.put<ProviderResponse>(`/api/providers/${encodeURIComponent(id)}`, request);
  }

  async deleteProvider(id: string): Promise<TransportResult<void>> {
    return this.delete(`/api/providers/${encodeURIComponent(id)}`);
  }

  async testProvider(provider: CreateProviderRequest): Promise<TransportResult<ProviderTestResult>> {
    return this.post<ProviderTestResult>("/api/providers/test", provider);
  }

  async setDefaultProvider(id: string): Promise<TransportResult<ProviderResponse>> {
    return this.post<ProviderResponse>(`/api/providers/${encodeURIComponent(id)}/default`, {});
  }

  // =========================================================================
  // MCP Operations
  // =========================================================================

  async listMcps(): Promise<TransportResult<McpListResponse>> {
    return this.get<McpListResponse>("/api/mcps");
  }

  async getMcp(id: string): Promise<TransportResult<McpServerConfig>> {
    return this.get<McpServerConfig>(`/api/mcps/${encodeURIComponent(id)}`);
  }

  async createMcp(request: CreateMcpRequest): Promise<TransportResult<McpServerConfig>> {
    return this.post<McpServerConfig>("/api/mcps", request);
  }

  async updateMcp(id: string, request: CreateMcpRequest): Promise<TransportResult<McpServerConfig>> {
    return this.put<McpServerConfig>(`/api/mcps/${encodeURIComponent(id)}`, request);
  }

  async deleteMcp(id: string): Promise<TransportResult<void>> {
    return this.delete(`/api/mcps/${encodeURIComponent(id)}`);
  }

  async testMcp(id: string): Promise<TransportResult<McpTestResult>> {
    return this.post<McpTestResult>(`/api/mcps/${encodeURIComponent(id)}/test`, {});
  }

  // =========================================================================
  // Conversation Operations
  // =========================================================================

  async getMessages(id: string): Promise<TransportResult<MessageResponse[]>> {
    // Route based on ID format:
    // - exec-{uuid} → execution messages (from dashboard/session history)
    // - other (web-xxx, etc.) → conversation messages (from active chat)
    if (id.startsWith('exec-')) {
      return this.get<MessageResponse[]>(`/api/executions/${encodeURIComponent(id)}/messages`);
    }
    return this.get<MessageResponse[]>(`/api/conversations/${encodeURIComponent(id)}/messages`);
  }

  /**
   * Get messages for a session with scope filtering.
   *
   * Scopes:
   * - `all`: All messages from all executions
   * - `root`: Only messages from root executions (main chat view)
   * - `execution`: Messages from a specific execution (requires execution_id)
   * - `delegates`: Only messages from delegated executions
   */
  async getSessionMessages(
    sessionId: string,
    query?: SessionMessagesQuery
  ): Promise<TransportResult<SessionMessage[]>> {
    const params = new URLSearchParams();
    if (query?.scope) params.set('scope', query.scope);
    if (query?.execution_id) params.set('execution_id', query.execution_id);
    if (query?.agent_id) params.set('agent_id', query.agent_id);

    const queryString = params.toString();
    const url = `/api/executions/v2/sessions/${encodeURIComponent(sessionId)}/messages${queryString ? `?${queryString}` : ''}`;

    return this.get<SessionMessage[]>(url);
  }

  // =========================================================================
  // Settings Operations
  // =========================================================================

  async getToolSettings(): Promise<TransportResult<ToolSettings>> {
    const result = await this.get<ToolSettingsResponse>("/api/settings/tools");
    if (result.success && result.data?.success && result.data.data) {
      return { success: true, data: result.data.data };
    }
    return { success: false, error: result.error || result.data?.error || "Failed to get tool settings" };
  }

  async updateToolSettings(settings: ToolSettings): Promise<TransportResult<ToolSettings>> {
    const result = await this.put<ToolSettingsResponse>("/api/settings/tools", settings);
    if (result.success && result.data?.success && result.data.data) {
      return { success: true, data: result.data.data };
    }
    return { success: false, error: result.error || result.data?.error || "Failed to update tool settings" };
  }

  // =========================================================================
  // Execution Log Operations
  // =========================================================================

  async listLogSessions(filter?: LogFilter): Promise<TransportResult<LogSession[]>> {
    const params = new URLSearchParams();
    if (filter?.agent_id) params.set("agent_id", filter.agent_id);
    if (filter?.level) params.set("level", filter.level);
    if (filter?.from_time) params.set("from_time", filter.from_time);
    if (filter?.to_time) params.set("to_time", filter.to_time);
    if (filter?.limit) params.set("limit", String(filter.limit));
    if (filter?.offset) params.set("offset", String(filter.offset));
    
    const query = params.toString();
    const url = query ? `/api/logs/sessions?${query}` : "/api/logs/sessions";
    return this.get<LogSession[]>(url);
  }

  async getLogSession(sessionId: string): Promise<TransportResult<SessionDetail>> {
    return this.get<SessionDetail>(`/api/logs/sessions/${encodeURIComponent(sessionId)}`);
  }

  async deleteLogSession(sessionId: string): Promise<TransportResult<void>> {
    return this.delete(`/api/logs/sessions/${encodeURIComponent(sessionId)}`);
  }

  async cleanupOldLogs(olderThanDays: number): Promise<TransportResult<{ deletedCount: number }>> {
    if (!this.config) {
      return { success: false, error: "Transport not initialized" };
    }

    try {
      // Calculate timestamp from days ago
      const olderThan = new Date(Date.now() - olderThanDays * 24 * 60 * 60 * 1000).toISOString();
      const response = await fetch(`${this.config.httpUrl}/api/logs/cleanup?older_than=${encodeURIComponent(olderThan)}`, {
        method: "DELETE",
        headers: {
          "Content-Type": "application/json",
        },
      });

      if (!response.ok) {
        const text = await response.text();
        return { success: false, error: text || `HTTP ${response.status}` };
      }

      const data = await response.json();
      return { success: true, data };
    } catch (error) {
      return { success: false, error: String(error) };
    }
  }

  // =========================================================================
  // Session Operations (V2 API)
  // =========================================================================

  /** List sessions with their executions (V2 API - use this for dashboard) */
  async listSessionsFull(filter?: SessionFilter): Promise<TransportResult<SessionWithExecutions[]>> {
    const params = new URLSearchParams();
    if (filter?.status) params.set("status", filter.status);
    if (filter?.root_agent_id) params.set("root_agent_id", filter.root_agent_id);
    if (filter?.limit) params.set("limit", String(filter.limit));
    if (filter?.offset) params.set("offset", String(filter.offset));

    const query = params.toString();
    const url = query ? `/api/executions/v2/sessions/full?${query}` : "/api/executions/v2/sessions/full";
    return this.get<SessionWithExecutions[]>(url);
  }

  /** Get a single session with executions (V2 API) */
  async getSessionFull(sessionId: string): Promise<TransportResult<SessionWithExecutions>> {
    return this.get<SessionWithExecutions>(`/api/executions/v2/sessions/${encodeURIComponent(sessionId)}/full`);
  }

  /** Get dashboard stats (V2 API - session + execution counts) */
  async getDashboardStats(): Promise<TransportResult<DashboardStats>> {
    return this.get<DashboardStats>("/api/executions/stats");
  }

  // =========================================================================
  // Legacy Execution Session Operations (deprecated)
  // =========================================================================

  /** @deprecated Use listSessionsFull() instead */
  async listExecutionSessions(filter?: ExecutionSessionFilter): Promise<TransportResult<ExecutionSession[]>> {
    // Redirect to V2 API and convert response format
    const result = await this.listSessionsFull({
      status: filter?.status as SessionFilter["status"],
      limit: filter?.limit,
      offset: filter?.offset,
    });

    if (!result.success || !result.data) {
      return { success: false, error: result.error || "Failed to fetch sessions" };
    }

    // Convert V2 format to legacy format for backwards compatibility
    const legacySessions: ExecutionSession[] = [];
    for (const session of result.data) {
      for (const exec of session.executions) {
        legacySessions.push({
          id: exec.id,
          conversation_id: session.id, // session_id becomes conversation_id
          agent_id: exec.agent_id,
          parent_session_id: exec.parent_execution_id,
          status: exec.status,
          created_at: exec.started_at || session.created_at,
          started_at: exec.started_at,
          completed_at: exec.completed_at,
          tokens_in: exec.tokens_in,
          tokens_out: exec.tokens_out,
          error: exec.error,
        });
      }
    }

    return { success: true, data: legacySessions };
  }

  /** @deprecated Use getSessionFull() instead */
  async getExecutionSession(sessionId: string): Promise<TransportResult<ExecutionSession>> {
    return this.get<ExecutionSession>(`/api/executions/v2/sessions/${encodeURIComponent(sessionId)}`);
  }

  /** @deprecated Use getDashboardStats() instead */
  async getExecutionStats(): Promise<TransportResult<ExecutionStats>> {
    return this.get<ExecutionStats>("/api/executions/stats/counts");
  }

  async pauseSession(sessionId: string): Promise<TransportResult<void>> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      return { success: false, error: "WebSocket not connected" };
    }

    const command = {
      type: "pause",
      session_id: sessionId,
    };

    try {
      this.ws.send(JSON.stringify(command));
      return { success: true };
    } catch (error) {
      return { success: false, error: String(error) };
    }
  }

  async resumeSession(sessionId: string): Promise<TransportResult<void>> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      return { success: false, error: "WebSocket not connected" };
    }

    const command = {
      type: "resume",
      session_id: sessionId,
    };

    try {
      this.ws.send(JSON.stringify(command));
      return { success: true };
    } catch (error) {
      return { success: false, error: String(error) };
    }
  }

  async cancelSession(sessionId: string): Promise<TransportResult<void>> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      return { success: false, error: "WebSocket not connected" };
    }

    const command = {
      type: "cancel",
      session_id: sessionId,
    };

    try {
      this.ws.send(JSON.stringify(command));
      return { success: true };
    } catch (error) {
      return { success: false, error: String(error) };
    }
  }

  async endSession(sessionId: string): Promise<TransportResult<void>> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      return { success: false, error: "WebSocket not connected" };
    }

    const command = {
      type: "end_session",
      session_id: sessionId,
    };

    console.log("[SESSION_DEBUG] Sending end_session command:", sessionId);

    try {
      this.ws.send(JSON.stringify(command));
      return { success: true };
    } catch (error) {
      return { success: false, error: String(error) };
    }
  }

  async cleanupExecutionSessions(olderThan?: string): Promise<TransportResult<{ deleted: number }>> {
    if (!this.config) {
      return { success: false, error: "Transport not initialized" };
    }

    // If no timestamp provided, use a future date to delete everything
    const timestamp = olderThan || new Date(Date.now() + 86400000).toISOString();

    try {
      const response = await fetch(
        `${this.config.httpUrl}/api/executions/cleanup?older_than=${encodeURIComponent(timestamp)}`,
        {
          method: "DELETE",
          headers: { "Content-Type": "application/json" },
        }
      );

      if (!response.ok) {
        return { success: false, error: `HTTP ${response.status}: ${response.statusText}` };
      }

      const data = await response.json();
      return { success: true, data };
    } catch (error) {
      return { success: false, error: String(error) };
    }
  }

  // =========================================================================
  // Agent Execution
  // =========================================================================

  async executeAgent(
    agentId: string,
    conversationId: string,
    message: string,
    sessionId?: string
  ): Promise<TransportResult<{ conversationId: string; sessionId?: string }>> {
    // Send execute command via WebSocket
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      return { success: false, error: "WebSocket not connected" };
    }

    const command: Record<string, unknown> = {
      type: "invoke",
      agent_id: agentId,
      conversation_id: conversationId,
      message,
    };

    // Include session_id to continue an existing session
    if (sessionId) {
      command.session_id = sessionId;
    }

    console.log("[SESSION_DEBUG] WebSocket sending invoke command:", {
      type: command.type,
      agent_id: command.agent_id,
      conversation_id: command.conversation_id,
      session_id: command.session_id || "(none - new session)",
    });

    try {
      this.ws.send(JSON.stringify(command));
      return { success: true, data: { conversationId, sessionId } };
    } catch (error) {
      return { success: false, error: String(error) };
    }
  }

  async stopAgent(conversationId: string): Promise<TransportResult<void>> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      return { success: false, error: "WebSocket not connected" };
    }

    const command = {
      type: "stop",
      conversation_id: conversationId,
    };

    try {
      this.ws.send(JSON.stringify(command));
      return { success: true };
    } catch (error) {
      return { success: false, error: String(error) };
    }
  }

  // =========================================================================
  // Event Streaming (Legacy)
  // =========================================================================

  /**
   * @deprecated Use subscribeConversation() instead for server-side routing
   */
  subscribe(conversationId: string, callback: EventCallback): UnsubscribeFn {
    if (!this.eventCallbacks.has(conversationId)) {
      this.eventCallbacks.set(conversationId, new Set());
    }
    this.eventCallbacks.get(conversationId)!.add(callback);

    return () => {
      const callbacks = this.eventCallbacks.get(conversationId);
      if (callbacks) {
        callbacks.delete(callback);
        if (callbacks.size === 0) {
          this.eventCallbacks.delete(conversationId);
        }
      }
    };
  }

  async connect(): Promise<TransportResult<void>> {
    if (!this.config) {
      return { success: false, error: "Transport not initialized" };
    }

    // Already connected
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      return { success: true };
    }

    // Close existing connection if in wrong state
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }

    this.setConnectionState({ status: "connecting" });

    return new Promise((resolve) => {
      try {
        this.ws = new WebSocket(this.config!.wsUrl);

        this.ws.onopen = () => {
          console.log("[HttpTransport] WebSocket connected");
          this.reconnectAttempts = 0;
          this.setConnectionState({ status: "connected" });
          this.startHeartbeat();
          this.setupBrowserEventHandlers();
          this.resubscribeAll();
          resolve({ success: true });
        };

        this.ws.onmessage = (event) => {
          try {
            const data = JSON.parse(event.data) as StreamEvent;
            this.handleWebSocketMessage(data);
          } catch (error) {
            console.error("[HttpTransport] Failed to parse WebSocket message:", error);
          }
        };

        this.ws.onclose = () => {
          console.log("[HttpTransport] WebSocket disconnected");
          this.stopHeartbeat();
          this.attemptReconnect();
        };

        this.ws.onerror = (error) => {
          console.error("[HttpTransport] WebSocket error:", error);
          this.setConnectionState({ status: "failed", error: "WebSocket connection failed" });
          resolve({ success: false, error: "WebSocket connection failed" });
        };
      } catch (error) {
        this.setConnectionState({ status: "failed", error: String(error) });
        resolve({ success: false, error: String(error) });
      }
    });
  }

  async disconnect(): Promise<void> {
    this.cleanupBrowserEventHandlers();
    this.stopHeartbeat();
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.eventCallbacks.clear();
    this.globalCallbacks.clear();
    this.conversationSubscriptions.clear();
    this.globalEventCallbacks.clear();
    this.setConnectionState({ status: "disconnected", reason: "user" });
  }

  isConnected(): boolean {
    return this.ws !== null && this.ws.readyState === WebSocket.OPEN;
  }

  // =========================================================================
  // Subscription API (server-side routing)
  // =========================================================================

  /**
   * Get the current connection state.
   */
  getConnectionState(): ConnectionState {
    return this.connectionState;
  }

  /**
   * Subscribe to connection state changes.
   */
  onConnectionStateChange(callback: ConnectionStateCallback): UnsubscribeFn {
    this.connectionStateCallbacks.add(callback);
    // Immediately notify of current state
    callback(this.connectionState);
    return () => this.connectionStateCallbacks.delete(callback);
  }

  /**
   * Subscribe to conversation events with server-side routing.
   * Only subscribed clients receive events for this conversation.
   */
  subscribeConversation(
    conversationId: string,
    options: SubscriptionOptions
  ): UnsubscribeFn {
    let state = this.conversationSubscriptions.get(conversationId);

    if (!state) {
      state = {
        callbacks: new Set(),
        errorCallbacks: new Map(),
        confirmedCallbacks: new Map(),
        confirmed: false,
        lastSeq: 0,
      };
      this.conversationSubscriptions.set(conversationId, state);
      // Send subscribe message to server
      this.sendSubscribe(conversationId);
    }

    // Wrap callback to include sequence tracking
    const wrappedCallback: ConversationCallback = (event) => {
      const currentState = this.conversationSubscriptions.get(conversationId);
      if (currentState && event.seq !== undefined) {
        // Check for sequence gap
        if (event.seq > currentState.lastSeq + 1 && currentState.lastSeq > 0) {
          console.warn(
            `[Transport] Sequence gap: expected ${currentState.lastSeq + 1}, got ${event.seq}. ` +
              `Recommend refreshing conversation state via API.`
          );
        }
        currentState.lastSeq = event.seq;
      }
      options.onEvent(event);
    };

    state.callbacks.add(wrappedCallback);

    // Track error and confirmed callbacks per wrapped callback
    if (options.onError) {
      state.errorCallbacks.set(wrappedCallback, options.onError);
    }
    if (options.onConfirmed) {
      state.confirmedCallbacks.set(wrappedCallback, options.onConfirmed);
    }

    return () => {
      const state = this.conversationSubscriptions.get(conversationId);
      if (!state) return;

      state.callbacks.delete(wrappedCallback);
      state.errorCallbacks.delete(wrappedCallback);
      state.confirmedCallbacks.delete(wrappedCallback);

      // If no more callbacks, unsubscribe from server
      if (state.callbacks.size === 0) {
        this.conversationSubscriptions.delete(conversationId);
        this.sendUnsubscribe(conversationId);
      }
    };
  }

  /**
   * Subscribe to global events (stats updates, notifications).
   */
  onGlobalEvent(callback: GlobalCallback): UnsubscribeFn {
    this.globalEventCallbacks.add(callback);
    return () => this.globalEventCallbacks.delete(callback);
  }

  /**
   * Manual reconnect - resets attempt counter and tries again.
   */
  async reconnect(): Promise<void> {
    this.reconnectAttempts = 0;
    if (this.ws) {
      this.ws.close();
    }
    await this.connect();
  }

  // ─────────────────────────────────────────────────────────────────────────
  // Connection State Management
  // ─────────────────────────────────────────────────────────────────────────

  private setConnectionState(state: ConnectionState): void {
    this.connectionState = state;
    // Use snapshot to avoid iterator invalidation if callback modifies set
    const callbacks = [...this.connectionStateCallbacks];
    for (const callback of callbacks) {
      try {
        callback(state);
      } catch (e) {
        console.error("[Transport] Connection state callback error:", e);
      }
    }
  }

  // ─────────────────────────────────────────────────────────────────────────
  // Heartbeat
  // ─────────────────────────────────────────────────────────────────────────

  private startHeartbeat(): void {
    this.stopHeartbeat();
    this.lastPong = Date.now();

    this.pingInterval = setInterval(() => {
      if (Date.now() - this.lastPong > this.PONG_TIMEOUT) {
        console.warn("[Transport] Ping timeout, reconnecting");
        this.ws?.close(4000, "Ping timeout");
        return;
      }

      if (this.ws?.readyState === WebSocket.OPEN) {
        this.ws.send(JSON.stringify({ type: "ping" }));
      }
    }, this.PING_INTERVAL);
  }

  private stopHeartbeat(): void {
    if (this.pingInterval) {
      clearInterval(this.pingInterval);
      this.pingInterval = null;
    }
  }

  // ─────────────────────────────────────────────────────────────────────────
  // Browser Event Handlers
  // ─────────────────────────────────────────────────────────────────────────

  private setupBrowserEventHandlers(): void {
    this.cleanupBrowserEventHandlers();

    // Handle tab visibility changes
    this.visibilityHandler = () => {
      if (document.visibilityState === "visible") {
        if (this.ws?.readyState !== WebSocket.OPEN) {
          this.reconnectAttempts = 0;
          this.connect();
        }
      }
    };
    document.addEventListener("visibilitychange", this.visibilityHandler);

    // Handle network online/offline
    this.onlineHandler = () => {
      if (this.connectionState.status !== "connected") {
        this.reconnectAttempts = 0;
        this.connect();
      }
    };
    window.addEventListener("online", this.onlineHandler);
  }

  private cleanupBrowserEventHandlers(): void {
    if (this.visibilityHandler) {
      document.removeEventListener("visibilitychange", this.visibilityHandler);
      this.visibilityHandler = null;
    }
    if (this.onlineHandler) {
      window.removeEventListener("online", this.onlineHandler);
      this.onlineHandler = null;
    }
  }

  // ─────────────────────────────────────────────────────────────────────────
  // Subscription Protocol
  // ─────────────────────────────────────────────────────────────────────────

  private sendSubscribe(conversationId: string): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      console.log(`[Transport] Sending subscribe for ${conversationId}`);
      this.ws.send(
        JSON.stringify({
          type: "subscribe",
          conversation_id: conversationId,
        })
      );
      this.pendingSubscriptions.delete(conversationId);
    } else {
      // Queue for when WebSocket connects
      console.log(`[Transport] Queueing subscribe for ${conversationId} (WS not ready)`);
      this.pendingSubscriptions.add(conversationId);
    }
  }

  private sendUnsubscribe(conversationId: string): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(
        JSON.stringify({
          type: "unsubscribe",
          conversation_id: conversationId,
        })
      );
    }
  }

  private resubscribeAll(): void {
    // Re-subscribe existing subscriptions
    for (const [conversationId, state] of this.conversationSubscriptions) {
      state.confirmed = false;
      // Don't reset lastSeq - we want to detect gaps after reconnect
      this.sendSubscribe(conversationId);
    }

    // Flush any pending subscriptions that were queued before WS connected
    for (const conversationId of this.pendingSubscriptions) {
      if (this.conversationSubscriptions.has(conversationId)) {
        this.sendSubscribe(conversationId);
      }
    }
    this.pendingSubscriptions.clear();
  }

  // =========================================================================
  // Private Helpers
  // =========================================================================

  /**
   * Main WebSocket message handler - routes through both subscription
   * and legacy event systems for backwards compatibility.
   */
  private handleWebSocketMessage(data: StreamEvent): void {
    // Try new subscription system first
    if (this.handleSubscriptionMessage(data)) return;
    if (this.handleGlobalMessage(data)) return;
    if (this.handleConversationMessage(data)) return;

    // Fall back to legacy event handling for backwards compatibility
    this.handleEvent(data);
  }

  private handleSubscriptionMessage(message: StreamEvent): boolean {
    switch (message.type) {
      case "subscribed": {
        const convId = message.conversation_id as string;
        const currentSeq = message.current_sequence as number;
        const state = this.conversationSubscriptions.get(convId);
        if (state) {
          state.confirmed = true;
          state.lastSeq = currentSeq;
          console.log(`[Transport] Subscribed to ${convId} at seq ${currentSeq}`);
          // Notify confirmed callbacks
          for (const confirmedCb of state.confirmedCallbacks.values()) {
            try {
              confirmedCb(currentSeq);
            } catch (e) {
              console.error(e);
            }
          }
        }
        return true;
      }

      case "unsubscribed": {
        console.log(`[Transport] Unsubscribed from ${message.conversation_id}`);
        return true;
      }

      case "subscription_error": {
        const errorMsg = message as unknown as SubscriptionErrorMessage;
        console.error(
          `[Transport] Subscription error: ${errorMsg.code} - ${errorMsg.message}`
        );
        const state = this.conversationSubscriptions.get(errorMsg.conversation_id);
        if (state) {
          // Notify all error callbacks
          for (const errorCb of state.errorCallbacks.values()) {
            try {
              errorCb(errorMsg);
            } catch (e) {
              console.error(e);
            }
          }
        }
        this.conversationSubscriptions.delete(errorMsg.conversation_id);
        return true;
      }

      case "pong": {
        this.lastPong = Date.now();
        return true;
      }

      default:
        return false;
    }
  }

  private handleGlobalMessage(message: StreamEvent): boolean {
    if (message.type === "stats_update" || message.type === "session_notification") {
      const callbacks = [...this.globalEventCallbacks];
      for (const callback of callbacks) {
        try {
          callback(message as GlobalEvent);
        } catch (e) {
          console.error(e);
        }
      }
      return true;
    }
    return false;
  }

  private handleConversationMessage(message: StreamEvent): boolean {
    const conversationId = (message.conversation_id ??
      message.parent_conversation_id) as string | undefined;

    // Debug: log all conversation-related messages
    if (conversationId) {
      console.log(`[Transport] Received event type=${message.type} for conv=${conversationId.slice(0, 20)}...`);
    }

    if (!conversationId) return false;

    const state = this.conversationSubscriptions.get(conversationId);
    if (state) {
      const callbacks = [...state.callbacks];
      console.log(`[Transport] Routing ${message.type} to ${callbacks.length} subscriber(s)`);
      for (const callback of callbacks) {
        try {
          callback(message as ConversationEvent);
        } catch (e) {
          console.error(e);
        }
      }
      return true;
    } else {
      console.log(`[Transport] No subscription for ${conversationId.slice(0, 20)}... (subscribed to: ${[...this.conversationSubscriptions.keys()].join(", ")})`);
    }
    return false;
  }

  private handleEvent(event: StreamEvent): void {
    // Log agent_started events for session debugging
    if (event.type === "agent_started") {
      console.log("[SESSION_DEBUG] Received agent_started event from server:", {
        type: event.type,
        session_id: event.session_id,
        conversation_id: event.conversation_id,
        agent_id: event.agent_id,
      });
    }

    // Extract conversation_id from event
    // For most events: conversation_id
    // For delegation events: parent_conversation_id (so parent UI gets notified)
    const conversationId = (event.conversation_id ?? event.parent_conversation_id) as string | undefined;

    // Notify conversation-specific callbacks
    if (conversationId && this.eventCallbacks.has(conversationId)) {
      for (const callback of this.eventCallbacks.get(conversationId)!) {
        callback(event);
      }
    }

    // Notify global callbacks
    for (const callback of this.globalCallbacks) {
      callback(event);
    }
  }

  private attemptReconnect(): void {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      console.log("[HttpTransport] Max reconnect attempts reached");
      this.setConnectionState({
        status: "failed",
        error: "Max reconnect attempts reached",
      });
      return;
    }

    this.reconnectAttempts++;
    const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);
    console.log(`[HttpTransport] Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);

    this.setConnectionState({
      status: "reconnecting",
      attempt: this.reconnectAttempts,
      maxAttempts: this.maxReconnectAttempts,
    });

    setTimeout(() => {
      this.connect();
    }, delay);
  }

  private async get<T>(path: string): Promise<TransportResult<T>> {
    if (!this.config) {
      return { success: false, error: "Transport not initialized" };
    }

    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 5000);

      const response = await fetch(`${this.config.httpUrl}${path}`, {
        method: "GET",
        headers: { "Content-Type": "application/json" },
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      if (!response.ok) {
        return { success: false, error: `HTTP ${response.status}: ${response.statusText}` };
      }

      const data = await response.json();
      return { success: true, data };
    } catch (error) {
      if (error instanceof Error && error.name === 'AbortError') {
        return { success: false, error: "Request timeout - is the daemon running?" };
      }
      return { success: false, error: String(error) };
    }
  }

  private async post<T>(path: string, body: unknown): Promise<TransportResult<T>> {
    if (!this.config) {
      return { success: false, error: "Transport not initialized" };
    }

    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 30000);

      const response = await fetch(`${this.config.httpUrl}${path}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      if (!response.ok) {
        return { success: false, error: `HTTP ${response.status}: ${response.statusText}` };
      }

      const data = await response.json();
      return { success: true, data };
    } catch (error) {
      if (error instanceof Error && error.name === 'AbortError') {
        return { success: false, error: "Request timeout" };
      }
      return { success: false, error: String(error) };
    }
  }

  private async put<T>(path: string, body: unknown): Promise<TransportResult<T>> {
    if (!this.config) {
      return { success: false, error: "Transport not initialized" };
    }

    try {
      const response = await fetch(`${this.config.httpUrl}${path}`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });

      if (!response.ok) {
        return { success: false, error: `HTTP ${response.status}: ${response.statusText}` };
      }

      const data = await response.json();
      return { success: true, data };
    } catch (error) {
      return { success: false, error: String(error) };
    }
  }

  private async delete(path: string): Promise<TransportResult<void>> {
    if (!this.config) {
      return { success: false, error: "Transport not initialized" };
    }

    try {
      const response = await fetch(`${this.config.httpUrl}${path}`, {
        method: "DELETE",
        headers: { "Content-Type": "application/json" },
      });

      if (!response.ok && response.status !== 204) {
        return { success: false, error: `HTTP ${response.status}: ${response.statusText}` };
      }

      return { success: true };
    } catch (error) {
      return { success: false, error: String(error) };
    }
  }
}
