import { Node, Edge } from '@xyflow/react';

// ============================================================================
// Node Data Types
// ============================================================================

export interface BaseNodeData {
  label: string;
  [key: string]: any; // Index signature for XY Flow compatibility
}

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
  | OrchestratorNodeData
  | SubagentNodeData
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
  agentId: string;
  displayName: string;
  description: string;
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
  skills: string[];
  mcps: string[];
  subagents: Record<string, SubagentConfig>;  // key = subagent folder name
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
