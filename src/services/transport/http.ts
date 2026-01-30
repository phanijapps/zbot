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
} from "./types";

// ============================================================================
// HTTP Transport Implementation
// ============================================================================

export class HttpTransport implements Transport {
  readonly mode = "web" as const;

  private config: TransportConfig | null = null;
  private ws: WebSocket | null = null;
  private eventCallbacks: Map<string, Set<EventCallback>> = new Map();
  private globalCallbacks: Set<EventCallback> = new Set();
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectDelay = 1000;

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
  // Agent Execution
  // =========================================================================

  async executeAgent(
    agentId: string,
    conversationId: string,
    message: string
  ): Promise<TransportResult<{ conversationId: string }>> {
    // Send execute command via WebSocket
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      return { success: false, error: "WebSocket not connected" };
    }

    const command = {
      type: "invoke",
      agent_id: agentId,
      conversation_id: conversationId,
      message,
    };

    try {
      this.ws.send(JSON.stringify(command));
      return { success: true, data: { conversationId } };
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
  // Event Streaming
  // =========================================================================

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

    return new Promise((resolve) => {
      try {
        this.ws = new WebSocket(this.config!.wsUrl);

        this.ws.onopen = () => {
          console.log("[HttpTransport] WebSocket connected");
          this.reconnectAttempts = 0;
          resolve({ success: true });
        };

        this.ws.onmessage = (event) => {
          try {
            const data = JSON.parse(event.data) as StreamEvent;
            this.handleEvent(data);
          } catch (error) {
            console.error("[HttpTransport] Failed to parse WebSocket message:", error);
          }
        };

        this.ws.onclose = () => {
          console.log("[HttpTransport] WebSocket disconnected");
          this.attemptReconnect();
        };

        this.ws.onerror = (error) => {
          console.error("[HttpTransport] WebSocket error:", error);
          resolve({ success: false, error: "WebSocket connection failed" });
        };
      } catch (error) {
        resolve({ success: false, error: String(error) });
      }
    });
  }

  async disconnect(): Promise<void> {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.eventCallbacks.clear();
    this.globalCallbacks.clear();
  }

  isConnected(): boolean {
    return this.ws !== null && this.ws.readyState === WebSocket.OPEN;
  }

  // =========================================================================
  // Private Helpers
  // =========================================================================

  private handleEvent(event: StreamEvent): void {
    // Extract conversation_id from event if present
    const conversationId = event.conversation_id as string | undefined;

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
      return;
    }

    this.reconnectAttempts++;
    const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);
    console.log(`[HttpTransport] Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);

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
