import React, { useEffect, useCallback, useRef, useState } from 'react';
import { useParams, useNavigate, useLocation } from 'react-router-dom';
import { ArrowLeft, Save, Loader2, Undo, Redo, Plus, Play } from 'lucide-react';
import { WorkflowEditor } from './components/WorkflowEditor';
import { NewAgentDialog } from './components/NewAgentDialog';
import { ExecutionPanel } from './components/ExecutionPanel';
import { useWorkflowStore } from './stores/workflowStore';
import { useWorkflowExecution } from './hooks/useWorkflowExecution';
import * as workflowService from '@/services/workflow';

export const WorkflowIDEPage: React.FC = () => {
  const { agentId } = useParams<{ agentId: string }>();
  const navigate = useNavigate();
  const location = useLocation();
  const { nodes, edges, setNodes, setEdges, isDirty, setIsDirty, orchestratorConfig, setOrchestratorConfig, undo, redo, canUndo, canRedo } = useWorkflowStore();
  const [loading, setLoading] = React.useState(true);
  const [saving, setSaving] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [showNewAgentDialog, setShowNewAgentDialog] = React.useState(false);

  // Execution panel state
  const [showExecutionPanel, setShowExecutionPanel] = useState(false);

  // Use workflow execution hook
  const {
    executeWorkflow,
    stopExecution,
    isExecuting,
    streamOutput,
    executionError,
  } = useWorkflowExecution(agentId || '', nodes);

  // Store navigation state for back navigation
  const fromLocationRef = useRef<string>('/');
  const restoreAgentIdRef = useRef<string | undefined>(undefined);

  // Capture where we came from and which agent to restore
  useEffect(() => {
    const state = location.state as { from?: string; restoreAgentId?: string } | null;
    fromLocationRef.current = state?.from || '/';
    restoreAgentIdRef.current = state?.restoreAgentId;
  }, [location.state]);

  // Load workflow from backend
  const loadWorkflow = useCallback(async () => {
    if (!agentId) return;

    setLoading(true);
    setError(null);

    try {
      const graph = await workflowService.getOrchestratorStructure(agentId);

      // Migration: Convert Orchestrator node to flow-level config, add Start/End nodes
      let migratedNodes = graph.nodes.filter(node => node.type);
      let migratedEdges = graph.edges.map(edge => ({
        id: edge.id,
        source: edge.source,
        target: edge.target,
        label: edge.label,
      }));

      // Check if workflow has an orchestrator node (old format)
      const orchestratorNode = migratedNodes.find(n => n.type === 'orchestrator');

      if (orchestratorNode) {
        console.log('[loadWorkflow] Migrating workflow with Orchestrator node');

        // Extract orchestrator config from node data
        const orchestratorData = orchestratorNode.data as any;
        const orchestratorConfig = orchestratorData ? {
          displayName: orchestratorData.displayName || orchestratorData.label || 'Orchestrator',
          description: orchestratorData.description || '',
          providerId: orchestratorData.providerId || '',
          model: orchestratorData.model || '',
          temperature: orchestratorData.temperature || 0.7,
          maxTokens: orchestratorData.maxTokens || 2000,
          systemInstructions: orchestratorData.instructions || orchestratorData.systemPrompt || '',
          mcps: orchestratorData.mcps || [],
          skills: orchestratorData.skills || [],
        } : undefined;

        // Set orchestrator config at flow level
        if (orchestratorConfig) {
          setOrchestratorConfig(orchestratorConfig);
        }

        // Remove orchestrator node
        migratedNodes = migratedNodes.filter(n => n.type !== 'orchestrator');

        // Remove edges connected to orchestrator
        const orchestratorId = orchestratorNode.id;
        migratedEdges = migratedEdges.filter(e =>
          e.source !== orchestratorId && e.target !== orchestratorId
        );

        // Add Start node
        const startNodeId = `start-${Date.now()}`;
        const startNode = {
          id: startNodeId,
          type: 'start' as const,
          position: { x: 100, y: 100 },
          data: {
            label: 'Start',
            triggerType: 'manual',
          },
        };
        migratedNodes.push(startNode);

        // Add End node
        const endNodeId = `end-${Date.now()}`;
        const endNode = {
          id: endNodeId,
          type: 'end' as const,
          position: { x: 100, y: 500 },
          data: {
            label: 'End',
          },
        };
        migratedNodes.push(endNode);

        // Connect Start to first subagent (if any) or End
        const firstSubagent = migratedNodes.find(n => n.type === 'subagent');
        if (firstSubagent) {
          migratedEdges.push({
            id: `edge-${startNodeId}-${firstSubagent.id}`,
            source: startNodeId,
            target: firstSubagent.id,
            label: undefined,
          });
        } else {
          // No subagents, connect Start directly to End
          migratedEdges.push({
            id: `edge-${startNodeId}-${endNodeId}`,
            source: startNodeId,
            target: endNodeId,
            label: undefined,
          });
        }

        // Connect last subagent to End (if any)
        const lastSubagent = migratedNodes.filter(n => n.type === 'subagent').pop();
        if (lastSubagent) {
          migratedEdges.push({
            id: `edge-${lastSubagent.id}-${endNodeId}`,
            source: lastSubagent.id,
            target: endNodeId,
            label: undefined,
          });
        }

        console.log('[loadWorkflow] Migration complete - Start/End nodes added');
      } else {
        // No orchestrator node found, use existing orchestrator config
        if (graph.orchestrator) {
          setOrchestratorConfig(graph.orchestrator);
        }

        // Check if Start/End nodes exist, if not add them
        const hasStart = migratedNodes.some(n => n.type === 'start');
        const hasEnd = migratedNodes.some(n => n.type === 'end');

        if (!hasStart || !hasEnd) {
          console.log('[loadWorkflow] Adding missing Start/End nodes');

          if (!hasStart) {
            const startNodeId = `start-${Date.now()}`;
            const startNode = {
              id: startNodeId,
              type: 'start' as const,
              position: { x: 100, y: 100 },
              data: {
                label: 'Start',
                triggerType: 'manual',
              },
            };
            migratedNodes.push(startNode);

            // Connect Start to first node
            const firstNode = migratedNodes[0];
            if (firstNode) {
              migratedEdges.push({
                id: `edge-${startNodeId}-${firstNode.id}`,
                source: startNodeId,
                target: firstNode.id,
                label: undefined,
              });
            }
          }

          if (!hasEnd) {
            const endNodeId = `end-${Date.now()}`;
            const endNode = {
              id: endNodeId,
              type: 'end' as const,
              position: { x: 100, y: 500 },
              data: {
                label: 'End',
              },
            };
            migratedNodes.push(endNode);

            // Connect last node to End
            const lastNode = migratedNodes[migratedNodes.length - 1];
            if (lastNode && lastNode.type !== 'end') {
              migratedEdges.push({
                id: `edge-${lastNode.id}-${endNodeId}`,
                source: lastNode.id,
                target: endNodeId,
                label: undefined,
              });
            }
          }
        }
      }

      // Convert nodes to XY Flow format - ensure data structure matches XY Flow expectations
      const xyFlowNodes = migratedNodes
        .filter(node => node.type) // Filter out nodes with undefined type
        .map(node => ({
          id: node.id,
          type: node.type as string, // Type assertion since we filtered
          position: node.position,
          data: node.data as Record<string, unknown>, // Type assertion for XY Flow compatibility
        }));

      console.log('[loadWorkflow] Loaded nodes with positions:', xyFlowNodes.map(n => ({ id: n.id, type: n.type, position: n.position })));

      // Convert edges to XY Flow format
      const xyFlowEdges = migratedEdges.map(edge => ({
        id: edge.id,
        source: edge.source,
        target: edge.target,
        label: edge.label,
      }));

      setNodes(xyFlowNodes);
      setEdges(xyFlowEdges);

      if (graph.orchestrator) {
        setOrchestratorConfig(graph.orchestrator);
      }

      // Reset dirty flag after loading - workflow is in sync with backend
      setIsDirty(false);
    } catch (err) {
      console.error('Failed to load workflow:', err);
      setError(err instanceof Error ? err.message : 'Failed to load workflow');
    } finally {
      setLoading(false);
    }
  }, [agentId, setNodes, setEdges, setOrchestratorConfig, setIsDirty]);

  // Save workflow to backend
  const saveWorkflow = useCallback(async () => {
    if (!agentId) return;

    setSaving(true);
    setError(null);

    try {
      console.log('[saveWorkflow] Saving nodes with positions:', nodes.map(n => ({ id: n.id, type: n.type, position: n.position })));
      console.log('[saveWorkflow] Saving orchestratorConfig:', orchestratorConfig);

      const graph: workflowService.WorkflowGraph = {
        nodes: nodes
          .filter(node => node.type) // Filter out nodes with undefined type
          .map(node => ({
            id: node.id,
            type: node.type as string, // Type assertion since we filtered
            position: node.position,
            data: node.data as workflowService.WorkflowNodeData, // Type assertion for service
          })),
        edges: edges.map(edge => ({
          id: edge.id,
          source: edge.source as string,
          target: edge.target as string,
          label: edge.label as string | undefined,
        })),
        orchestrator: orchestratorConfig,
      };
      
      await workflowService.saveOrchestratorStructure(agentId, graph);
      setIsDirty(false);
    } catch (err) {
      console.error('Failed to save workflow:', err);
      setError(err instanceof Error ? err.message : 'Failed to save workflow');
    } finally {
      setSaving(false);
    }
  }, [agentId, nodes, edges, orchestratorConfig, setIsDirty]);

  // Load workflow on mount
  useEffect(() => {
    loadWorkflow();
  }, [loadWorkflow]);

  if (!agentId) {
    return (
      <div className="flex items-center justify-center h-full">
        <p className="text-gray-500">No agent selected</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full bg-gray-900 text-white">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-gray-800 bg-gray-950">
        <div className="flex items-center gap-4">
          <button
            onClick={() => navigate(fromLocationRef.current, { state: { restoreAgentId: restoreAgentIdRef.current } })}
            className="p-2 hover:bg-gray-800 rounded-lg transition-colors"
          >
            <ArrowLeft size={20} />
          </button>
          <div>
            <h1 className="text-lg font-semibold">Workflow IDE</h1>
            <p className="text-sm text-gray-400">Agent: {agentId}</p>
          </div>
          <button
            onClick={() => setShowNewAgentDialog(true)}
            className="p-2 hover:bg-gray-800 rounded-lg transition-colors text-purple-400 hover:text-purple-300"
            title="Create new agent"
          >
            <Plus size={20} />
          </button>
        </div>
        
        <div className="flex items-center gap-2">
          {error && (
            <span className="text-sm text-red-400">{error}</span>
          )}
          {isDirty && (
            <span className="text-sm text-yellow-400">Unsaved changes</span>
          )}
          <button
            onClick={undo}
            disabled={!canUndo()}
            className="p-2 bg-gray-700 hover:bg-gray-600 disabled:bg-gray-800 disabled:text-gray-600 rounded-lg transition-colors"
            title="Undo (Ctrl+Z)"
          >
            <Undo size={16} />
          </button>
          <button
            onClick={redo}
            disabled={!canRedo()}
            className="p-2 bg-gray-700 hover:bg-gray-600 disabled:bg-gray-800 disabled:text-gray-600 rounded-lg transition-colors"
            title="Redo (Ctrl+Y)"
          >
            <Redo size={16} />
          </button>
          <button
            onClick={saveWorkflow}
            disabled={saving || loading || !isDirty}
            className="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-700 disabled:text-gray-500 rounded-lg transition-colors"
          >
            {saving ? (
              <>
                <Loader2 size={16} className="animate-spin" />
                Saving...
              </>
            ) : (
              <>
                <Save size={16} />
                Save
              </>
            )}
          </button>
          {/* Run/Execute button */}
          <button
            onClick={() => setShowExecutionPanel(true)}
            disabled={loading || isDirty}
            className="flex items-center gap-2 px-4 py-2 bg-green-600 hover:bg-green-700 disabled:bg-gray-700 disabled:text-gray-500 rounded-lg transition-colors"
            title={isDirty ? "Save changes before running" : "Run workflow"}
          >
            <Play size={16} />
            Run
          </button>
        </div>
      </div>

      {/* Main Content - Editor + Execution Panel */}
      <div className="flex-1 flex overflow-hidden">
        {/* Editor */}
        <div className="flex-1 overflow-hidden">
          {loading ? (
            <div className="flex items-center justify-center h-full">
              <Loader2 size={32} className="animate-spin text-gray-500" />
            </div>
          ) : (
            <WorkflowEditor agentId={agentId} />
          )}
        </div>

        {/* Execution Panel - Slide-out from right */}
        <ExecutionPanel
          isOpen={showExecutionPanel}
          onClose={() => setShowExecutionPanel(false)}
          isExecuting={isExecuting}
          onStop={stopExecution}
          onExecute={executeWorkflow}
          streamOutput={streamOutput}
          executionError={executionError}
        />
      </div>

      {/* New Agent Dialog */}
      {showNewAgentDialog && (
        <NewAgentDialog onClose={() => setShowNewAgentDialog(false)} />
      )}
    </div>
  );
};

export default WorkflowIDEPage;
