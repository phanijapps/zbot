# AgentZero Workflow IDE - Starter Implementation

This document provides concrete, copy-paste ready code to bootstrap the new Workflow IDE.

## Prerequisites

```bash
# Install XY Flow (React Flow v12+)
npm install @xyflow/react

# You should already have these from existing project:
# - zustand (state management)
# - lucide-react (icons)
# - @radix-ui/* (UI components)
```

---

## Step 1: TypeScript Types

**File: `src/features/workflow-ide/types/workflow.ts`**

```typescript
import { Node, Edge } from '@xyflow/react';

// ============================================================================
// Node Data Types
// ============================================================================

export interface BaseNodeData {
  label: string;
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
// Typed Nodes
// ============================================================================

export type WorkflowNode = Node<WorkflowNodeData>;

export type OrchestratorNode = Node<OrchestratorNodeData, 'orchestrator'>;
export type SubagentNode = Node<SubagentNodeData, 'subagent'>;
export type ToolNode = Node<ToolNodeData, 'tool'>;
export type InputNode = Node<InputNodeData, 'input'>;
export type OutputNode = Node<OutputNodeData, 'output'>;

// ============================================================================
// Edge Types
// ============================================================================

export interface WorkflowEdgeData {
  label?: string;
  condition?: string;  // For conditional edges
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
```

---

## Step 2: Zustand Store

**File: `src/features/workflow-ide/stores/workflowStore.ts`**

```typescript
import { create } from 'zustand';
import { devtools } from 'zustand/middleware';
import {
  Node,
  Edge,
  OnNodesChange,
  OnEdgesChange,
  OnConnect,
  applyNodeChanges,
  applyEdgeChanges,
  addEdge,
  Connection,
} from '@xyflow/react';
import type {
  WorkflowNode,
  WorkflowEdge,
  WorkflowExecutionState,
  NodeExecutionStatus,
} from '../types/workflow';

interface WorkflowState {
  // Graph state
  nodes: WorkflowNode[];
  edges: WorkflowEdge[];
  
  // Selection
  selectedNodeId: string | null;
  selectedEdgeId: string | null;
  
  // Dirty tracking
  isDirty: boolean;
  lastSaved: Date | null;
  
  // Execution
  execution: WorkflowExecutionState;
  
  // Actions - Graph
  setNodes: (nodes: WorkflowNode[]) => void;
  setEdges: (edges: WorkflowEdge[]) => void;
  onNodesChange: OnNodesChange;
  onEdgesChange: OnEdgesChange;
  onConnect: OnConnect;
  
  // Actions - Nodes
  addNode: (node: WorkflowNode) => void;
  updateNode: (nodeId: string, data: Partial<WorkflowNode['data']>) => void;
  deleteNode: (nodeId: string) => void;
  
  // Actions - Selection
  setSelectedNodeId: (nodeId: string | null) => void;
  setSelectedEdgeId: (edgeId: string | null) => void;
  
  // Actions - Execution
  setNodeExecutionStatus: (nodeId: string, status: NodeExecutionStatus) => void;
  addExecutionLog: (log: Omit<ExecutionLog, 'id' | 'timestamp'>) => void;
  clearExecution: () => void;
  
  // Actions - Persistence
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
      isDirty: false,
      lastSaved: null,
      execution: initialExecutionState,

      // Graph actions
      setNodes: (nodes) => set({ nodes }),
      setEdges: (edges) => set({ edges }),
      
      onNodesChange: (changes) => {
        set({
          nodes: applyNodeChanges(changes, get().nodes) as WorkflowNode[],
          isDirty: true,
        });
      },
      
      onEdgesChange: (changes) => {
        set({
          edges: applyEdgeChanges(changes, get().edges) as WorkflowEdge[],
          isDirty: true,
        });
      },
      
      onConnect: (connection: Connection) => {
        set({
          edges: addEdge(
            { ...connection, type: 'default' },
            get().edges
          ) as WorkflowEdge[],
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
      markDirty: () => set({ isDirty: true }),
      markSaved: () => set({ isDirty: false, lastSaved: new Date() }),
      reset: () => set({
        nodes: [],
        edges: [],
        selectedNodeId: null,
        selectedEdgeId: null,
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
```

