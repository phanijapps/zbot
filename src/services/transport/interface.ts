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
  // Agent Execution
  // =========================================================================

  /** Execute an agent with a message */
  executeAgent(
    agentId: string,
    conversationId: string,
    message: string
  ): Promise<TransportResult<{ conversationId: string }>>;

  /** Stop an agent execution */
  stopAgent(conversationId: string): Promise<TransportResult<void>>;

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
