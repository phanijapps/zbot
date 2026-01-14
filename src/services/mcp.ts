// ============================================================================
// MCP SERVERS SERVICE
// Frontend service for MCP server management
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import type { MCPServer } from "@/features/mcp/types";

/**
 * List all MCP servers
 */
export async function listMCPServers(): Promise<MCPServer[]> {
  return invoke("list_mcp_servers");
}

/**
 * Get a single MCP server by ID
 */
export async function getMCPServer(id: string): Promise<MCPServer> {
  return invoke("get_mcp_server", { id });
}

/**
 * Create a new MCP server
 */
export async function createMCPServer(server: Omit<MCPServer, "id" | "createdAt">): Promise<MCPServer> {
  const serverWithId: MCPServer = {
    ...server,
    id: server.name.toLowerCase().replace(/\s+/g, "-"),
    createdAt: new Date().toISOString(),
  };
  return invoke("create_mcp_server", { server: serverWithId });
}

/**
 * Update an existing MCP server
 */
export async function updateMCPServer(id: string, server: Omit<MCPServer, "id" | "createdAt">): Promise<MCPServer> {
  const serverWithId: MCPServer = {
    ...server,
    id: server.name.toLowerCase().replace(/\s+/g, "-"),
    createdAt: new Date().toISOString(),
  };
  return invoke("update_mcp_server", { id, server: serverWithId });
}

/**
 * Delete an MCP server
 */
export async function deleteMCPServer(id: string): Promise<void> {
  return invoke("delete_mcp_server", { id });
}

/**
 * Start an MCP server
 */
export async function startMCPServer(id: string): Promise<void> {
  return invoke("start_mcp_server", { id });
}

/**
 * Stop an MCP server
 */
export async function stopMCPServer(id: string): Promise<void> {
  return invoke("stop_mcp_server", { id });
}

/**
 * Test an MCP server configuration
 */
export async function testMCPServer(server: Omit<MCPServer, "id" | "createdAt">): Promise<{ success: boolean; message: string; tools?: string[] }> {
  return invoke("test_mcp_server", { server });
}
