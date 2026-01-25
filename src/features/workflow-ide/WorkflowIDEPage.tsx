import React, { useEffect, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { ArrowLeft, Save, Loader2 } from 'lucide-react';
import { WorkflowEditor } from './components/WorkflowEditor';
import { useWorkflowStore } from './stores/workflowStore';
import * as workflowService from '@/services/workflow';

export const WorkflowIDEPage: React.FC = () => {
  const { agentId } = useParams<{ agentId: string }>();
  const navigate = useNavigate();
  const { nodes, edges, setNodes, setEdges, isDirty, setIsDirty, setOrchestratorConfig } = useWorkflowStore();
  const [loading, setLoading] = React.useState(true);
  const [saving, setSaving] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  // Load workflow from backend
  const loadWorkflow = useCallback(async () => {
    if (!agentId) return;
    
    setLoading(true);
    setError(null);
    
    try {
      const graph = await workflowService.getOrchestratorStructure(agentId);
      
      // Convert nodes to XY Flow format - ensure data structure matches XY Flow expectations
      const xyFlowNodes = graph.nodes
        .filter(node => node.type) // Filter out nodes with undefined type
        .map(node => ({
          id: node.id,
          type: node.type as string, // Type assertion since we filtered
          position: node.position,
          data: node.data as Record<string, unknown>, // Type assertion for XY Flow compatibility
        }));
      
      // Convert edges to XY Flow format
      const xyFlowEdges = graph.edges.map(edge => ({
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
    } catch (err) {
      console.error('Failed to load workflow:', err);
      setError(err instanceof Error ? err.message : 'Failed to load workflow');
    } finally {
      setLoading(false);
    }
  }, [agentId, setNodes, setEdges, setOrchestratorConfig]);

  // Save workflow to backend
  const saveWorkflow = useCallback(async () => {
    if (!agentId) return;
    
    setSaving(true);
    setError(null);
    
    try {
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
      };
      
      await workflowService.saveOrchestratorStructure(agentId, graph);
      setIsDirty(false);
    } catch (err) {
      console.error('Failed to save workflow:', err);
      setError(err instanceof Error ? err.message : 'Failed to save workflow');
    } finally {
      setSaving(false);
    }
  }, [agentId, nodes, edges, setIsDirty]);

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
            onClick={() => navigate(-1)}
            className="p-2 hover:bg-gray-800 rounded-lg transition-colors"
          >
            <ArrowLeft size={20} />
          </button>
          <div>
            <h1 className="text-lg font-semibold">Workflow IDE</h1>
            <p className="text-sm text-gray-400">Agent: {agentId}</p>
          </div>
        </div>
        
        <div className="flex items-center gap-2">
          {error && (
            <span className="text-sm text-red-400">{error}</span>
          )}
          {isDirty && (
            <span className="text-sm text-yellow-400">Unsaved changes</span>
          )}
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
        </div>
      </div>

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
    </div>
  );
};

export default WorkflowIDEPage;
