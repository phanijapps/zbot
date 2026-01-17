// ============================================================================
// MCP SERVERS FEATURE - Types
// TypeScript types for Model Context Protocol servers
// Supports both stdio (command-based) and HTTP-based MCP servers
// ============================================================================

/** MCP Server type */
export type MCPServerType = 'stdio' | 'http' | 'sse' | 'streamable-http';

/** Stdio configuration (command-based MCP server) */
export interface StdioConfig {
  command: string;
  args: string[];
  env?: Record<string, string>;
}

/** HTTP configuration (HTTP-based MCP server) */
export interface HttpConfig {
  url: string;
  headers?: Record<string, string>;
}

/** SSE configuration (Server-Sent Events MCP server) */
export interface SseConfig {
  url: string;
  headers?: Record<string, string>;
}

/** MCP Server configuration */
export interface MCPServer {
  /** Unique identifier for the server */
  id: string;
  /** Human-readable name */
  name: string;
  /** Description of what the server does */
  description: string;
  /** Server type (stdio, http, sse, or streamable-http) */
  type: MCPServerType;
  /** Stdio configuration (for type='stdio') */
  command?: string;
  /** Arguments for stdio command */
  args?: string[];
  /** Environment variables for stdio */
  env?: Record<string, string>;
  /** HTTP URL (for type='http', 'sse', or 'streamable-http') */
  url?: string;
  /** HTTP headers (for type='http', 'sse', or 'streamable-http') */
  headers?: Record<string, string>;
  /** Whether the server is enabled */
  enabled: boolean;
  /** Current status of the server */
  status: MCPServerStatus;
  /** Whether the server has been tested/validated */
  validated?: boolean;
  /** When the server was created */
  createdAt: string;
}

/** MCP Server status */
export type MCPServerStatus = 'running' | 'stopped' | 'error';

/** Raw MCP configuration from backend */
export type MCPServerConfig = StdioMcpConfig | HttpMcpConfig | SseMcpConfig | StreamableHttpMcpConfig;

/** Stdio MCP config (backend format) */
export interface StdioMcpConfig {
  type: 'stdio';
  id?: string;
  name: string;
  description: string;
  command: string;
  args: string[];
  env?: Record<string, string>;
  enabled?: boolean;
}

/** HTTP MCP config (backend format) */
export interface HttpMcpConfig {
  type: 'http';
  id?: string;
  name: string;
  description: string;
  url: string;
  headers?: Record<string, string>;
  enabled?: boolean;
}

/** SSE MCP config (backend format) */
export interface SseMcpConfig {
  type: 'sse';
  id?: string;
  name: string;
  description: string;
  url: string;
  headers?: Record<string, string>;
  enabled?: boolean;
}

/** StreamableHttp MCP config (backend format) */
export interface StreamableHttpMcpConfig {
  type: 'streamable-http';
  id?: string;
  name: string;
  description: string;
  url: string;
  headers?: Record<string, string>;
  enabled?: boolean;
}

/** Preset MCP server template */
export interface MCPServerPreset {
  name: string;
  description: string;
  command: string;
  args: string;
  env?: string;
}

/** Preset MCP servers */
export const MCP_SERVER_PRESETS: MCPServerPreset[] = [
  {
    name: 'Filesystem Server',
    description: 'Access and manipulate files in specified directories',
    command: 'npx',
    args: '-y @modelcontextprotocol/server-filesystem /path/to/allowed/directory',
  },
  {
    name: 'GitHub Server',
    description: 'Interact with GitHub repositories, issues, and pull requests',
    command: 'npx',
    args: '-y @modelcontextprotocol/server-github',
    env: 'GITHUB_PERSONAL_ACCESS_TOKEN=your_token_here',
  },
  {
    name: 'PostgreSQL Server',
    description: 'Query PostgreSQL databases',
    command: 'npx',
    args: '-y @modelcontextprotocol/server-postgres',
    env: 'POSTGRES_CONNECTION_STRING=postgresql://user:pass@localhost/db',
  },
  {
    name: 'Brave Search',
    description: 'Web search capabilities using Brave Search API',
    command: 'npx',
    args: '-y @modelcontextprotocol/server-brave-search',
    env: 'BRAVE_API_KEY=your_api_key',
  },
  {
    name: 'SQLite Server',
    description: 'Query SQLite databases',
    command: 'npx',
    args: '-y @modelcontextprotocol/server-sqlite',
    env: 'SQLITE_DB_PATH=/path/to/database.db',
  },
  {
    name: 'Puppeteer Server',
    description: 'Web automation and scraping with Puppeteer',
    command: 'npx',
    args: '-y @modelcontextprotocol/server-puppeteer',
  },
];

/** HTTP Preset MCP servers */
export const MCP_HTTP_PRESETS: MCPServerPreset[] = [
  {
    name: 'Web Search (Z.ai)',
    description: 'Web search via Z.ai HTTP MCP',
    command: '',
    args: 'https://api.z.ai/api/mcp/web_search_prime/mcp',
  },
];

/** SSE Preset MCP servers */
export const MCP_SSE_PRESETS: MCPServerPreset[] = [
  {
    name: 'Web Search (Z.ai SSE)',
    description: 'Web search via Z.ai SSE MCP',
    command: '',
    args: 'https://api.z.ai/api/mcp/web_search_prime/sse',
  },
];

/** StreamableHttp Preset MCP servers */
export const MCP_STREAMABLE_HTTP_PRESETS: MCPServerPreset[] = [
  {
    name: 'Web Search (Z.ai)',
    description: 'Web search via Z.ai streamable-http MCP',
    command: '',
    args: 'https://api.z.ai/api/mcp/web_search_prime/mcp',
  },
];

/** Get status badge color */
export function getStatusColor(status: MCPServerStatus): string {
  switch (status) {
    case 'running':
      return 'bg-green-500/20 text-green-400 border-green-500/30';
    case 'stopped':
      return 'bg-gray-500/20 text-gray-400 border-gray-500/30';
    case 'error':
      return 'bg-red-500/20 text-red-400 border-red-500/30';
    default:
      return 'bg-gray-500/20 text-gray-400 border-gray-500/30';
  }
}

/** Get status icon component */
export function getStatusIcon(status: MCPServerStatus): string {
  switch (status) {
    case 'running':
      return '✓';
    case 'stopped':
      return '○';
    case 'error':
      return '!';
    default:
      return '○';
  }
}
