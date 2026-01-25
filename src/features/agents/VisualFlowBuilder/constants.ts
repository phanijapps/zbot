// ============================================================================
// ZERO IDE - CONSTANTS
// Configuration constants for the Zero IDE Orchestrator workflow builder
// ============================================================================

import type { NodeTemplate, NodeType } from "./types";

// -----------------------------------------------------------------------------
// Canvas Configuration
// -----------------------------------------------------------------------------

export const CANVAS_CONFIG = {
  // Minimum and maximum zoom levels
  MIN_ZOOM: 0.25,
  MAX_ZOOM: 3,

  // Zoom step for wheel and button clicks
  ZOOM_STEP: 0.1,

  // Grid size for snap-to-grid (in pixels, at 100% zoom)
  GRID_SIZE: 20,

  // Canvas panning momentum
  PAN_MOMENTUM: 0.95,

  // Default viewport
  DEFAULT_VIEWPORT: {
    x: 0,
    y: 0,
    zoom: 1,
  },

  // Animation durations (in ms)
  ANIMATION_DURATION: {
    NODE_SELECT: 150,
    NODE_DRAG: 0, // Instant for responsiveness
    PANEL_SLIDE: 280,
    PANEL_FADE: 200,
    CONNECTION_DRAW: 200,
    ZOOM: 150,
  },
} as const;

// -----------------------------------------------------------------------------
// Node Dimensions
// -----------------------------------------------------------------------------

export const NODE_DIMENSIONS = {
  WIDTH: 240,
  HEIGHT: 120,
  MINI_WIDTH: 180,
  MINI_HEIGHT: 80,

  HEADER_HEIGHT: 40,
  PORT_SIZE: 12,
  PORT_SPACING: 8,

  CORNER_RADIUS: 12,
} as const;

// -----------------------------------------------------------------------------
// Node Colors
// -----------------------------------------------------------------------------

export const NODE_COLORS = {
  start: {
    bg: "from-green-500/20 to-emerald-600/20",
    border: "border-green-500/30",
    icon: "text-green-400",
    accent: "#22c55e",
  },
  end: {
    bg: "from-red-500/20 to-rose-600/20",
    border: "border-red-500/30",
    icon: "text-red-400",
    accent: "#ef4444",
  },
  subagent: {
    bg: "from-indigo-500/20 to-blue-600/20",
    border: "border-indigo-500/30",
    icon: "text-indigo-400",
    accent: "#6366f1",
  },
  conditional: {
    bg: "from-pink-500/20 to-rose-600/20",
    border: "border-pink-500/30",
    icon: "text-pink-400",
    accent: "#ec4899",
  },
} as const;

// -----------------------------------------------------------------------------
// Node Icons (Lucide React)
// -----------------------------------------------------------------------------

export const NODE_ICONS: Record<NodeType, string> = {
  start: "Play",
  end: "Circle",
  subagent: "ListChecks",
  conditional: "GitBranch",
};

// -----------------------------------------------------------------------------
// Node Templates (for sidebar palette)
// -----------------------------------------------------------------------------

export const NODE_TEMPLATES: NodeTemplate[] = [
  // Basic Nodes
  {
    type: "start",
    label: "Start",
    icon: "Play",
    description: "Workflow entry point (manual, scheduled, or webhook)",
    category: "basic",
    defaultData: {
      displayName: "Start",
      triggerType: "manual",
      schedule: undefined,
    },
  },
  {
    type: "end",
    label: "End",
    icon: "Circle",
    description: "Workflow exit point",
    category: "basic",
    defaultData: {
      displayName: "End",
    },
  },

  // Subagent Nodes (tasks that the orchestrator can delegate to)
  {
    type: "subagent",
    label: "Subagent",
    icon: "ListChecks",
    description: "Task that the orchestrator can delegate to",
    category: "basic",
    defaultData: {
      subagentId: "",           // Will be set when selecting/creating subagent
      displayName: "Subagent",
    },
  },

  // Flow Control Nodes (for future use)
  {
    type: "conditional",
    label: "Conditional",
    icon: "GitBranch",
    description: "Route workflow based on conditions",
    category: "flow",
    defaultData: {
      displayName: "Conditional Router",
      conditions: [],
      defaultTargetNodeId: undefined,
    },
  },
];

// -----------------------------------------------------------------------------
// Connection Configuration
// -----------------------------------------------------------------------------