---

## Step 3: Custom Nodes

**File: `src/features/workflow-ide/components/nodes/SubagentNode.tsx`**

```typescript
import React, { memo } from 'react';
import { Handle, Position, NodeProps } from '@xyflow/react';
import { Bot, Settings, Wrench, Server } from 'lucide-react';
import type { SubagentNodeData } from '../../types/workflow';
import { cn } from '@/core/utils/cn';

export const SubagentNode = memo(({ data, selected }: NodeProps<SubagentNodeData>) => {
  const hasTools = (data.skills?.length ?? 0) > 0 || (data.mcps?.length ?? 0) > 0;

  return (
    <div
      className={cn(
        'rounded-lg border-2 bg-white shadow-md min-w-[200px]',
        'transition-all duration-200',
        selected ? 'border-blue-500 shadow-lg' : 'border-gray-200',
      )}
    >
      {/* Input Handle */}
      <Handle
        type="target"
        position={Position.Left}
        className="!w-3 !h-3 !bg-blue-500 !border-2 !border-white"
      />

      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2 bg-gradient-to-r from-purple-500 to-purple-600 text-white rounded-t-md">
        <Bot size={16} />
        <span className="font-medium text-sm truncate">
          {data.displayName || data.subagentId}
        </span>
      </div>

      {/* Body */}
      <div className="px-3 py-2 space-y-2">
        {/* Description */}
        {data.description && (
          <p className="text-xs text-gray-500 line-clamp-2">
            {data.description}
          </p>
        )}

        {/* Model Badge */}
        <div className="flex items-center gap-1">
          <span className="text-xs px-2 py-0.5 bg-gray-100 rounded text-gray-600">
            {data.model || 'No model'}
          </span>
        </div>

        {/* Tools/MCPs indicator */}
        {hasTools && (
          <div className="flex items-center gap-2 text-xs text-gray-500">
            {data.skills?.length > 0 && (
              <span className="flex items-center gap-1">
                <Wrench size={10} />
                {data.skills.length}
              </span>
            )}
            {data.mcps?.length > 0 && (
              <span className="flex items-center gap-1">
                <Server size={10} />
                {data.mcps.length}
              </span>
            )}
          </div>
        )}
      </div>

      {/* Output Handle */}
      <Handle
        type="source"
        position={Position.Right}
        className="!w-3 !h-3 !bg-green-500 !border-2 !border-white"
      />
    </div>
  );
});

SubagentNode.displayName = 'SubagentNode';
```

**File: `src/features/workflow-ide/components/nodes/OrchestratorNode.tsx`**

```typescript
import React, { memo } from 'react';
import { Handle, Position, NodeProps } from '@xyflow/react';
import { Crown, Settings } from 'lucide-react';
import type { OrchestratorNodeData } from '../../types/workflow';
import { cn } from '@/core/utils/cn';

export const OrchestratorNode = memo(({ data, selected }: NodeProps<OrchestratorNodeData>) => {
  return (
    <div
      className={cn(
        'rounded-lg border-2 bg-white shadow-md min-w-[220px]',
        'transition-all duration-200',
        selected ? 'border-amber-500 shadow-lg ring-2 ring-amber-200' : 'border-gray-200',
      )}
    >
      {/* Input Handle */}
      <Handle
        type="target"
        position={Position.Left}
        className="!w-3 !h-3 !bg-blue-500 !border-2 !border-white"
      />

      {/* Header - Distinguished styling for orchestrator */}
      <div className="flex items-center gap-2 px-3 py-2 bg-gradient-to-r from-amber-500 to-orange-500 text-white rounded-t-md">
        <Crown size={16} />
        <span className="font-semibold text-sm">
          {data.displayName || 'Orchestrator'}
        </span>
      </div>

      {/* Body */}
      <div className="px-3 py-2 space-y-2">
        {data.description && (
          <p className="text-xs text-gray-500 line-clamp-2">
            {data.description}
          </p>
        )}

        <div className="flex items-center gap-1">
          <span className="text-xs px-2 py-0.5 bg-amber-100 rounded text-amber-700 font-medium">
            {data.model || 'No model'}
          </span>
        </div>

        {/* Provider info */}
        <div className="text-xs text-gray-400">
          Provider: {data.providerId || 'Not set'}
        </div>
      </div>

      {/* Output Handle */}
      <Handle
        type="source"
        position={Position.Right}
        className="!w-3 !h-3 !bg-green-500 !border-2 !border-white"
      />
    </div>
  );
});

OrchestratorNode.displayName = 'OrchestratorNode';
```

