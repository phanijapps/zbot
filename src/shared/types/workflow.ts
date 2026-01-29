// ============================================================================
// WORKFLOW TYPES
// Clean architecture types for workflow system
// ============================================================================

// ============================================================================
// GRAPH TYPES (matches .workflow/graph.yaml)
// ============================================================================

export type WorkflowPattern = 'pipeline' | 'parallel' | 'router' | 'custom';

export interface WorkflowGraphNode {
  /** Role hint: input_processor, generator, enhancer, output_formatter */
  role?: string;
  /** Human-readable description */
  description?: string;
  /** Next node(s) - string for single, array for multiple */
  next?: string | string[];
  /** Whether next nodes should execute in parallel */
  parallel?: boolean;
  /** Whether this node can be skipped */
  optional?: boolean;
}

export interface StartConfig {
  /** Trigger type: user_message, scheduled, webhook, manual */
  trigger: string;
}

export interface EndConfig {
  /** Which node provides the final output */
  output?: string;
}

export interface ConditionRule {
  /** Condition description */
  when: string;
  /** Node to route to */
  route_to?: string;
  /** Nodes to skip */
  skip?: string[];
}

/** Workflow graph definition (stored in .workflow/graph.yaml) */
export interface WorkflowGraph {
  version: number;
  pattern: WorkflowPattern;
  start: StartConfig;
  nodes: Record<string, WorkflowGraphNode>;
  end: EndConfig;
  conditions?: ConditionRule[];
}

// ============================================================================
// LAYOUT TYPES (matches .workflow/layout.json)
// ============================================================================

export interface Position {
  x: number;
  y: number;
}

export interface Viewport {
  x: number;
  y: number;
  zoom: number;
}

/** Workflow layout (stored in .workflow/layout.json) */
export interface WorkflowLayout {
  version: number;
  viewport: Viewport;
  nodes: Record<string, Position>;
}

// ============================================================================
// XY FLOW TYPES (for frontend rendering)
// ============================================================================

export interface XYFlowNodeData {
  label: string;
  displayName: string;
  description?: string;
  subagentId?: string;
  providerId?: string;
  model?: string;
  temperature?: number;
  maxTokens?: number;
  instructions?: string;
  role?: string;
  optional?: boolean;
  triggerType?: string;
}

export interface XYFlowNode {
  id: string;
  type: 'start' | 'end' | 'subagent' | 'conditional';
  position: Position;
  data: XYFlowNodeData;
}

export interface XYFlowEdge {
  id: string;
  source: string;
  target: string;
  label?: string;
  animated?: boolean;
}

// ============================================================================
// ORCHESTRATOR TYPES
// ============================================================================

export interface OrchestratorInfo {
  name: string;
  displayName: string;
  description: string;
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
  instructions: string;
  skills: string[];
  mcps: string[];
}

// ============================================================================
// COMPLETE WORKFLOW DATA
// ============================================================================

/** Complete workflow data returned by the loader */
export interface WorkflowData {
  nodes: XYFlowNode[];
  edges: XYFlowEdge[];
  orchestrator: OrchestratorInfo;
  pattern: WorkflowPattern;
}

// ============================================================================
// SUBAGENT CONFIG (matches .subagents/{name}/config.yaml)
// ============================================================================

export interface SubagentConfig {
  name: string;
  displayName: string;
  description: string;
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
  thinkingEnabled: boolean;
  voiceRecordingEnabled: boolean;
  skills: string[];
  mcps: string[];
}

// ============================================================================
// AGENT CONFIG (matches config.yaml - NO instructions)
// ============================================================================

export interface AgentConfig {
  name: string;
  displayName: string;
  description: string;
  agentType?: 'llm' | 'orchestrator';
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
  thinkingEnabled: boolean;
  voiceRecordingEnabled: boolean;
  skills: string[];
  mcps: string[];
  // NOTE: systemInstruction is NOT here - it's in AGENTS.md
}

// ============================================================================
// HELPER TYPES
// ============================================================================

export interface ValidationError {
  nodeId: string;
  message: string;
}

export interface ValidationWarning {
  nodeId: string;
  message: string;
}

export interface ValidationResult {
  valid: boolean;
  errors: ValidationError[];
  warnings: ValidationWarning[];
}
