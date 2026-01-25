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
} from '../types/workflow';
import type { OrchestratorConfig } from '@/services/workflow';

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

  // Actions - Persistence
  setIsDirty: (isDirty: boolean) => void;
  markDirty: () => void;
  markSaved: () => void;
  reset: () => void;
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

      // Graph actions
      setNodes: (nodes) => set({ nodes }),
      setEdges: (edges) => set({ edges }),

      onNodesChange: (changes) => {
        set({
          nodes: applyNodeChanges(changes, get().nodes),
          isDirty: true,
        });
      },

      onEdgesChange: (changes) => {
        set({
          edges: applyEdgeChanges(changes, get().edges),
          isDirty: true,
        });
      },

      onConnect: (connection: Connection) => {
        set({
          edges: addEdge({ ...connection, type: 'default' }, get().edges),
          isDirty: true,
        });
      },

      // Node actions
      addNode: (node) => {
        set({
          nodes: [...get().nodes, node],
          isDirty: true,
        });
      },

      updateNode: (nodeId, data) => {
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
        set({
          nodes: get().nodes.filter((node) => node.id !== nodeId),
          edges: get().edges.filter(
            (edge) => edge.source !== nodeId && edge.target !== nodeId
          ),
          selectedNodeId: get().selectedNodeId === nodeId ? null : get().selectedNodeId,
          isDirty: true,
        });
      },

      // Selection
      setSelectedNodeId: (nodeId) => set({ selectedNodeId: nodeId, selectedEdgeId: null }),
      setSelectedEdgeId: (edgeId) => set({ selectedEdgeId: edgeId, selectedNodeId: null }),

      // Orchestrator config
      setOrchestratorConfig: (config) => set({ orchestratorConfig: config }),
      updateOrchestratorConfig: (updates) => set({
        orchestratorConfig: get().orchestratorConfig
          ? { ...get().orchestratorConfig!, ...updates }
          : undefined,
        isDirty: true,
      }),

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
      }),
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
