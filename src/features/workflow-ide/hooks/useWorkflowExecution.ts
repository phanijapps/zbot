// ============================================================================
// USE WORKFLOW EXECUTION
// Hook for tracking workflow execution state from stream events
// ============================================================================

import { useCallback, useEffect } from 'react';
import { useWorkflowStore } from '../stores/workflowStore';
import type { AgentStreamEvent } from '@/shared/types/agent';

/**
 * Extract subagent ID from tool name
 * Subagent tools are named after their subagent_id (e.g., "recipe-finder")
 */
function extractSubagentId(toolName: string): string | null {
  // Skip non-subagent tools
  const NON_SUBAGENT_TOOLS = [
    'read', 'write', 'search', 'grep', 'glob', 'list_directory',
    'execute_python_code', 'execute_shell_command', 'browse',
    'date_time', 'user_input', 'request_input', 'show_content',
  ];
  
  if (NON_SUBAGENT_TOOLS.includes(toolName)) {
    return null;
  }
  
  return toolName;
}

/**
 * Map tool name to node ID in the workflow
 * Nodes are named like "subagent-{timestamp}-{random}" but have subagentId in data
 */
function findNodeIdBySubagentId(
  nodes: Array<{ id: string; data: Record<string, unknown> }>,
  subagentId: string
): string | null {
  const node = nodes.find(n => n.data.subagentId === subagentId);
  return node?.id || null;
}

/**
 * Hook for tracking workflow execution from agent stream events
 * 
 * @param nodes - The workflow nodes to track execution for
 * @returns Object with handleEvent function for processing stream events
 */
export function useWorkflowExecution(
  nodes: Array<{ id: string; data: Record<string, unknown> }>
) {
  const { setNodeExecutionStatus, clearExecution, execution } = useWorkflowStore();

  /**
   * Reset execution state when agent/nodes change
   */
  useEffect(() => {
    clearExecution();
  }, [clearExecution]);

  /**
   * Handle a stream event and update node execution status
   */
  const handleEvent = useCallback(
    (event: AgentStreamEvent) => {
      switch (event.type) {
        case 'metadata':
          // Agent started - mark orchestrator as running
          const orchestratorNode = nodes.find(n => n.data.type === 'orchestrator');
          if (orchestratorNode) {
            setNodeExecutionStatus(orchestratorNode.id, 'running');
          }
          break;

        case 'tool_call_start': {
          // Check if this is a subagent tool
          const subagentId = extractSubagentId(event.toolName);
          if (subagentId) {
            const nodeId = findNodeIdBySubagentId(nodes, subagentId);
            if (nodeId) {
              setNodeExecutionStatus(nodeId, 'running');
            }
          }
          break;
        }

        case 'tool_result': {
          // Tool execution finished
          const subagentId = extractSubagentId(event.toolName);
          if (subagentId) {
            const nodeId = findNodeIdBySubagentId(nodes, subagentId);
            if (nodeId) {
              setNodeExecutionStatus(nodeId, event.error ? 'failed' : 'completed');
            }
          }
          break;
        }

        case 'done':
          // Agent finished - mark all running nodes as completed
          execution.nodeStates &&
            Object.entries(execution.nodeStates).forEach(([nodeId, state]) => {
              if (state.status === 'running') {
                setNodeExecutionStatus(nodeId, 'completed');
              }
            });
          
          // Mark orchestrator as completed
          const orchNode = nodes.find(n => n.data.type === 'orchestrator');
          if (orchNode) {
            setNodeExecutionStatus(orchNode.id, 'completed');
          }
          break;

        case 'error':
          // Error occurred - mark running nodes as failed
          execution.nodeStates &&
            Object.entries(execution.nodeStates).forEach(([nodeId, state]) => {
              if (state.status === 'running') {
                setNodeExecutionStatus(nodeId, 'failed');
              }
            });
          break;
      }
    },
    [nodes, setNodeExecutionStatus, execution.nodeStates]
  );

  return {
    handleEvent,
    clearExecution,
    execution,
  };
}
