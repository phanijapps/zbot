import { Node, Edge } from '@xyflow/react';

// ============================================================================
// Node Data Types
// ============================================================================

export interface BaseNodeData {
  label: string;
  [key: string]: any; // Index signature for XY Flow compatibility
}

// Start Node Data - BPMN start event
export interface StartNodeData extends BaseNodeData {
  triggerType: 'manual' | 'scheduled' | 'webhook';
  schedule?: string; // cron expression for scheduled triggers
}

// End Node Data - BPMN end event
export interface EndNodeData extends BaseNodeData {
  // No additional properties needed
}

// Conditional Node Data - BPMN gateway for branching logic (DRAFT)
export interface ConditionalNodeData extends BaseNodeData {
  condition: string;           // Expression to evaluate
  branches: ConditionalBranch[]; // Possible branches
  defaultBranch?: string;       // Default branch ID if no conditions match
}

// Conditional branch definition
export interface ConditionalBranch {
  id: string;
  name: string;
  condition: string;           // JavaScript expression to evaluate
  targetNodeId?: string;        // Connected node for this branch
}

// Subagent Node Data - Worker agent that orchestrator can delegate to
export interface SubagentNodeData extends BaseNodeData {
  subagentId: string;        // Folder name in .subagents/
  displayName: string;
  description: string;
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
  systemPrompt: string;      // Content of AGENTS.md
  skills: string[];
  mcps: string[];
  tools: string[];           // Built-in tools enabled for this agent
  middleware?: string;       // YAML middleware configuration
}

// Orchestrator Node Data - Legacy: Only for migration purposes
// Orchestrator is now flow-level config, not a node
export interface OrchestratorNodeData extends BaseNodeData {
  agentId: string;
  displayName: string;
  description: string;
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
  systemPrompt: string;
  skills: string[];
  mcps: string[];
}

export interface ToolNodeData extends BaseNodeData {
  toolType: 'builtin' | 'mcp' | 'skill';
  toolId: string;
  enabled: boolean;
}

export interface InputNodeData extends BaseNodeData {
  description: string;
}

export interface OutputNodeData extends BaseNodeData {
  format: 'text' | 'json' | 'markdown';
}

// Union type for all node data
export type WorkflowNodeData =
  | StartNodeData
  | EndNodeData
  | ConditionalNodeData   // DRAFT - Conditional branching
  | SubagentNodeData
  | OrchestratorNodeData  // Legacy: for migration only
  | ToolNodeData
  | InputNodeData
  | OutputNodeData;

// ============================================================================
// Typed Nodes - Use XY Flow's Node type with proper typing
// ============================================================================

export type WorkflowNode = Node<WorkflowNodeData, string | undefined>;

// ============================================================================
// Edge Types
// ============================================================================

export interface WorkflowEdgeData {
  label?: string;
  condition?: string;  // For conditional edges
  [key: string]: any;
}

export type WorkflowEdge = Edge<WorkflowEdgeData>;

// ============================================================================
// Workflow Definition (matches folder structure)
// ============================================================================

export interface SubagentConfig {
  name: string;
  displayName: string;
  description: string;
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
  skills: string[];
  mcps: string[];
}

export interface OrchestratorConfig {
  displayName: string;
  description?: string;
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
  systemInstructions: string;
  mcps: string[];
  skills: string[];
  middleware?: string;
}

// ============================================================================
// Execution State
// ============================================================================

export type NodeExecutionStatus = 'idle' | 'pending' | 'running' | 'completed' | 'failed';

export interface NodeExecutionState {
  nodeId: string;
  status: NodeExecutionStatus;
  startedAt?: Date;
  completedAt?: Date;
  result?: string;
  error?: string;
}

export interface WorkflowExecutionState {
  isExecuting: boolean;
  currentNodeId?: string;
  nodeStates: Record<string, NodeExecutionState>;
  logs: ExecutionLog[];
}

export interface ExecutionLog {
  id: string;
  timestamp: Date;
  level: 'info' | 'warn' | 'error' | 'debug';
  nodeId?: string;
  message: string;
}

// ============================================================================
// Validation State
// ============================================================================

export interface NodeValidation {
  nodeId: string;
  errors: string[];
  warnings: string[];
}

export interface WorkflowValidation {
  isValid: boolean;
  nodeErrors: number;
  nodeWarnings: number;
  nodes: Record<string, NodeValidation>;
}

// ============================================================================
// Workflow Execution Events (from Tauri backend)
// ============================================================================

/**
 * Workflow stream event from the execute_workflow Tauri command
 * Event channel: workflow-stream://{invocationId}
 */
export interface WorkflowStreamEvent {
  type: 'token' | 'agent_start' | 'agent_end' | 'tool_call_start' | 'tool_result' | 'turn_complete' | 'done' | 'error' | 'cancelled' | 'unknown';
  timestamp: number;
  content?: string;
  agentId?: string;      // For agent_start/agent_end events
  toolId?: string;
  toolName?: string;
  args?: Record<string, unknown>;
  result?: string;
  error?: string;
  turnComplete?: boolean;
  finalMessage?: string;
}

/**
 * Workflow node status event for visual feedback
 * Event channel: workflow-node://{workflowId}
 */
export interface WorkflowNodeStatusEvent {
  nodeId: string;
  status: NodeExecutionStatus;
  agentId?: string;
  message?: string;
  timestamp: number;
}

/**
 * Result from execute_workflow Tauri command
 */
export interface WorkflowExecutionResult {
  workflow_id: string;
  invocation_id: string;
  session_id: string;
  response: string;
  done: boolean;
}

