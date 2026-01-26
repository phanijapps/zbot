import { create } from 'zustand';
import { devtools } from 'zustand/middleware';
import {
  OnNodesChange,
  OnEdgesChange,
  OnConnect,
  applyNodeChanges,
  applyEdgeChanges,
  addEdge,
  Connection,
  Node,
  Edge,
} from '@xyflow/react';
import type {
  WorkflowExecutionState,
  NodeExecutionStatus,
  NodeValidation,
  WorkflowValidation,
} from '../types/workflow';
import type { OrchestratorConfig, ValidationResult } from '@/services/workflow';
import * as workflowService from '@/services/workflow';
import { useWorkflowHistory, saveToHistory } from './workflowHistoryStore';

interface WorkflowState {
  // Graph state
  nodes: Node[];
  edges: Edge[];

  // Selection
  selectedNodeId: string | null;
  selectedEdgeId: string | null;

  // Orchestrator configuration (flow-level)
  orchestratorConfig?: OrchestratorConfig;

  // Dirty tracking
  isDirty: boolean;
  lastSaved: Date | null;

  // Execution
  execution: WorkflowExecutionState;

  // Validation
  validation: WorkflowValidation;

  // Actions - Graph
  setNodes: (nodes: Node[]) => void;
  setEdges: (edges: Edge[]) => void;
  onNodesChange: OnNodesChange;
  onEdgesChange: OnEdgesChange;
  onConnect: OnConnect;

  // Actions - Nodes
  addNode: (node: Node) => void;
  updateNode: (nodeId: string, data: Partial<Node['data']>) => void;
  deleteNode: (nodeId: string) => void;

  // Actions - Edges
  updateEdge: (edgeId: string, updates: Partial<Edge['data']> & { label?: string }) => void;

  // Actions - Selection
  setSelectedNodeId: (nodeId: string | null) => void;
  setSelectedEdgeId: (edgeId: string | null) => void;

  // Actions - Orchestrator Config
  setOrchestratorConfig: (config?: OrchestratorConfig) => void;
  updateOrchestratorConfig: (updates: Partial<OrchestratorConfig>) => void;

  // Actions - Execution
  setNodeExecutionStatus: (nodeId: string, status: NodeExecutionStatus) => void;
  addExecutionLog: (log: Omit<import('../types/workflow').ExecutionLog, 'id' | 'timestamp'>) => void;
  clearExecution: () => void;

  // Actions - Validation
  runValidation: () => Promise<void>;
  setValidation: (validation: WorkflowValidation) => void;

  // Actions - Persistence
  setIsDirty: (isDirty: boolean) => void;
  markDirty: () => void;
  markSaved: () => void;
  reset: () => void;

  // Actions - Undo/Redo
  undo: () => void;
  redo: () => void;
  canUndo: () => boolean;
  canRedo: () => boolean;
}

const initialExecutionState: WorkflowExecutionState = {
  isExecuting: false,
  currentNodeId: undefined,
  nodeStates: {},
  logs: [],
};

