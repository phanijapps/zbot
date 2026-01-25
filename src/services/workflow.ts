// ============================================================================
// WORKFLOW SERVICE
// Tauri commands for workflow IDE integration
// ============================================================================

import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// TYPES
// ============================================================================

export interface WorkflowPosition {
  x: number;
  y: number;
}

export interface WorkflowNodeData {
  label: string;
  [key: string]: any;
}

export interface WorkflowNode {
  id: string;
  type: string;
  position: WorkflowPosition;
  data: WorkflowNodeData;
}

export interface WorkflowEdge {
  id: string;
  source: string;
  target: string;
  label?: string;
}

export interface OrchestratorConfig {
  displayName: string;
  description: string;
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
  systemInstructions: string;
  mcps: string[];
  skills: string[];
  middleware?: string;
}

export interface WorkflowGraph {
  nodes: WorkflowNode[];
  edges: WorkflowEdge[];
  orchestrator?: OrchestratorConfig;
}

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

// ============================================================================
// TAURI COMMANDS
// ============================================================================

/**
 * Get the orchestrator structure as a visual graph
 * Reads from .subagents/ folder and AGENTS.md
 */
export async function getOrchestratorStructure(agentId: string): Promise<WorkflowGraph> {
  return invoke<WorkflowGraph>('get_orchestrator_structure', { agentId });
}

/**
 * Save the orchestrator structure
 * Creates .subagents/ folders from the visual graph and generates AGENTS.md
 */
export async function saveOrchestratorStructure(
  agentId: string,
  graph: WorkflowGraph
): Promise<void> {
  return invoke<void>('save_orchestrator_structure', { agentId, graph });
}

/**
 * Validate a workflow graph
 */
export async function validateWorkflow(graph: WorkflowGraph): Promise<ValidationResult> {
  return invoke<ValidationResult>('validate_workflow', { graph });
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/**
 * Generate a unique node ID
 */
export function generateNodeId(type: string): string {
  return `${type}-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}

/**
 * Generate a unique edge ID
 */
export function generateEdgeId(source: string, target: string): string {
  return `edge-${source}-${target}-${Date.now()}`;
}

/**
 * Create a new orchestrator node
 */
export function createOrchestratorNode(config?: Partial<OrchestratorConfig>): WorkflowNode {
  return {
    id: generateNodeId('orchestrator'),
    type: 'orchestrator',
    position: { x: 100, y: 100 },
    data: {
      label: config?.displayName || 'Orchestrator',
      displayName: config?.displayName || 'Orchestrator',
      description: config?.description || '',
      providerId: config?.providerId || '',
      model: config?.model || '',
      temperature: config?.temperature ?? 0.7,
      maxTokens: config?.maxTokens ?? 2000,
    },
  };
}

/**
 * Create a new subagent node
 */
export function createSubagentNode(config?: {
  displayName?: string;
  description?: string;
  subagentId?: string;
  providerId?: string;
  model?: string;
}): WorkflowNode {
  const subagentId = config?.subagentId || `subagent-${Date.now()}`;
  return {
    id: generateNodeId('subagent'),
    type: 'subagent',
    position: { x: 400, y: 100 },
    data: {
      label: config?.displayName || 'Subagent',
      displayName: config?.displayName || 'Subagent',
      description: config?.description || '',
      subagentId,
      providerId: config?.providerId || '',
      model: config?.model || '',
      temperature: 0.7,
      maxTokens: 4096,
      systemPrompt: '',
      mcps: [],
      skills: [],
    },
  };
}

/**
 * Create a new edge between two nodes
 */
export function createEdge(
  source: string,
  target: string,
  label?: string
): WorkflowEdge {
  return {
    id: generateEdgeId(source, target),
    source,
    target,
    label,
  };
}
