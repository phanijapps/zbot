// ============================================================================
// SHARED CONSTANTS
// Application-wide constants
// ============================================================================

export const APP_NAME = "Agent Zero";
export const APP_VERSION = "0.1.0";

// Storage keys for Tauri API
export const STORAGE_KEYS = {
  CONVERSATIONS: "conversations",
  AGENTS: "agents",
  PROVIDERS: "providers",
  MCP_SERVERS: "mcp_servers",
  SKILLS: "skills",
  SETTINGS: "settings",
} as const;

// Tauri command names (Rust backend)
export const TAURI_COMMANDS = {
  // Core
  GREET: "greet",

  // Storage
  GET_STORED_VALUE: "get_stored_value",
  SET_STORED_VALUE: "set_stored_value",
  DELETE_STORED_VALUE: "delete_stored_value",

  // Conversations
  LIST_CONVERSATIONS: "list_conversations",
  GET_CONVERSATION: "get_conversation",
  CREATE_CONVERSATION: "create_conversation",
  UPDATE_CONVERSATION: "update_conversation",
  DELETE_CONVERSATION: "delete_conversation",

  // Agents
  LIST_AGENTS: "list_agents",
  GET_AGENT: "get_agent",
  CREATE_AGENT: "create_agent",
  UPDATE_AGENT: "update_agent",
  DELETE_AGENT: "delete_agent",

  // Providers
  LIST_PROVIDERS: "list_providers",
  GET_PROVIDER: "get_provider",
  CREATE_PROVIDER: "create_provider",
  UPDATE_PROVIDER: "update_provider",
  DELETE_PROVIDER: "delete_provider",

  // MCP
  LIST_MCP_SERVERS: "list_mcp_servers",
  GET_MCP_SERVER: "get_mcp_server",
  CREATE_MCP_SERVER: "create_mcp_server",
  UPDATE_MCP_SERVER: "update_mcp_server",
  DELETE_MCP_SERVER: "delete_mcp_server",
  START_MCP_SERVER: "start_mcp_server",
  STOP_MCP_SERVER: "stop_mcp_server",

  // Skills
  LIST_SKILLS: "list_skills",
  GET_SKILL: "get_skill",
  CREATE_SKILL: "create_skill",
  UPDATE_SKILL: "update_skill",
  DELETE_SKILL: "delete_skill",
} as const;