export const useWorkflowStore = create<WorkflowState>()(
  devtools(
    (set, get) => ({
      // Initial state
      nodes: [],
      edges: [],
      selectedNodeId: null,
      selectedEdgeId: null,
      orchestratorConfig: undefined,
      isDirty: false,
      lastSaved: null,
      execution: initialExecutionState,
      validation: {
        isValid: true,
        nodeErrors: 0,
        nodeWarnings: 0,
        nodes: {},
      },

      // Graph actions
      setNodes: (nodes) => {
        // Save current state to history before changing
        const currentNodes = get().nodes;
        const currentEdges = get().edges;
        const currentConfig = get().orchestratorConfig;
        saveToHistory(currentNodes, currentEdges, currentConfig);
        set({ nodes });
      },
      setEdges: (edges) => {
        // Save current state to history before changing
        const currentNodes = get().nodes;
        const currentEdges = get().edges;
        const currentConfig = get().orchestratorConfig;
        saveToHistory(currentNodes, currentEdges, currentConfig);
        set({ edges });
      },

      onNodesChange: (changes) => {
        // Only mark dirty for meaningful changes (not dimensions/select which happen on load)
        const meaningfulChanges = changes.some(
          (c) => c.type === 'position' || c.type === 'remove' || c.type === 'add'
        );
        set({
          nodes: applyNodeChanges(changes, get().nodes),
          ...(meaningfulChanges ? { isDirty: true } : {}),
        });
        // Trigger validation after node changes
        get().runValidation();
      },

      onEdgesChange: (changes) => {
        // Only mark dirty for meaningful changes (not select which happens on load)
        const meaningfulChanges = changes.some(
          (c) => c.type === 'remove' || c.type === 'add'
        );
        set({
          edges: applyEdgeChanges(changes, get().edges),
          ...(meaningfulChanges ? { isDirty: true } : {}),
        });
        // Trigger validation after edge changes
        get().runValidation();
      },

      onConnect: (connection: Connection) => {
        // Save current state to history before adding edge
        const currentNodes = get().nodes;
        const currentEdges = get().edges;
        const currentConfig = get().orchestratorConfig;
        saveToHistory(currentNodes, currentEdges, currentConfig);

        set({
          edges: addEdge({ ...connection, type: 'default' }, get().edges),
          isDirty: true,
        });
      },

      // Node actions
      addNode: (node) => {
        // Save current state to history before adding node
        const currentNodes = get().nodes;
        const currentEdges = get().edges;
        const currentConfig = get().orchestratorConfig;
        saveToHistory(currentNodes, currentEdges, currentConfig);

        set({
          nodes: [...get().nodes, node],
          isDirty: true,
        });
        get().runValidation();
      },

      updateNode: (nodeId, data) => {
        // Save current state to history before updating node
        const currentNodes = get().nodes;
        const currentEdges = get().edges;
        const currentConfig = get().orchestratorConfig;
        saveToHistory(currentNodes, currentEdges, currentConfig);

        set({
          nodes: get().nodes.map((node) =>
            node.id === nodeId
              ? { ...node, data: { ...node.data, ...data } }
              : node
          ),
          isDirty: true,
        });
      },

      deleteNode: (nodeId) => {
        // Save current state to history before deleting node
        const currentNodes = get().nodes;
        const currentEdges = get().edges;
        const currentConfig = get().orchestratorConfig;
        saveToHistory(currentNodes, currentEdges, currentConfig);

        set({
          nodes: get().nodes.filter((node) => node.id !== nodeId),
          edges: get().edges.filter(
            (edge) => edge.source !== nodeId && edge.target !== nodeId
          ),
          selectedNodeId: get().selectedNodeId === nodeId ? null : get().selectedNodeId,
          isDirty: true,
        });
        get().runValidation();
      },

      // Edge actions
      updateEdge: (edgeId, updates) => {
        // Save current state to history before updating edge
        const currentNodes = get().nodes;
        const currentEdges = get().edges;
        const currentConfig = get().orchestratorConfig;
        saveToHistory(currentNodes, currentEdges, currentConfig);

        set({
          edges: get().edges.map((edge) =>
            edge.id === edgeId
              ? {
                  ...edge,
                  data: { ...(edge.data || {}), ...updates },
                  label: updates.label !== undefined ? String(updates.label) : edge.label,
                }
              : edge
          ),
          isDirty: true,
        });
      },

      // Selection
      setSelectedNodeId: (nodeId) => set({ selectedNodeId: nodeId, selectedEdgeId: null }),
      setSelectedEdgeId: (edgeId) => set({ selectedEdgeId: edgeId, selectedNodeId: null }),

      // Orchestrator config
      setOrchestratorConfig: (config) => {
        // Save current state to history before changing config
        const currentNodes = get().nodes;
        const currentEdges = get().edges;
        const currentConfig = get().orchestratorConfig;
        saveToHistory(currentNodes, currentEdges, currentConfig);

        set({ orchestratorConfig: config });
      },
      updateOrchestratorConfig: (updates) => {
        // Save current state to history before updating config
        const currentNodes = get().nodes;
        const currentEdges = get().edges;
        const currentConfig = get().orchestratorConfig;
        saveToHistory(currentNodes, currentEdges, currentConfig);

        set({
          orchestratorConfig: get().orchestratorConfig
            ? { ...get().orchestratorConfig!, ...updates }
            : undefined,
          isDirty: true,
        });
        get().runValidation();
      },

      // Execution
      setNodeExecutionStatus: (nodeId, status) => {
        set({
          execution: {
            ...get().execution,
            currentNodeId: status === 'running' ? nodeId : get().execution.currentNodeId,
            nodeStates: {
              ...get().execution.nodeStates,
              [nodeId]: {
                nodeId,
                status,
                startedAt: status === 'running' ? new Date() : get().execution.nodeStates[nodeId]?.startedAt,
                completedAt: ['completed', 'failed'].includes(status) ? new Date() : undefined,
              },
            },
          },
        });
      },

      addExecutionLog: (log) => {
        set({
          execution: {
            ...get().execution,
            logs: [
              ...get().execution.logs,
              {
                ...log,
                id: `log-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
                timestamp: new Date(),
              },
            ],
          },
        });
      },

      clearExecution: () => set({ execution: initialExecutionState }),

      // Validation
      setValidation: (validation) => set({ validation }),

      runValidation: async () => {
        const state = get();
        const { nodes, edges, orchestratorConfig } = state;

        // Build the graph for validation
        const graph: workflowService.WorkflowGraph = {
          nodes: nodes.map((node) => ({
            id: node.id,
            type: node.type || 'unknown',
            position: { x: node.position.x, y: node.position.y },
            data: {
              label: (node.data as any).label || node.id,
              ...node.data,
            },
          })),
          edges: edges.map((edge) => ({
            id: edge.id,
            source: edge.source,
            target: edge.target,
            label: typeof edge.label === 'string' ? edge.label : undefined,
          })),
          orchestrator: orchestratorConfig,
        };

        try {
          const result: ValidationResult = await workflowService.validateWorkflow(graph);

          // Convert service format to store format
          const nodeValidations: Record<string, NodeValidation> = {};

          // Group errors by node
          for (const error of result.errors) {
            if (!nodeValidations[error.nodeId]) {
              nodeValidations[error.nodeId] = { nodeId: error.nodeId, errors: [], warnings: [] };
            }
            nodeValidations[error.nodeId].errors.push(error.message);
          }

          // Group warnings by node
          for (const warning of result.warnings) {
            if (!nodeValidations[warning.nodeId]) {
              nodeValidations[warning.nodeId] = { nodeId: warning.nodeId, errors: [], warnings: [] };
            }
            nodeValidations[warning.nodeId].warnings.push(warning.message);
          }

          const totalErrors = result.errors.length;
          const totalWarnings = result.warnings.length;

          set({
            validation: {
              isValid: totalErrors === 0,
              nodeErrors: totalErrors,
              nodeWarnings: totalWarnings,
              nodes: nodeValidations,
            },
          });
        } catch (error) {
          console.error('Validation failed:', error);
          set({
            validation: {
              isValid: false,
              nodeErrors: 1,
              nodeWarnings: 0,
              nodes: {
                global: {
                  nodeId: 'global',
                  errors: [`Validation service error: ${error}`],
                  warnings: [],
                },
              },
            },
          });
        }
      },

      // Persistence
      setIsDirty: (isDirty) => set({ isDirty }),
      markDirty: () => set({ isDirty: true }),
      markSaved: () => set({ isDirty: false, lastSaved: new Date() }),
      reset: () => set({
        nodes: [],
        edges: [],
        selectedNodeId: null,
        selectedEdgeId: null,
        orchestratorConfig: undefined,
        isDirty: false,
        lastSaved: null,
        execution: initialExecutionState,
        validation: {
          isValid: true,
          nodeErrors: 0,
          nodeWarnings: 0,
          nodes: {},
        },
      }),

      // Undo/Redo
      undo: () => {
        const historyStore = useWorkflowHistory.getState();
        const previousState = historyStore.undo();

        if (previousState) {
          set({
            nodes: previousState.nodes,
            edges: previousState.edges,
            orchestratorConfig: previousState.orchestratorConfig,
            isDirty: true,
          });
          get().runValidation();
        }
      },

      redo: () => {
        const historyStore = useWorkflowHistory.getState();
        const nextState = historyStore.redo();

        if (nextState) {
          set({
            nodes: nextState.nodes,
            edges: nextState.edges,
            orchestratorConfig: nextState.orchestratorConfig,
            isDirty: true,
          });
          get().runValidation();
        }
      },

      canUndo: () => {
        return useWorkflowHistory.getState().canUndo();
      },

      canRedo: () => {
        return useWorkflowHistory.getState().canRedo();
      },
    }),
    { name: 'WorkflowStore' }
  )
);

// Selectors
export const selectSelectedNode = (state: WorkflowState) =>
  state.nodes.find((n) => n.id === state.selectedNodeId);

export const selectSelectedEdge = (state: WorkflowState) =>
  state.edges.find((e) => e.id === state.selectedEdgeId);

export const selectSubagentNodes = (state: WorkflowState) =>
  state.nodes.filter((n) => n.type === 'subagent');
