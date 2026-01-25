import { useCallback, useRef, useMemo } from 'react';
import {
  ReactFlow,
  ReactFlowProvider,
  Background,
  Controls,
  MiniMap,
  BackgroundVariant,
  useReactFlow,
  Node,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';

import { nodeTypes } from './nodes';
import { NodePalette } from './panels/NodePalette';
import { PropertiesPanel } from './panels/PropertiesPanel';
import { useWorkflowStore } from '../stores/workflowStore';

interface WorkflowEditorProps {
  agentId: string;
}

const WorkflowEditorInner: React.FC<WorkflowEditorProps> = ({ agentId: _agentId }) => {
  const reactFlowWrapper = useRef<HTMLDivElement>(null);
  const { screenToFlowPosition } = useReactFlow();

  // Store
  const {
    nodes,
    edges,
    onNodesChange,
    onEdgesChange,
    onConnect,
    addNode,
    setSelectedNodeId,
    setSelectedEdgeId,
    execution,
  } = useWorkflowStore();

  // Enhance nodes with execution status for visualization
  const enhancedNodes = useMemo(() => {
    return nodes.map(node => {
      const nodeState = execution.nodeStates?.[node.id];
      return {
        ...node,
        data: {
          ...node.data,
          _executionStatus: nodeState?.status || 'idle',
        },
      };
    });
  }, [nodes, execution.nodeStates]);

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

      const newNode: Node = {
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
    (_: React.MouseEvent, node: Node) => {
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
          nodes={enhancedNodes}
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
              // Check execution status for color
              const nodeState = execution.nodeStates?.[node.id];
              if (nodeState?.status === 'running') return '#3b82f6';
              if (nodeState?.status === 'completed') return '#22c55e';
              if (nodeState?.status === 'failed') return '#ef4444';
              
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
        </ReactFlow>
      </div>

      {/* Right: Properties Panel */}
      <PropertiesPanel />
    </div>
  );
};

// Helper: Get default data for node type
function getDefaultNodeData(type: string): Record<string, any> {
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
      };
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
