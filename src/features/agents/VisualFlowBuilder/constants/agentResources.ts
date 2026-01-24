// ============================================================================
// VISUAL FLOW BUILDER - AGENT RESOURCES
// Constants for available tools, MCPs, and skills
// ============================================================================

// -----------------------------------------------------------------------------
// Built-in Tools
// -----------------------------------------------------------------------------

export const BUILTIN_TOOLS = [
  { id: "web_search", name: "Web Search", description: "Search the web for information", category: "search" },
  { id: "web_fetch", name: "Web Fetch", description: "Fetch content from a URL", category: "web" },
  { id: "file_read", name: "File Read", description: "Read file contents", category: "file" },
  { id: "file_write", name: "File Write", description: "Write content to a file", category: "file" },
  { id: "file_list", name: "File List", description: "List files in a directory", category: "file" },
  { id: "code_interpreter", name: "Code Interpreter", description: "Execute code in a sandbox", category: "code" },
  { id: "shell_execute", name: "Shell Execute", description: "Execute shell commands", category: "system" },
] as const;

export type BuiltInToolId = typeof BUILTIN_TOOLS[number]["id"];

// -----------------------------------------------------------------------------
// Tool Categories
// -----------------------------------------------------------------------------

export const TOOL_CATEGORIES = {
  search: { label: "Search", icon: "🔍", color: "text-blue-400" },
  web: { label: "Web", icon: "🌐", color: "text-green-400" },
  file: { label: "File", icon: "📁", color: "text-yellow-400" },
  code: { label: "Code", icon: "💻", color: "text-purple-400" },
  system: { label: "System", icon: "⚙️", color: "text-gray-400" },
  database: { label: "Database", icon: "🗄️", color: "text-orange-400" },
} as const;

// -----------------------------------------------------------------------------
// MCP Server Templates (common MCP servers)
// -----------------------------------------------------------------------------

export const MCP_TEMPLATES = [
  { id: "filesystem", name: "Filesystem", description: "Access local filesystem", command: "npx -y @modelcontextprotocol/server-filesystem", args: "/path/to/allowed" },
  { id: "github", name: "GitHub", description: "Interact with GitHub repositories", command: "npx -y @modelcontextprotocol/server-github", args: "" },
  { id: "postgres", name: "PostgreSQL", description: "Query PostgreSQL databases", command: "npx -y @modelcontextprotocol/server-postgres", args: "postgresql://..." },
  { id: "sqlite", name: "SQLite", description: "Query SQLite databases", command: "npx -y @modelcontextprotocol/server-sqlite", args: "/path/to/db.sqlite" },
  { id: "brave-search", name: "Brave Search", description: "Web search via Brave API", command: "npx -y @modelcontextprotocol/server-brave-search", args: "" },
  { id: "memory", name: "Memory", description: "Persistent memory storage", command: "npx -y @modelcontextprotocol/server-memory", args: "" },
  { id: "time", name: "Time", description: "Get current time and date", command: "npx -y @modelcontextprotocol/server-time", args: "" },
] as const;

export type MCPTemplateId = typeof MCP_TEMPLATES[number]["id"];

// -----------------------------------------------------------------------------
// Built-in Skills
// -----------------------------------------------------------------------------

export const BUILTIN_SKILLS = [
  { id: "entity-extract", name: "Entity Extract", description: "Extract named entities from text", category: "nlp" },
  { id: "sentiment", name: "Sentiment Analysis", description: "Analyze sentiment of text", category: "nlp" },
  { id: "summarizer", name: "Summarizer", description: "Summarize long content", category: "text" },
  { id: "translator", name: "Translator", description: "Translate between languages", category: "text" },
  { id: "code-explainer", name: "Code Explainer", description: "Explain code functionality", category: "code" },
  { id: "web-scraper", name: "Web Scraper", description: "Scrape web content", category: "web" },
  { id: "data-analyzer", name: "Data Analyzer", description: "Analyze and visualize data", category: "data" },
  { id: "yaml-validator", name: "YAML Validator", description: "Validate YAML syntax", category: "validation" },
] as const;

export type BuiltInSkillId = typeof BUILTIN_SKILLS[number]["id"];

// -----------------------------------------------------------------------------
// Skill Categories
// -----------------------------------------------------------------------------

export const SKILL_CATEGORIES = {
  nlp: { label: "NLP", icon: "🧠", color: "text-blue-400" },
  text: { label: "Text", icon: "📝", color: "text-green-400" },
  code: { label: "Code", icon: "💻", color: "text-purple-400" },
  web: { label: "Web", icon: "🌐", color: "text-cyan-400" },
  data: { label: "Data", icon: "📊", color: "text-orange-400" },
  validation: { label: "Validation", icon: "✓", color: "text-teal-400" },
} as const;

// -----------------------------------------------------------------------------
// Middleware Types
// -----------------------------------------------------------------------------

export const MIDDLEWARE_TYPES = [
  { id: "retry", name: "Retry", description: "Retry failed requests with exponential backoff" },
  { id: "cache", name: "Cache", description: "Cache responses to reduce API calls" },
  { id: "rate_limit", name: "Rate Limit", description: "Limit request rate to prevent throttling" },
  { id: "timeout", name: "Timeout", description: "Set timeout for requests" },
  { id: "logging", name: "Logging", description: "Log all requests and responses" },
  { id: "validation", name: "Validation", description: "Validate inputs and outputs" },
] as const;

export type MiddlewareTypeId = typeof MIDDLEWARE_TYPES[number]["id"];

// -----------------------------------------------------------------------------
// Helper functions
// -----------------------------------------------------------------------------

/**
 * Get tool by ID
 */
export function getToolById(id: string) {
  return BUILTIN_TOOLS.find((tool) => tool.id === id);
}

/**
 * Get MCP template by ID
 */
export function getMCPById(id: string) {
  return MCP_TEMPLATES.find((mcp) => mcp.id === id);
}

/**
 * Get skill by ID
 */
export function getSkillById(id: string) {
  return BUILTIN_SKILLS.find((skill) => skill.id === id);
}

/**
 * Get tools by category
 */
export function getToolsByCategory(category: string) {
  return BUILTIN_TOOLS.filter((tool) => tool.category === category);
}

/**
 * Get skills by category
 */
export function getSkillsByCategory(category: string) {
  return BUILTIN_SKILLS.filter((skill) => skill.category === category);
}
