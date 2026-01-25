// ============================================================================
// ZERO IDE - TYPES
// TypeScript types for the Zero IDE Orchestrator workflow builder
// ============================================================================

// -----------------------------------------------------------------------------
// Node Types (Orchestrator Architecture - BPMN-style)
// -----------------------------------------------------------------------------

export type NodeType =
  | "start"        // Start Event (BPMN thin circle)
  | "end"          // End Event (BPMN thick circle)
  | "subagent"     // Subagent task (inline, created as tools for orchestrator)
  | "conditional"; // Conditional router node (for future use)

// -----------------------------------------------------------------------------
// Base Node Interface
// -----------------------------------------------------------------------------

export interface BaseNode {
  id: string;
  type: NodeType;
  position: { x: number; y: number };
  data: NodeData;
  selected?: boolean;
  lastModified?: number;
}

// -----------------------------------------------------------------------------
// Orchestrator Config (Flow-level LLM Configuration)
// The Orchestrator is the ONE agent that manages the entire flow
// -----------------------------------------------------------------------------

export interface OrchestratorConfig {
  displayName: string;
  description?: string;

  // LLM Configuration
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;

  // Tools (categorized selection)
  tools: ToolSelection;

  // MCPs (server IDs from mcps.json)
  mcps: string[];

  // Skills (skill names from vault/skills/)
  skills: string[];

  // Middleware
  middleware: MiddlewareConfig;

  // System Instructions
  systemInstructions: string;
}

// Default orchestrator config
export const DEFAULT_ORCHESTRATOR_CONFIG: OrchestratorConfig = {
  displayName: "My Agent",
  description: "",
  providerId: "",
  model: "",
  temperature: 0.7,
  maxTokens: 4096,
  tools: {
    fsTools: { enabled: false, tools: {} },
    kgTools: { enabled: false, tools: {} },
    execTools: { enabled: false, tools: {} },
    uiTools: { enabled: false, tools: {} },
    agentTools: { enabled: false, tools: {} },
  },
  mcps: [],
  skills: [],
  middleware: {},
  systemInstructions: "",
};

// -----------------------------------------------------------------------------
// Node Data by Type
// -----------------------------------------------------------------------------

export type NodeData =
  | StartNodeData
  | EndNodeData
  | SubagentNodeData
  | ConditionalNodeData;

// -----------------------------------------------------------------------------
// Tool Selection (Categorized)
// -----------------------------------------------------------------------------

export interface ToolSelection {
  // File System Tools
  fsTools?: ToolCategory;
  // Knowledge Graph Tools
  kgTools?: ToolCategory;
  // Execution Tools
  execTools?: ToolCategory;
  // UI Tools
  uiTools?: ToolCategory;
  // Agent Tools
  agentTools?: ToolCategory;
}

export interface ToolCategory {
  enabled: boolean;
  tools: Record<string, boolean>;
}

// Built-in tool definitions
export const BUILT_IN_TOOLS = {
  fsTools: ["read", "write", "edit", "grep", "glob"],
  kgTools: ["list_entities", "search_entities", "get_relationships", "add_entity", "add_relationship"],
  execTools: ["python", "load_skill"],
  uiTools: ["request_input", "show_content"],
  agentTools: ["create_agent"],
} as const;

// -----------------------------------------------------------------------------
// Middleware Configuration
// -----------------------------------------------------------------------------

export interface MiddlewareConfig {
  summarization?: {
    enabled: boolean;
    triggerTokens?: number;
    keepMessages?: number;
  };
  contextEditing?: {
    enabled: boolean;
    triggerTokens?: number;
    keepToolResults?: number;
  };
}

// -----------------------------------------------------------------------------
// Start Event Node Data (BPMN-style)
// -----------------------------------------------------------------------------

export interface StartNodeData {
  displayName: string;
  triggerType: "manual" | "scheduled" | "webhook";
  schedule?: string; // cron expression for scheduled triggers
}

// -----------------------------------------------------------------------------
// End Event Node Data (BPMN-style)
// -----------------------------------------------------------------------------

export interface EndNodeData {
  displayName: string;
}

// -----------------------------------------------------------------------------
// Subagent Node Data
// Subagents are mini-agents stored in .subagents/ folder
// -----------------------------------------------------------------------------

export interface SubagentNodeData {
  subagentId: string;     // Reference to subagent (e.g., "research_agent")
  displayName: string;    // Display name for the node in the flow
}

// -----------------------------------------------------------------------------
// Conditional Node Data (Orchestrator only)
// -----------------------------------------------------------------------------