**File: `src/features/workflow-ide/components/nodes/index.ts`**

```typescript
import { OrchestratorNode } from './OrchestratorNode';
import { SubagentNode } from './SubagentNode';

// Register all custom node types
export const nodeTypes = {
  orchestrator: OrchestratorNode,
  subagent: SubagentNode,
  // Add more node types here as implemented:
  // tool: ToolNode,
  // input: InputNode,
  // output: OutputNode,
};

export { OrchestratorNode, SubagentNode };
```

---

## Step 4: Main Editor Component

**File: `src/features/workflow-ide/components/WorkflowEditor.tsx`**

```typescript
import React, { useCallback, useRef } from 'react';
import {
  ReactFlow,
  ReactFlowProvider,
  Background,
  Controls,
  MiniMap,
  BackgroundVariant,
  useReactFlow,
  Panel,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';

import { nodeTypes } from './nodes';
import { NodePalette } from './panels/NodePalette';
import { PropertiesPanel } from './panels/PropertiesPanel';
import { WorkflowToolbar } from './WorkflowToolbar';
import { useWorkflowStore } from '../stores/workflowStore';
import type { WorkflowNode, SubagentNodeData } from '../types/workflow';

interface WorkflowEditorProps {
  agentId: string;
}

const WorkflowEditorInner: React.FC<WorkflowEditorProps> = ({ agentId }) => {
  const reactFlowWrapper = useRef<HTMLDivElement>(null);
  const { screenToFlowPosition } = useReactFlow();

  // Store
  const {
    nodes,
    edges,
    selectedNodeId,
    onNodesChange,
    onEdgesChange,
    onConnect,
    addNode,
    setSelectedNodeId,
    setSelectedEdgeId,
  } = useWorkflowStore();

  // Handle drag over (for node palette)
  const onDragOver = useCallback((event: React.DragEvent) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = 'move';
  }, []);

  // Handle drop from node palette
  const onDrop = useCallback(
    (event: React.DragEvent) => {
      event.preventDefault();

      const type = event.dataTransfer.getData('application/workflow-node-type');
      if (!type) return;

      const position = screenToFlowPosition({
        x: event.clientX,
        y: event.clientY,
      });

      const newNode: WorkflowNode = {
        id: `${type}-${Date.now()}`,
        type,
        position,
        data: getDefaultNodeData(type),
      };

      addNode(newNode);
    },
    [screenToFlowPosition, addNode]
  );

  // Node click handler
  const onNodeClick = useCallback(
    (_: React.MouseEvent, node: WorkflowNode) => {
      setSelectedNodeId(node.id);
    },
    [setSelectedNodeId]
  );

  // Edge click handler
  const onEdgeClick = useCallback(
    (_: React.MouseEvent, edge: any) => {
      setSelectedEdgeId(edge.id);
    },
    [setSelectedEdgeId]
  );

  // Pane click (deselect)
  const onPaneClick = useCallback(() => {
    setSelectedNodeId(null);
    setSelectedEdgeId(null);
  }, [setSelectedNodeId, setSelectedEdgeId]);

  return (
    <div className="flex h-full">
      {/* Left: Node Palette */}
      <NodePalette />

      {/* Center: Canvas */}
      <div className="flex-1 h-full" ref={reactFlowWrapper}>
        <ReactFlow
          nodes={nodes}
          edges={edges}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          onDrop={onDrop}
          onDragOver={onDragOver}
          onNodeClick={onNodeClick}
          onEdgeClick={onEdgeClick}
          onPaneClick={onPaneClick}
          nodeTypes={nodeTypes}
          fitView
          snapToGrid
          snapGrid={[15, 15]}
          deleteKeyCode={['Backspace', 'Delete']}
        >
          <Background variant={BackgroundVariant.Dots} gap={15} size={1} />
          <Controls />
          <MiniMap
            nodeColor={(node) => {
              switch (node.type) {
                case 'orchestrator':
                  return '#f59e0b';
                case 'subagent':
                  return '#8b5cf6';
                default:
                  return '#6b7280';
              }
            }}
            zoomable
            pannable
          />
          <Panel position="top-right">
            <WorkflowToolbar agentId={agentId} />
          </Panel>
        </ReactFlow>
      </div>

      {/* Right: Properties Panel */}
      <PropertiesPanel />
    </div>
  );
};

// Helper: Get default data for node type
function getDefaultNodeData(type: string): WorkflowNode['data'] {
  switch (type) {
    case 'orchestrator':
      return {
        label: 'Orchestrator',
        agentId: '',
        displayName: 'New Orchestrator',
        description: '',
        providerId: '',
        model: '',
        temperature: 0.7,
        maxTokens: 2000,
        systemPrompt: '',
        skills: [],
        mcps: [],
      };
    case 'subagent':
      return {
        label: 'Subagent',
        subagentId: `subagent-${Date.now()}`,
        displayName: 'New Subagent',
        description: '',
        providerId: '',
        model: '',
        temperature: 0.7,
        maxTokens: 2000,
        systemPrompt: '',
        skills: [],
        mcps: [],
      } as SubagentNodeData;
    default:
      return { label: 'Unknown' };
  }
}

// Wrapper with provider
export const WorkflowEditor: React.FC<WorkflowEditorProps> = ({ agentId }) => {
  return (
    <ReactFlowProvider>
      <WorkflowEditorInner agentId={agentId} />
    </ReactFlowProvider>
  );
};

export default WorkflowEditor;
```

