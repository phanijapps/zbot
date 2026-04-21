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
  ChatSessionInit,
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
  SetupStatus,
  // Embedding backend types
  EmbeddingsBackend,
  EmbeddingsStatus,
  EmbeddingsHealth,
  CuratedModel,
  EmbeddingConfig,
  ConfigureProgressEvent,
} from "./types";

export { getProviderDefaultModel } from "./types";

// ============================================================================
// Default Configuration
// ============================================================================

const GATEWAY_HTTP_PORT = 18791;
const GATEWAY_WS_PORT = 18790;

/**
 * Build default gateway URLs from the current page's hostname. Serving
 * the UI on a LAN IP (e.g. http://192.168.1.5:3000 from a phone) needs
 * the gateway at http://192.168.1.5:18791 — hard-coding "localhost"
 * leaves the phone talking to itself. Falls back to localhost in
 * non-browser contexts (SSR/tests).
 */
function defaultConfig(): TransportConfig {
  if (typeof window === "undefined" || !window.location) {
    return {
      httpUrl: `http://localhost:${GATEWAY_HTTP_PORT}`,
      wsUrl: `ws://localhost:${GATEWAY_WS_PORT}`,
    };
  }
  const host = window.location.hostname || "localhost";
  const wsProto = window.location.protocol === "https:" ? "wss" : "ws";
  const httpProto = window.location.protocol === "https:" ? "https" : "http";
  return {
    httpUrl: `${httpProto}://${host}:${GATEWAY_HTTP_PORT}`,
    wsUrl: `${wsProto}://${host}:${GATEWAY_WS_PORT}`,
  };
}

/**
 * Get configuration from environment or use defaults.
 */
function getConfig(): TransportConfig {
  const fallback = defaultConfig();
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
        httpUrl: httpUrl || fallback.httpUrl,
        wsUrl: wsUrl || fallback.wsUrl,
      };
    }
  }

  return fallback;
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
    ...getConfig(),
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
