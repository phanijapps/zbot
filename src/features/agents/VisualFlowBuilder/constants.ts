// ============================================================================
// VISUAL FLOW BUILDER - CONSTANTS
// Configuration constants for the visual workflow builder
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
  trigger: {
    bg: "from-green-500/20 to-emerald-600/20",
    border: "border-green-500/30",
    icon: "text-green-400",
    accent: "#22c55e",
  },
  agent: {
    bg: "from-violet-500/20 to-purple-600/20",
    border: "border-violet-500/30",
    icon: "text-violet-400",
    accent: "#8b5cf6",
  },
  parallel: {
    bg: "from-blue-500/20 to-cyan-600/20",
    border: "border-blue-500/30",
    icon: "text-blue-400",
    accent: "#3b82f6",
  },
  sequential: {
    bg: "from-orange-500/20 to-amber-600/20",
    border: "border-orange-500/30",
    icon: "text-orange-400",
    accent: "#f97316",
  },
  conditional: {
    bg: "from-pink-500/20 to-rose-600/20",
    border: "border-pink-500/30",
    icon: "text-pink-400",
    accent: "#ec4899",
  },
  loop: {
    bg: "from-yellow-500/20 to-lime-600/20",
    border: "border-yellow-500/30",
    icon: "text-yellow-400",
    accent: "#eab308",
  },
  aggregator: {
    bg: "from-teal-500/20 to-cyan-600/20",
    border: "border-teal-500/30",
    icon: "text-teal-400",
    accent: "#14b8a6",
  },
  subtask: {
    bg: "from-indigo-500/20 to-blue-600/20",
    border: "border-indigo-500/30",
    icon: "text-indigo-400",
    accent: "#6366f1",
  },
} as const;

// -----------------------------------------------------------------------------
// Node Icons (Lucide React)
// -----------------------------------------------------------------------------

export const NODE_ICONS: Record<NodeType, string> = {
  trigger: "Play",
  agent: "Bot",
  parallel: "Zap",
  sequential: "ArrowRight",
  conditional: "GitBranch",
  loop: "Repeat",
  aggregator: "Merge",
  subtask: "ListChecks",
};

// -----------------------------------------------------------------------------
// Node Templates (for sidebar palette)
// -----------------------------------------------------------------------------

export const NODE_TEMPLATES: NodeTemplate[] = [
  // Basic Nodes
  {
    type: "trigger",
    label: "Start",
    icon: "Play",
    description: "Manual or scheduled trigger",
    category: "basic",
    defaultData: {
      displayName: "Start",
      triggerType: "manual",
      schedule: undefined,
    },
  },
  {
    type: "agent",
    label: "Agent",
    icon: "Bot",
    description: "AI agent with model and tools",
    category: "basic",
    defaultData: {
      displayName: "New Agent",
      description: "",
      providerId: "",
      model: "",
      temperature: 0.7,
      maxTokens: 4096,
      tools: [],
      mcps: [],
      skills: [],
      systemInstructions: "",
      middleware: [],
    },
  },

  // Flow Control Nodes
  {
    type: "parallel",
    label: "Parallel",
    icon: "Zap",
    description: "Execute multiple agents concurrently",
    category: "flow",
    defaultData: {
      displayName: "Parallel Split",
      mergeStrategy: "all",
      subagents: [],
    },
  },
  {
    type: "sequential",
    label: "Sequential",
    icon: "ArrowRight",
    description: "Execute agents in order",
    category: "flow",
    defaultData: {
      displayName: "Sequential Flow",
      subtasks: [],
    },
  },
  {
    type: "conditional",
    label: "Conditional",
    icon: "GitBranch",
    description: "Route based on conditions",
    category: "flow",
    defaultData: {
      displayName: "Conditional Router",
      conditions: [],
    },
  },
  {
    type: "loop",
    label: "Loop",
    icon: "Repeat",
    description: "Repeat until condition met",
    category: "flow",
    defaultData: {
      displayName: "Loop",
      exitCondition: "",
      maxIterations: 3,
      bodyNodeId: "",
    },
  },
  {
    type: "aggregator",
    label: "Merge",
    icon: "Merge",
    description: "Combine multiple responses",
    category: "flow",
    defaultData: {
      displayName: "Aggregator",
      strategy: "concatenate",
      template: "",
      customInstructions: "",
    },
  },

  // Advanced Nodes
  {
    type: "subtask",
    label: "Subtask",
    icon: "ListChecks",
    description: "Parallel subtask with own context",
    category: "advanced",
    defaultData: {
      displayName: "Subtask",
      context: "",
      tasks: [],
      goal: "",
      agentNodeId: "",
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
  NO_TOOLS: "No tools selected (agent will have limited capabilities)",
  EMPTY_DISPLAY_NAME: "Display name cannot be empty",
  DUPLICATE_NAME: "Display name must be unique",
  NO_SUBAGENTS: "Parallel node must have at least 2 subagents",
  NO_CONDITIONS: "Conditional node must have at least 2 conditions",
  NO_EXIT_CONDITION: "Loop node must have an exit condition",
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
