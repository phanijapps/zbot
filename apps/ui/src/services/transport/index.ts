// ============================================================================
// TRANSPORT LAYER
// HTTP/WebSocket communication with the gateway
// ============================================================================

import type { Transport } from "./interface";
import type { TransportConfig } from "./types";
import { HttpTransport } from "./http";

// Re-export types
export type { Transport } from "./interface";
export type {
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
  ModelRegistryResponse,
  ModelProfile,
  ModelCapabilities,
  HealthResponse,
  StatusResponse,
  EventCallback,
  UnsubscribeFn,
  StreamEvent,
  McpServerSummary,
  McpListResponse,
  McpServerConfig,
  CreateMcpRequest,
  McpTestResult,
  MessageResponse,
  SessionMessage,
  MessageScope,
  SessionMessagesQuery,
  ConversationResponse,
  ToolSettings,
  LogSettings,
  UpdateLogSettingsRequest,
  ExecutionSettings,
  LogLevel,
  LogCategory,
  SessionStatus,
  ExecutionLog,
  LogSession,
  SessionDetail,
  LogFilter,
  // Subscription types
  SubscriptionScope,
  SubscriptionOptions,
  // Plugin types
  PluginInfo,
  PluginsResponse,
  // Cron types
  CronJobResponse,
  CreateCronJobRequest,
  UpdateCronJobRequest,
  CronTriggerResult,
} from "./types";

export { getProviderDefaultModel } from "./types";

// ============================================================================
// Default Configuration
// ============================================================================

const DEFAULT_CONFIG: TransportConfig = {
  httpUrl: "http://localhost:18791",
  wsUrl: "ws://localhost:18790",
};

/**
 * Get configuration from environment or use defaults.
 */
function getConfig(): TransportConfig {
  // In web mode, check for environment variables or window config
  if (typeof window !== "undefined") {
    const windowConfig = (window as { __ZERO_CONFIG__?: TransportConfig }).__ZERO_CONFIG__;
    if (windowConfig) {
      return windowConfig;
    }
  }

  // Check for URL parameters (useful for development)
  if (typeof window !== "undefined" && window.location) {
    const params = new URLSearchParams(window.location.search);
    const httpUrl = params.get("gateway_http");
    const wsUrl = params.get("gateway_ws");

    if (httpUrl || wsUrl) {
      return {
        httpUrl: httpUrl || DEFAULT_CONFIG.httpUrl,
        wsUrl: wsUrl || DEFAULT_CONFIG.wsUrl,
      };
    }
  }

  return DEFAULT_CONFIG;
}

// ============================================================================
// Transport Factory
// ============================================================================

/**
 * Create a transport instance.
 */
export function createTransport(): Transport {
  return new HttpTransport();
}

// ============================================================================
// Global Transport Instance
// ============================================================================

let globalTransport: Transport | null = null;
let initialized = false;

/**
 * Get the global transport instance.
 * Creates and initializes it if not already done.
 */
export async function getTransport(): Promise<Transport> {
  if (!globalTransport) {
    globalTransport = createTransport();
  }

  if (!initialized) {
    await globalTransport.initialize(getConfig());
    initialized = true;
  }

  return globalTransport;
}

/**
 * Initialize the transport with custom configuration.
 * Should be called early in app startup.
 */
export async function initializeTransport(config?: Partial<TransportConfig>): Promise<Transport> {
  globalTransport = createTransport();

  const finalConfig = {
    ...DEFAULT_CONFIG,
    ...config,
  };

  await globalTransport.initialize(finalConfig);
  initialized = true;

  return globalTransport;
}

/**
 * Check if transport is initialized.
 */
export function isTransportInitialized(): boolean {
  return initialized;
}

/**
 * Reset the transport (for testing).
 */
export function resetTransport(): void {
  if (globalTransport) {
    globalTransport.disconnect();
  }
  globalTransport = null;
  initialized = false;
}