---

## Step 5: Node Palette

**File: `src/features/workflow-ide/components/panels/NodePalette.tsx`**

```typescript
import React from 'react';
import { Crown, Bot, Wrench, PlayCircle, StopCircle } from 'lucide-react';

interface NodeTypeDefinition {
  type: string;
  label: string;
  icon: React.ReactNode;
  description: string;
  color: string;
}

const nodeDefinitions: NodeTypeDefinition[] = [
  {
    type: 'orchestrator',
    label: 'Orchestrator',
    icon: <Crown size={18} />,
    description: 'Main coordinating agent',
    color: 'bg-amber-100 border-amber-300 text-amber-700',
  },
  {
    type: 'subagent',
    label: 'Subagent',
    icon: <Bot size={18} />,
    description: 'Specialized worker agent',
    color: 'bg-purple-100 border-purple-300 text-purple-700',
  },
  // Future node types:
  // {
  //   type: 'tool',
  //   label: 'Tool',
  //   icon: <Wrench size={18} />,
  //   description: 'Built-in or MCP tool',
  //   color: 'bg-green-100 border-green-300 text-green-700',
  // },
];

export const NodePalette: React.FC = () => {
  const onDragStart = (event: React.DragEvent, nodeType: string) => {
    event.dataTransfer.setData('application/workflow-node-type', nodeType);
    event.dataTransfer.effectAllowed = 'move';
  };

  return (
    <div className="w-64 border-r bg-gray-50 p-4 overflow-y-auto">
      <h3 className="text-sm font-semibold text-gray-700 mb-4">Node Palette</h3>
      
      <div className="space-y-2">
        {nodeDefinitions.map((node) => (
          <div
            key={node.type}
            className={`
              flex items-center gap-3 p-3 rounded-lg border-2 cursor-grab
              transition-all duration-200 hover:shadow-md
              ${node.color}
            `}
            draggable
            onDragStart={(e) => onDragStart(e, node.type)}
          >
            <div className="flex-shrink-0">{node.icon}</div>
            <div className="min-w-0">
              <div className="font-medium text-sm">{node.label}</div>
              <div className="text-xs opacity-70 truncate">{node.description}</div>
            </div>
          </div>
        ))}
      </div>

      <div className="mt-6 pt-4 border-t">
        <h4 className="text-xs font-semibold text-gray-500 mb-2 uppercase">
          Instructions
        </h4>
        <p className="text-xs text-gray-500">
          Drag nodes onto the canvas to build your workflow. Connect nodes by
          dragging from output handles (right) to input handles (left).
        </p>
      </div>
    </div>
  );
};
```

