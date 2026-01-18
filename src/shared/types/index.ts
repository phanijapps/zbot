// ============================================================================
// SHARED TYPES
// Central type definitions used across the application
// ============================================================================

// ============================================================================
// DOMAIN: Core
// ============================================================================

/** Application-wide configuration */
export interface AppConfig {
  appName: string;
  version: string;
  theme: "light" | "dark" | "system";
}

/** Navigation route definition */
export interface Route {
  path: string;
  label: string;
  icon?: string;
  description?: string;
}

// ============================================================================
// DOMAIN: Conversations
// ============================================================================

/** Chat message */
export interface Message {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  timestamp: number;
  metadata?: Record<string, unknown>;
}

/** Conversation thread */
export interface Conversation {
  id: string;
  title: string;
  agentId: string;
  messages: Message[];
  createdAt: number;
  updatedAt: number;
}

// ============================================================================
// DOMAIN: Agents
// ============================================================================

/** Agent type from zero-app framework */
export type AgentType =
  | "llm"
  | "sequential"
  | "parallel"
  | "loop"
  | "conditional"
  | "llm_conditional"
  | "custom";

/** Agent configuration */
export interface Agent {
  id: string;
  name: string;
  displayName: string;
  description: string;
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
  thinkingEnabled?: boolean;
  instructions: string;
  mcps: string[];
  skills: string[];
  /** Middleware configuration (YAML string) */
  middleware?: string;
  /** Zero-app framework agent type */
  agentType?: AgentType;
  /** System instruction for LLM agents (from config.yaml) */
  systemInstruction?: string;
  /** Sub-agents for workflow agents */
  subAgents?: Agent[];
  createdAt: string;
}

/** Sequential agent configuration */
export interface SequentialAgentConfig {
  description?: string;
}

/** Parallel agent configuration */
export interface ParallelAgentConfig {
  description?: string;
}

/** Loop agent configuration */
export interface LoopAgentConfig {
  maxIterations?: number;
  untilEscalation?: boolean;
  description?: string;
}

/** Conditional agent configuration (rule-based) */
export interface ConditionalAgentConfig {
  condition: string; // e.g., "state.is_premium" or "state.user_count > 10"
  ifAgent: string; // agent name
  elseAgent?: string; // agent name
  description?: string;
}

/** LLM conditional agent configuration (LLM-based routing) */
export interface LlmConditionalAgentConfig {
  instruction: string; // Classification instruction
  routes: Record<string, string>; // label -> agent name mapping
  defaultRoute?: string; // Fallback agent name
  description?: string;
}

/** Custom agent configuration */
export interface CustomAgentConfig {
  description?: string;
}

/** Middleware configuration */
export interface MiddlewareConfig {
  summarization?: SummarizationConfig;
  contextEditing?: ContextEditingConfig;
}

export interface SummarizationConfig {
  enabled: boolean;
  maxTokens: number;
  triggerAt: number; // Token threshold
  provider?: string; // Provider for summarization
}

export interface ContextEditingConfig {
  enabled: boolean;
  keepLastN: number;
  keepPolicy: "all" | "user_only" | "assistant_only";
}

// ============================================================================
// DOMAIN: Providers
// ============================================================================

/** API Provider credentials */
export interface Provider {
  id: string;
  name: string;
  description: string;
  apiKey: string;
  baseUrl: string;
  models: string[];
  verified?: boolean;
  createdAt: string;
}

/** Provider test result */
export interface ProviderTestResult {
  success: boolean;
  message: string;
  models?: string[];
}

// ============================================================================
// DOMAIN: MCP Servers
// ============================================================================

/** MCP Server connection */
export interface MCPServer {
  id: string;
  name: string;
  command: string;
  args: string[];
  env?: Record<string, string>;
  enabled: boolean;
}

// ============================================================================
// DOMAIN: Skills
// ============================================================================

/** Skill/Plugin configuration */
export interface Skill {
  id: string;
  name: string;
  displayName: string;
  description: string;
  category: string;
  instructions: string;
  createdAt: string;
}

// ============================================================================
// DOMAIN: Settings
// ============================================================================

/** Application settings */
export interface AppSettings {
  theme: "light" | "dark" | "system";
  fontSize: "small" | "medium" | "large";
  autoSave: boolean;
  defaultProvider?: string;
  defaultAgent?: string;
}
