// ============================================================================
// TRANSPORT LAYER
// Abstraction layer for Tauri IPC and HTTP/WebSocket communication
// ============================================================================

import type { Transport } from "./interface";
import type { TransportConfig, TransportMode } from "./types";
import { HttpTransport } from "./http";
import { TauriTransport } from "./tauri";

// Re-export types
export type { Transport } from "./interface";
export type {
  TransportConfig,
  TransportMode,
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
  McpServerSummary,
  McpListResponse,
  McpServerConfig,
  CreateMcpRequest,
  McpTestResult,
  MessageResponse,
  ConversationResponse,
  ToolSettings,
  LogLevel,
  LogCategory,
  SessionStatus,
  ExecutionLog,
  LogSession,
  SessionDetail,
  LogFilter,
} from "./types";

// ============================================================================
// Transport Detection
// ============================================================================

/**
 * Detect whether we're running in Tauri or as a web app.
 */
export function detectTransportMode(): TransportMode {
  // Check if Tauri APIs are available
  if (typeof window !== "undefined" && "__TAURI__" in window) {
    return "tauri";
  }
  return "web";
}

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
 * Create a transport instance based on the current environment.
 */
export function createTransport(mode?: TransportMode): Transport {
  const actualMode = mode || detectTransportMode();

  if (actualMode === "tauri") {
    return new TauriTransport();
  }

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
  const mode = detectTransportMode();
  globalTransport = createTransport(mode);

  const finalConfig = {
    ...DEFAULT_CONFIG,
    ...config,
  };

  await globalTransport.initialize(finalConfig);
  initialized = true;

  console.log(`[Transport] Initialized in ${mode} mode`);
  console.log(`[Transport] HTTP: ${finalConfig.httpUrl}`);
  console.log(`[Transport] WebSocket: ${finalConfig.wsUrl}`);

  return globalTransport;
}

/**
 * Get the current transport mode.
 */
export function getTransportMode(): TransportMode {
  return globalTransport?.mode || detectTransportMode();
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
