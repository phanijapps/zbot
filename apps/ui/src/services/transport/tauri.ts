// ============================================================================
// TAURI TRANSPORT (STUB)
// Tauri has been removed - this is a stub for type compatibility.
// Use HttpTransport instead.
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

const NOT_SUPPORTED = "Tauri transport is not available. Use web mode instead.";

/**
 * Stub TauriTransport - Tauri has been removed from this project.
 * This exists only for type compatibility. Use HttpTransport instead.
 */
export class TauriTransport implements Transport {
  readonly mode = "tauri" as const;

  async initialize(_config: TransportConfig): Promise<void> {
    console.warn(NOT_SUPPORTED);
  }

  async health(): Promise<TransportResult<HealthResponse>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async status(): Promise<TransportResult<StatusResponse>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async listAgents(): Promise<TransportResult<AgentResponse[]>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async getAgent(_id: string): Promise<TransportResult<AgentResponse>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async createAgent(_request: CreateAgentRequest): Promise<TransportResult<AgentResponse>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async updateAgent(_id: string, _request: UpdateAgentRequest): Promise<TransportResult<AgentResponse>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async deleteAgent(_id: string): Promise<TransportResult<void>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async listSkills(): Promise<TransportResult<SkillResponse[]>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async getSkill(_id: string): Promise<TransportResult<SkillResponse>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async createSkill(_request: CreateSkillRequest): Promise<TransportResult<SkillResponse>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async updateSkill(_id: string, _request: UpdateSkillRequest): Promise<TransportResult<SkillResponse>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async deleteSkill(_id: string): Promise<TransportResult<void>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async listProviders(): Promise<TransportResult<ProviderResponse[]>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async getProvider(_id: string): Promise<TransportResult<ProviderResponse>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async createProvider(_request: CreateProviderRequest): Promise<TransportResult<ProviderResponse>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async updateProvider(_id: string, _request: UpdateProviderRequest): Promise<TransportResult<ProviderResponse>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async deleteProvider(_id: string): Promise<TransportResult<void>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async testProvider(_provider: CreateProviderRequest): Promise<TransportResult<ProviderTestResult>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async setDefaultProvider(_id: string): Promise<TransportResult<ProviderResponse>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async listMcps(): Promise<TransportResult<McpListResponse>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async getMcp(_id: string): Promise<TransportResult<McpServerConfig>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async createMcp(_request: CreateMcpRequest): Promise<TransportResult<McpServerConfig>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async updateMcp(_id: string, _request: CreateMcpRequest): Promise<TransportResult<McpServerConfig>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async deleteMcp(_id: string): Promise<TransportResult<void>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async testMcp(_id: string): Promise<TransportResult<McpTestResult>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async getMessages(_id: string): Promise<TransportResult<MessageResponse[]>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async getSessionMessages(
    _sessionId: string,
    _query?: SessionMessagesQuery
  ): Promise<TransportResult<SessionMessage[]>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async getToolSettings(): Promise<TransportResult<ToolSettings>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async updateToolSettings(_settings: ToolSettings): Promise<TransportResult<ToolSettings>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async listLogSessions(_filter?: LogFilter): Promise<TransportResult<LogSession[]>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async getLogSession(_sessionId: string): Promise<TransportResult<SessionDetail>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async deleteLogSession(_sessionId: string): Promise<TransportResult<void>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async cleanupOldLogs(_olderThanDays: number): Promise<TransportResult<{ deletedCount: number }>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  // V2 API methods
  async listSessionsFull(_filter?: SessionFilter): Promise<TransportResult<SessionWithExecutions[]>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async getSessionFull(_sessionId: string): Promise<TransportResult<SessionWithExecutions>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async getDashboardStats(): Promise<TransportResult<DashboardStats>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  // Legacy methods (deprecated)
  async listExecutionSessions(_filter?: ExecutionSessionFilter): Promise<TransportResult<ExecutionSession[]>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async getExecutionSession(_sessionId: string): Promise<TransportResult<ExecutionSession>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async getExecutionStats(): Promise<TransportResult<ExecutionStats>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async pauseSession(_sessionId: string): Promise<TransportResult<void>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async resumeSession(_sessionId: string): Promise<TransportResult<void>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async cancelSession(_sessionId: string): Promise<TransportResult<void>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async cleanupExecutionSessions(_olderThan?: string): Promise<TransportResult<{ deleted: number }>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async executeAgent(
    _agentId: string,
    _conversationId: string,
    _message: string,
    _sessionId?: string
  ): Promise<TransportResult<{ conversationId: string; sessionId?: string }>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async stopAgent(_conversationId: string): Promise<TransportResult<void>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  subscribe(_conversationId: string, _callback: EventCallback): UnsubscribeFn {
    return () => {};
  }

  async connect(): Promise<TransportResult<void>> {
    return { success: false, error: NOT_SUPPORTED };
  }

  async disconnect(): Promise<void> {}

  isConnected(): boolean {
    return false;
  }
}
