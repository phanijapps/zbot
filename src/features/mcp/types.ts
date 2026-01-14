// ============================================================================
// MCP SERVERS FEATURE - Types
// TypeScript types for Model Context Protocol servers
// ============================================================================

/** MCP Server configuration */
export interface MCPServer {
  /** Unique identifier for the server */
  id: string;
  /** Human-readable name */
  name: string;
  /** Description of what the server does */
  description: string;
  /** Command to run (e.g., "npx", "node", "python") */
  command: string;
  /** Arguments to pass to the command */
  args: string[];
  /** Optional environment variables */
  env?: Record<string, string>;
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
