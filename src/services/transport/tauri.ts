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

  async executeAgent(
    _agentId: string,
    _conversationId: string,
    _message: string
  ): Promise<TransportResult<{ conversationId: string }>> {
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