export const CONNECTION_CONFIG = {
  // Bezier curve control point distances (as fraction of total distance)
  CONTROL_POINT_RATIO: 0.4,

  // Connection line styles
  LINE_WIDTH: 2,
  LINE_WIDTH_SELECTED: 3,

  // Animated flow indicator
  FLOW_INDICATOR_SIZE: 6,
  FLOW_INDICATOR_SPEED: 1, // pixels per frame

  // Colors
  COLOR_DEFAULT: "#6b7280",
  COLOR_SELECTED: "#8b5cf6",
  COLOR_ERROR: "#ef4444",
  COLOR_SUCCESS: "#22c55e",
  COLOR_ACTIVE: "#3b82f6",

  // Connection label style
  LABEL_PADDING: 6,
  LABEL_FONT_SIZE: 11,
} as const;

// -----------------------------------------------------------------------------
// Validation Messages
// -----------------------------------------------------------------------------

export const VALIDATION_MESSAGES = {
  NO_PROVIDER: "No provider selected",
  NO_MODEL: "No model selected",
  EMPTY_DISPLAY_NAME: "Display name cannot be empty",
  DUPLICATE_NAME: "Display name must be unique",
  NO_CONDITIONS: "Conditional node must have at least 1 condition",
  INVALID_CONDITION: "Invalid condition expression",
} as const;

// -----------------------------------------------------------------------------
// Keyboard Shortcuts
// -----------------------------------------------------------------------------

export const KEYBOARD_SHORTCUTS = {
  DELETE: ["Delete", "Backspace"],
  UNDO: ["Ctrl+Z", "Cmd+Z"],
  REDO: ["Ctrl+Shift+Z", "Cmd+Shift+Z"],
  COPY: ["Ctrl+C", "Cmd+C"],
  PASTE: ["Ctrl+V", "Cmd+V"],
  DUPLICATE: ["Ctrl+D", "Cmd+D"],
  SELECT_ALL: ["Ctrl+A", "Cmd+A"],
  ZOOM_IN: ["Ctrl++", "Cmd+="],
  ZOOM_OUT: ["Ctrl+-", "Cmd+-"],
  ZOOM_RESET: ["Ctrl+0", "Cmd+0"],
  SAVE: ["Ctrl+S", "Cmd+S"],
} as const;

// -----------------------------------------------------------------------------
// Save Configuration
// -----------------------------------------------------------------------------

export const SAVE_CONFIG = {
  // Auto-save debounce time (in ms)
  DEBOUNCE_MS: 1000,

  // Local storage key
  STORAGE_KEY: "agent_flow_autosave",
} as const;

// -----------------------------------------------------------------------------
// Export Options
// -----------------------------------------------------------------------------

export const EXPORT_OPTIONS = {
  // Image export
  IMAGE_FORMATS: ["png", "svg", "pdf"] as const,

  // Data export
  DATA_FORMATS: ["json", "yaml"] as const,

  // Default export scale
  DEFAULT_EXPORT_SCALE: 2,
} as const;

// -----------------------------------------------------------------------------
// Tool Categories (for Agent Node Properties)
// -----------------------------------------------------------------------------

export const TOOL_CATEGORIES_CONFIG = {
  fsTools: {
    label: "File System",
    icon: "📁",
    color: "text-yellow-400",
    description: "Read, write, and manipulate files",
    tools: {
      read: { name: "Read", description: "Read file contents" },
      write: { name: "Write", description: "Write content to files" },
      edit: { name: "Edit", description: "Edit file contents" },
      grep: { name: "Grep", description: "Search file contents" },
      glob: { name: "Glob", description: "Find files by pattern" },
    },
  },
  kgTools: {
    label: "Knowledge Graph",
    icon: "🕸️",
    color: "text-orange-400",
    description: "Query and manage knowledge graphs",
    tools: {
      list_entities: { name: "List Entities", description: "List all entities" },
      search_entities: { name: "Search Entities", description: "Search entities by query" },
      get_relationships: { name: "Get Relationships", description: "Get entity relationships" },
      add_entity: { name: "Add Entity", description: "Add a new entity" },
      add_relationship: { name: "Add Relationship", description: "Add entity relationship" },
    },
  },
  execTools: {
    label: "Execution",
    icon: "💻",
    color: "text-purple-400",
    description: "Execute code and load skills",
    tools: {
      python: { name: "Python", description: "Execute Python code" },
      load_skill: { name: "Load Skill", description: "Load a custom skill" },
    },
  },
  uiTools: {
    label: "UI",
    icon: "🖥️",
    color: "text-cyan-400",
    description: "User interaction tools",
    tools: {
      request_input: { name: "Request Input", description: "Request user input" },
      show_content: { name: "Show Content", description: "Display content to user" },
    },
  },
  agentTools: {
    label: "Agent",
    icon: "🤖",
    color: "text-violet-400",
    description: "Agent management tools",
    tools: {
      create_agent: { name: "Create Agent", description: "Create a sub-agent" },
    },
  },
} as const;
