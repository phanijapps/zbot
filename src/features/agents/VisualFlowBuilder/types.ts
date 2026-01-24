// ============================================================================
// VISUAL FLOW BUILDER - TYPES
// TypeScript types for the visual workflow builder
// ============================================================================

// -----------------------------------------------------------------------------
// Node Types
// -----------------------------------------------------------------------------

export type NodeType =
  | "trigger"      // Start/Trigger node
  | "agent"        // Agent node
  | "parallel"     // Parallel split node
  | "sequential"   // Sequential flow node
  | "conditional"  // Conditional router node
  | "loop"         // Loop node
  | "aggregator"   // Merge/Aggregator node
  | "subtask";     // Subtask node

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
// Node Data by Type
// -----------------------------------------------------------------------------

export type NodeData =
  | TriggerNodeData
  | AgentNodeData
  | ParallelNodeData
  | SequentialNodeData
  | ConditionalNodeData
  | LoopNodeData
  | AggregatorNodeData
  | SubtaskNodeData;

// -----------------------------------------------------------------------------
// Trigger Node Data
// -----------------------------------------------------------------------------

export interface TriggerNodeData {
  displayName: string;
  triggerType: "manual" | "scheduled";
  schedule?: string; // cron expression for scheduled triggers
}

// -----------------------------------------------------------------------------
// Agent Node Data
// -----------------------------------------------------------------------------

export interface AgentNodeData {
  displayName: string;
  description?: string;
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
  tools: string[];
  mcps: string[];
  skills: string[];
  systemInstructions?: string;
  middleware?: string[];
}

// -----------------------------------------------------------------------------
// Parallel Node Data
// -----------------------------------------------------------------------------

export interface ParallelNodeData {
  displayName: string;
  mergeStrategy: "all" | "first" | "majority" | "concatenate";
  subagents: string[]; // Agent node IDs
}

// -----------------------------------------------------------------------------
// Sequential Node Data
// -----------------------------------------------------------------------------

export interface SequentialNodeData {
  displayName: string;
  subtasks: string[]; // Agent node IDs in order
}

// -----------------------------------------------------------------------------
// Conditional Node Data
// -----------------------------------------------------------------------------

export interface ConditionalNodeData {
  displayName: string;
  conditions: ConditionalRoute[];
}

export interface ConditionalRoute {
  id: string;
  label: string;
  expression: string; // JavaScript expression for evaluation
  targetNodeId: string;
}

// -----------------------------------------------------------------------------
// Loop Node Data
// -----------------------------------------------------------------------------

export interface LoopNodeData {
  displayName: string;
  exitCondition: string; // JavaScript expression for exit
  maxIterations: number;
  bodyNodeId: string; // Agent node ID to loop
}

// -----------------------------------------------------------------------------
// Aggregator Node Data
// -----------------------------------------------------------------------------

export interface AggregatorNodeData {
  displayName: string;
  strategy: "concatenate" | "template" | "custom";
  template?: string; // For template-based aggregation
  customInstructions?: string; // For custom aggregation
}

// -----------------------------------------------------------------------------
// Subtask Node Data
// -----------------------------------------------------------------------------

export interface SubtaskNodeData {
  displayName: string;
  context?: string; // Optional context from parent
  tasks: string[]; // List of tasks to accomplish
  goal: string; // What needs to be done
  agentNodeId: string; // Reference to agent configuration
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
  | { type: "SET_VALIDATION"; validation: ValidationResult[] };

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
}

export interface PropertiesPanelProps {
  node: BaseNode | null;
  onClose: () => void;
  onUpdate: (updates: Partial<BaseNode>) => void;
  validationResults?: ValidationResult[];
}