---

## Step 6: Properties Panel

**File: `src/features/workflow-ide/components/panels/PropertiesPanel.tsx`**

```typescript
import React from 'react';
import { Settings, X } from 'lucide-react';
import { useWorkflowStore, selectSelectedNode } from '../../stores/workflowStore';
import type { SubagentNodeData, OrchestratorNodeData } from '../../types/workflow';

export const PropertiesPanel: React.FC = () => {
  const selectedNode = useWorkflowStore(selectSelectedNode);
  const updateNode = useWorkflowStore((s) => s.updateNode);
  const deleteNode = useWorkflowStore((s) => s.deleteNode);

  if (!selectedNode) {
    return (
      <div className="w-80 border-l bg-gray-50 p-4 flex flex-col items-center justify-center text-gray-400">
        <Settings size={48} className="mb-4 opacity-50" />
        <p className="text-sm">Select a node to edit properties</p>
      </div>
    );
  }

  const handleUpdate = (field: string, value: any) => {
    updateNode(selectedNode.id, { [field]: value });
  };

  return (
    <div className="w-80 border-l bg-white overflow-y-auto">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b bg-gray-50">
        <h3 className="font-semibold text-gray-700">Properties</h3>
        <button
          onClick={() => deleteNode(selectedNode.id)}
          className="p-1 text-red-500 hover:bg-red-50 rounded"
          title="Delete node"
        >
          <X size={18} />
        </button>
      </div>

      {/* Content */}
      <div className="p-4 space-y-4">
        {/* Common fields */}
        <div>
          <label className="block text-xs font-medium text-gray-500 mb-1">
            Display Name
          </label>
          <input
            type="text"
            className="w-full px-3 py-2 border rounded-md text-sm"
            value={(selectedNode.data as any).displayName || ''}
            onChange={(e) => handleUpdate('displayName', e.target.value)}
          />
        </div>

        <div>
          <label className="block text-xs font-medium text-gray-500 mb-1">
            Description
          </label>
          <textarea
            className="w-full px-3 py-2 border rounded-md text-sm"
            rows={3}
            value={(selectedNode.data as any).description || ''}
            onChange={(e) => handleUpdate('description', e.target.value)}
          />
        </div>

        {/* Subagent-specific fields */}
        {selectedNode.type === 'subagent' && (
          <>
            <div>
              <label className="block text-xs font-medium text-gray-500 mb-1">
                Subagent ID (folder name)
              </label>
              <input
                type="text"
                className="w-full px-3 py-2 border rounded-md text-sm font-mono"
                value={(selectedNode.data as SubagentNodeData).subagentId || ''}
                onChange={(e) => handleUpdate('subagentId', e.target.value.toLowerCase().replace(/\s+/g, '-'))}
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-500 mb-1">
                Provider
              </label>
              <select
                className="w-full px-3 py-2 border rounded-md text-sm"
                value={(selectedNode.data as SubagentNodeData).providerId || ''}
                onChange={(e) => handleUpdate('providerId', e.target.value)}
              >
                <option value="">Select provider...</option>
                <option value="openai">OpenAI</option>
                <option value="anthropic">Anthropic</option>
                <option value="deepseek">DeepSeek</option>
                {/* TODO: Load from configured providers */}
              </select>
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-500 mb-1">
                Model
              </label>
              <input
                type="text"
                className="w-full px-3 py-2 border rounded-md text-sm"
                value={(selectedNode.data as SubagentNodeData).model || ''}
                onChange={(e) => handleUpdate('model', e.target.value)}
                placeholder="e.g., gpt-4o-mini"
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-500 mb-1">
                Temperature ({(selectedNode.data as SubagentNodeData).temperature || 0.7})
              </label>
              <input
                type="range"
                min="0"
                max="2"
                step="0.1"
                className="w-full"
                value={(selectedNode.data as SubagentNodeData).temperature || 0.7}
                onChange={(e) => handleUpdate('temperature', parseFloat(e.target.value))}
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-500 mb-1">
                System Prompt (AGENTS.md content)
              </label>
              <textarea
                className="w-full px-3 py-2 border rounded-md text-sm font-mono"
                rows={8}
                value={(selectedNode.data as SubagentNodeData).systemPrompt || ''}
                onChange={(e) => handleUpdate('systemPrompt', e.target.value)}
                placeholder="# Instructions for this subagent..."
              />
            </div>
          </>
        )}

        {/* Node type badge */}
        <div className="pt-4 border-t">
          <span className="text-xs text-gray-400">
            Node Type: {selectedNode.type}
          </span>
        </div>
      </div>
    </div>
  );
};
```

