// ============================================================================
// AGENTS SERVICE
// Frontend service for agent management
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import type { Agent } from "@/shared/types";

/**
 * List all agents
 */
export async function listAgents(): Promise<Agent[]> {
  return invoke("list_agents");
}

/**
 * Get a single agent by ID
 */
export async function getAgent(id: string): Promise<Agent> {
  return invoke("get_agent", { id });
}

/**
 * Create a new agent
 */
export async function createAgent(agent: Omit<Agent, "id" | "createdAt">): Promise<Agent> {
  const agentWithId: Agent = {
    ...agent,
    id: agent.name, // Use name as ID
    createdAt: new Date().toISOString(),
  };
  return invoke("create_agent", { agent: agentWithId });
}

/**
 * Update an existing agent
 */
export async function updateAgent(id: string, agent: Omit<Agent, "id" | "createdAt">): Promise<Agent> {
  const agentWithId: Agent = {
    ...agent,
    id: agent.name, // Update ID if name changed
    createdAt: new Date().toISOString(),
  };
  return invoke("update_agent", { id, agent: agentWithId });
}

/**
 * Delete an agent
 */
export async function deleteAgent(id: string): Promise<void> {
  return invoke("delete_agent", { id });
}

/**
 * Validate agent name (lowercase, numbers, hyphens only, doesn't start/end with hyphen)
 */
export function validateAgentName(name: string): { valid: boolean; error?: string } {
  // Check length
  if (name.length === 0) {
    return { valid: false, error: "Name is required" };
  }
  if (name.length > 64) {
    return { valid: false, error: "Name must be 64 characters or less" };
  }

  // Check for valid characters (lowercase letters, numbers, hyphens)
  const validNameRegex = /^[a-z0-9-]+$/;
  if (!validNameRegex.test(name)) {
    return { valid: false, error: "Name can only contain lowercase letters, numbers, and hyphens" };
  }

  // Check for consecutive hyphens
  if (name.includes("--")) {
    return { valid: false, error: "Name cannot contain consecutive hyphens" };
  }

  // Check for leading/trailing hyphens
  if (name.startsWith("-") || name.endsWith("-")) {
    return { valid: false, error: "Name cannot start or end with a hyphen" };
  }

  return { valid: true };
}

/**
 * Sanitize a name to be valid as an agent name
 */
export function sanitizeAgentName(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9-]/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
}

// ============================================================================
// Agent File Operations
// ============================================================================

/** Agent file entry */
export interface AgentFile {
  name: string;
  path: string;
  isFile: boolean;
  isBinary: boolean;
  isProtected: boolean;
  size: number;
}

/** Agent file content */
export interface AgentFileContent {
  content: string;
  isBinary: boolean;
  isMarkdown: boolean;
}

/**
 * List files in an agent folder
 */
export async function listAgentFiles(agentId: string): Promise<AgentFile[]> {
  return invoke("list_agent_files", { agentId });
}

/**
 * Read a file's content from an agent folder
 */
export async function readAgentFile(agentId: string, filePath: string): Promise<AgentFileContent> {
  return invoke("read_agent_file", { agentId, filePath });
}

/**
 * Write or create a file in an agent folder
 */
export async function writeAgentFile(agentId: string, filePath: string, content: string): Promise<void> {
  return invoke("write_agent_file", { agentId, filePath, content });
}

/**
 * Create a folder in an agent directory
 */
export async function createAgentFolder(agentId: string, folderPath: string): Promise<void> {
  return invoke("create_agent_folder", { agentId, folderPath });
}

/**
 * Delete a file or folder from an agent directory
 */
export async function deleteAgentFile(agentId: string, filePath: string): Promise<void> {
  return invoke("delete_agent_file", { agentId, filePath });
}

/**
 * Upload/copy a file to an agent folder
 */
export async function uploadAgentFile(agentId: string, sourcePath: string, destPath: string): Promise<void> {
  return invoke("upload_agent_file", { agentId, sourcePath, destPath });
}
