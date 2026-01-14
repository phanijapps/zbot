// ============================================================================
// ROUTE DEFINITIONS
// Application navigation routes
// ============================================================================

import type { Route } from "../types";

export const ROUTES: Route[] = [
  {
    path: "/",
    label: "Conversations",
    icon: "MessageSquare",
    description: "Chat with your agents",
  },
  {
    path: "/agents",
    label: "Agents",
    icon: "Bot",
    description: "Manage your AI agents",
  },
  {
    path: "/providers",
    label: "Providers",
    icon: "Server",
    description: "Configure API providers",
  },
  {
    path: "/mcp",
    label: "MCP Servers",
    icon: "Network",
    description: "Model Context Protocol servers",
  },
  {
    path: "/skills",
    label: "Skills",
    icon: "Zap",
    description: "Agent skills and plugins",
  },
  {
    path: "/settings",
    label: "Settings",
    icon: "Settings",
    description: "Application settings",
  },
];

export const DEFAULT_ROUTE = ROUTES[0].path;
