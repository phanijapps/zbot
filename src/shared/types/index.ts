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
  instructions: string;
  mcps: string[];
  skills: string[];
  createdAt: string;
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