---

## Step 7: Page Wrapper

**File: `src/features/workflow-ide/WorkflowIDEPage.tsx`**

```typescript
import React, { useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';
import { WorkflowEditor } from './components/WorkflowEditor';
import { useWorkflowStore } from './stores/workflowStore';

export const WorkflowIDEPage: React.FC = () => {
  const { agentId } = useParams<{ agentId: string }>();
  const navigate = useNavigate();
  const reset = useWorkflowStore((s) => s.reset);

  // Reset store when agent changes
  useEffect(() => {
    reset();
    // TODO: Load existing workflow from .subagents/ structure
  }, [agentId, reset]);

  if (!agentId) {
    return (
      <div className="flex items-center justify-center h-full">
        <p className="text-gray-500">No agent selected</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center gap-4 px-4 py-3 border-b bg-white">
        <button
          onClick={() => navigate(-1)}
          className="p-2 hover:bg-gray-100 rounded-lg"
        >
          <ArrowLeft size={20} />
        </button>
        <div>
          <h1 className="text-lg font-semibold">Workflow IDE</h1>
          <p className="text-sm text-gray-500">Agent: {agentId}</p>
        </div>
      </div>

      {/* Editor */}
      <div className="flex-1 overflow-hidden">
        <WorkflowEditor agentId={agentId} />
      </div>
    </div>
  );
};

export default WorkflowIDEPage;
```

---

## Step 8: Add Route

**In your `App.tsx` or router configuration:**

```typescript
import { WorkflowIDEPage } from '@/features/workflow-ide/WorkflowIDEPage';

// Add to your routes:
<Route path="/workflow/:agentId" element={<WorkflowIDEPage />} />
```

---

## What This Gets You

After implementing these files, you'll have:

1. ✅ XY Flow canvas with pan/zoom/grid
2. ✅ Draggable node palette (Orchestrator, Subagent)
3. ✅ Custom node components with proper styling
4. ✅ Properties panel for editing selected nodes
5. ✅ Zustand store for state management
6. ✅ TypeScript types for workflow structures

## Next Steps After Bootstrap

1. **Add Tauri Commands**: `get_orchestrator_structure`, `save_orchestrator_structure`
2. **Load Existing Workflows**: Read from `.subagents/` on page load
3. **Save Workflows**: Generate `.subagents/` folder structure on save
4. **Add More Node Types**: Tool, MCP, Skill nodes
5. **Execution Visualization**: Subscribe to streaming events

---

## Quick Test

After setup, run:

```bash
npm run tauri dev
```

Navigate to `/workflow/test-agent` and you should see:
- Empty canvas with grid background
- Node palette on the left
- Ability to drag Orchestrator/Subagent nodes
- Properties panel on the right when selecting nodes
