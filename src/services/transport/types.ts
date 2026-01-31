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
// Transport Mode
// ============================================================================

export type TransportMode = "tauri" | "web";

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
  verified?: boolean;
  isDefault?: boolean;
  createdAt?: string;
}

export interface CreateProviderRequest {
  id?: string;
  name: string;
  description: string;
  apiKey: string;
  baseUrl: string;
  models: string[];
  embeddingModels?: string[];
}

export interface UpdateProviderRequest {
  name?: string;
  description?: string;
  apiKey?: string;
  baseUrl?: string;
  models?: string[];
  embeddingModels?: string[];
}

export interface ProviderTestResult {
  success: boolean;
  message: string;
  models?: string[];
}

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
  grep: boolean;
  glob: boolean;
  python: boolean;
  loadSkill: boolean;
  uiTools: boolean;
  knowledgeGraph: boolean;
  createAgent: boolean;
  introspection: boolean;
}

export interface ToolSettingsResponse {
  success: boolean;
  data?: ToolSettings;
  error?: string;
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