export interface ConditionalNodeData {
  displayName: string;
  conditions: ConditionalRoute[];
  defaultTargetNodeId?: string; // Optional default route
}

export interface ConditionalRoute {
  id: string;
  label: string;
  expression: string;    // JavaScript expression for evaluation
  targetNodeId: string;  // Target Subtask Node
}

// -----------------------------------------------------------------------------
// Connection Types
// -----------------------------------------------------------------------------

export interface Connection {
  id: string;
  sourceNodeId: string;
  sourcePort: string;
  targetNodeId: string;
  targetPort: string;
  label?: string; // Optional label for conditional routes
}

// -----------------------------------------------------------------------------
// Port Definition
// -----------------------------------------------------------------------------

export interface Port {
  id: string;
  type: "input" | "output";
  label?: string;
  nodeId: string;
}

// -----------------------------------------------------------------------------
// Canvas Viewport State
// -----------------------------------------------------------------------------

export interface Viewport {
  x: number;
  y: number;
  zoom: number;
}

// -----------------------------------------------------------------------------
// Canvas State
// -----------------------------------------------------------------------------

export interface CanvasState {
  nodes: BaseNode[];
  connections: Connection[];
  selectedNodeId: string | null;
  viewport: Viewport;
  orchestratorConfig: OrchestratorConfig; // Flow-level LLM configuration
  validation: ValidationResult[];
}

// -----------------------------------------------------------------------------
// Canvas Actions (for reducer)
// -----------------------------------------------------------------------------

export type CanvasAction =
  | { type: "ADD_NODE"; node: BaseNode }
  | { type: "DELETE_NODE"; id: string }
  | { type: "UPDATE_NODE"; id: string; updates: Partial<BaseNode> }
  | { type: "SELECT_NODE"; id: string | null }
  | { type: "ADD_CONNECTION"; connection: Connection }
  | { type: "DELETE_CONNECTION"; id: string }
  | { type: "SET_VIEWPORT"; viewport: Viewport }
  | { type: "PAN_VIEWPORT"; deltaX: number; deltaY: number }
  | { type: "ZOOM_VIEWPORT"; zoom: number; centerX?: number; centerY?: number }
  | { type: "SET_VALIDATION"; validation: ValidationResult[] }
  | { type: "UPDATE_ORCHESTRATOR"; updates: Partial<OrchestratorConfig> };

// -----------------------------------------------------------------------------
// Validation
// -----------------------------------------------------------------------------

export interface ValidationResult {
  nodeId?: string;
  type: "error" | "warning" | "info";
  message: string;
}

// -----------------------------------------------------------------------------
// Node Template (for sidebar palette)
// -----------------------------------------------------------------------------

export interface NodeTemplate {
  type: NodeType;
  label: string;
  icon: string;
  description: string;
  defaultData: NodeData;
  category: "basic" | "flow" | "advanced";
}

// -----------------------------------------------------------------------------
// Drag State
// -----------------------------------------------------------------------------

export interface DragState {
  isDragging: boolean;
  nodeId: string | null;
  startX: number;
  startY: number;
  initialPosition: { x: number; y: number } | null;
}

// -----------------------------------------------------------------------------
// Connection Creation State
// -----------------------------------------------------------------------------

export interface ConnectionCreationState {
  isCreating: boolean;
  sourceNodeId: string | null;
  sourcePort: string | null;
  currentX: number;
  currentY: number;
}

// -----------------------------------------------------------------------------
// Auto-save State
// -----------------------------------------------------------------------------

export type SaveStatus = "saved" | "saving" | "unsaved";

// -----------------------------------------------------------------------------
// Props for Components
// ----------------------------------------------------------------------------

export interface CanvasProps {
  state: CanvasState;
  dispatch: React.Dispatch<CanvasAction>;
  onNodeClick?: (nodeId: string) => void;
  onNodeDoubleClick?: (nodeId: string) => void;
}

export interface NodeProps {
  node: BaseNode;
  isSelected: boolean;
  onSelect: () => void;
  onUpdate: (updates: Partial<BaseNode>) => void;
  onDelete: () => void;
  onPortMouseDown?: (nodeId: string, port: string, portType: "input" | "output", position: { x: number; y: number }) => void;
}

export interface PropertiesPanelProps {
  agentId?: string;
  node: BaseNode | null;
  orchestratorConfig: OrchestratorConfig;
  onClose: () => void;
  onUpdate: (updates: Partial<BaseNode>) => void;
  onUpdateOrchestrator: (updates: Partial<OrchestratorConfig>) => void;
  validationResults?: ValidationResult[];
}
