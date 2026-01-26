// ============================================================================
// USE WORKFLOW EXECUTION
// Hook for executing workflows and tracking execution state from stream events
// ============================================================================

import { useCallback, useEffect, useState, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { useWorkflowStore } from '../stores/workflowStore';
import type { AgentStreamEvent } from '@/shared/types/agent';
import type {
  WorkflowStreamEvent,
  WorkflowNodeStatusEvent,
  WorkflowExecutionResult,
} from '../types/workflow';

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
  console.log('[Workflow] Finding node for subagentId:', subagentId);
  console.log('[Workflow] Available nodes:', nodes.map(n => ({ id: n.id, subagentId: n.data.subagentId })));
  const node = nodes.find(n => n.data.subagentId === subagentId);
  console.log('[Workflow] Found node:', node?.id || 'NOT FOUND');
  return node?.id || null;
}

/**
 * Hook for executing workflows and tracking execution state
 *
 * @param agentId - The agent/workflow ID to execute
 * @param nodes - The workflow nodes to track execution for
 * @returns Object with execution state and control functions
 */
export function useWorkflowExecution(
  agentId: string,
  nodes: Array<{ id: string; type?: string; data: Record<string, unknown> }>
) {
  const { setNodeExecutionStatus, clearExecution, execution, addExecutionLog } = useWorkflowStore();

  // Execution state
  const [isExecuting, setIsExecuting] = useState(false);
  const [currentInvocationId, setCurrentInvocationId] = useState<string | null>(null);
  const [streamOutput, setStreamOutput] = useState<string>('');
  const [executionError, setExecutionError] = useState<string | null>(null);

  // Track active listeners for cleanup
  const listenersRef = useRef<UnlistenFn[]>([]);

  /**
   * Reset execution state when agent/nodes change
   */
  useEffect(() => {
    clearExecution();
    setStreamOutput('');
    setExecutionError(null);
  }, [agentId, clearExecution]);

  /**
   * Cleanup listeners on unmount
   */
  useEffect(() => {
    return () => {
      listenersRef.current.forEach(unlisten => unlisten());
      listenersRef.current = [];
    };
  }, []);

  /**
   * Handle a stream event and update node execution status
   */
  const handleStreamEvent = useCallback(
    (event: WorkflowStreamEvent) => {
      switch (event.type) {
        case 'token':
          if (event.content) {
            setStreamOutput(prev => prev + event.content);
          }
          break;

        case 'agent_start': {
          // Agent lifecycle event - agent started
          if (event.agentId) {
            const nodeId = findNodeIdBySubagentId(nodes, event.agentId);
            if (nodeId) {
              setNodeExecutionStatus(nodeId, 'running');
              addExecutionLog({
                level: 'info',
                nodeId,
                message: `Agent started: ${event.agentId}`,
              });
            }
          }
          break;
        }

        case 'agent_end': {
          // Agent lifecycle event - agent completed
          if (event.agentId) {
            const nodeId = findNodeIdBySubagentId(nodes, event.agentId);
            if (nodeId) {
              setNodeExecutionStatus(nodeId, 'completed');
              addExecutionLog({
                level: 'info',
                nodeId,
                message: `Agent completed: ${event.agentId}`,
              });
            }
          }
          break;
        }

        case 'tool_call_start': {
          if (event.toolName) {
            const subagentId = extractSubagentId(event.toolName);
            if (subagentId) {
              const nodeId = findNodeIdBySubagentId(nodes, subagentId);
              if (nodeId) {
                setNodeExecutionStatus(nodeId, 'running');
                addExecutionLog({
                  level: 'info',
                  nodeId,
                  message: `Executing subagent: ${event.toolName}`,
                });
              }
            }
          }
          break;
        }

        case 'tool_result': {
          if (event.toolName) {
            const subagentId = extractSubagentId(event.toolName);
            if (subagentId) {
              const nodeId = findNodeIdBySubagentId(nodes, subagentId);
              if (nodeId) {
                setNodeExecutionStatus(nodeId, 'completed');
                addExecutionLog({
                  level: 'info',
                  nodeId,
                  message: `Completed: ${event.toolName}`,
                });
              }
            }
          }
          break;
        }

        case 'done':
          // Mark all running nodes as completed
          Object.entries(execution.nodeStates || {}).forEach(([nodeId, state]) => {
            if (state.status === 'running') {
              setNodeExecutionStatus(nodeId, 'completed');
            }
          });

          // Mark end node as completed if exists
          const endNode = nodes.find(n => n.type === 'end');
          if (endNode) {
            setNodeExecutionStatus(endNode.id, 'completed');
          }

          setIsExecuting(false);
          addExecutionLog({
            level: 'info',
            message: 'Workflow execution completed',
          });
          break;

        case 'error':
          // Mark running nodes as failed
          Object.entries(execution.nodeStates || {}).forEach(([nodeId, state]) => {
            if (state.status === 'running') {
              setNodeExecutionStatus(nodeId, 'failed');
            }
          });

          setIsExecuting(false);
          setExecutionError(event.error || 'Unknown error');
          addExecutionLog({
            level: 'error',
            message: event.error || 'Unknown error',
          });
          break;

        case 'cancelled':
          // Execution was stopped by user
          setIsExecuting(false);
          addExecutionLog({
            level: 'warn',
            message: 'Execution cancelled by user',
          });
          break;
      }
    },
    [nodes, setNodeExecutionStatus, execution.nodeStates, addExecutionLog]
  );

  /**
   * Handle node status event (for visual feedback)
   */
  const handleNodeStatusEvent = useCallback(
    (event: WorkflowNodeStatusEvent) => {
      setNodeExecutionStatus(event.nodeId, event.status);
      if (event.message) {
        addExecutionLog({
          level: 'info',
          nodeId: event.nodeId,
          message: event.message,
        });
      }
    },
    [setNodeExecutionStatus, addExecutionLog]
  );

  /**
   * Execute the workflow with a user message
   */
  const executeWorkflow = useCallback(
    async (message: string): Promise<WorkflowExecutionResult | null> => {
      if (isExecuting) {
        console.warn('Workflow is already executing');
        return null;
      }

      // Reset state
      clearExecution();
      setStreamOutput('');
      setExecutionError(null);
      setIsExecuting(true);

      // Mark start node as running
      const startNode = nodes.find(n => n.type === 'start');
      if (startNode) {
        setNodeExecutionStatus(startNode.id, 'running');
      }

      addExecutionLog({
        level: 'info',
        message: `Starting workflow execution: ${message}`,
      });

      // Generate invocation ID on frontend to avoid race condition
      const invocationId = crypto.randomUUID();
      setCurrentInvocationId(invocationId);

      // Set up event listeners BEFORE calling invoke to avoid missing events
      const streamChannel = `workflow-stream://${invocationId}`;
      const nodeChannel = `workflow-node://${agentId}`;

      // Clean up any existing listeners
      listenersRef.current.forEach(unlisten => unlisten());
      listenersRef.current = [];

      try {
        // Listen for stream events
        const streamUnlisten = await listen<WorkflowStreamEvent>(streamChannel, (event) => {
          console.log('[Workflow] Stream event:', event.payload.type, event.payload);
          handleStreamEvent(event.payload);
        });
        listenersRef.current.push(streamUnlisten);

        // Listen for node status events
        const nodeUnlisten = await listen<WorkflowNodeStatusEvent>(nodeChannel, (event) => {
          console.log('[Workflow] Node status event:', event.payload);
          handleNodeStatusEvent(event.payload);
        });
        listenersRef.current.push(nodeUnlisten);

        // Now call the Tauri command - pass invocationId so backend uses it
        const result = await invoke<WorkflowExecutionResult>('execute_workflow', {
          agentId,
          message,
          sessionId: null, // Auto-generate
          invocationId, // Pass our generated ID
        });

        // Mark start node as completed
        if (startNode) {
          setNodeExecutionStatus(startNode.id, 'completed');
        }

        return result;
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : String(error);
        setExecutionError(errorMessage);
        setIsExecuting(false);

        // Mark start node as failed
        if (startNode) {
          setNodeExecutionStatus(startNode.id, 'failed');
        }

        addExecutionLog({
          level: 'error',
          message: `Execution failed: ${errorMessage}`,
        });

        return null;
      }
    },
    [
      agentId,
      nodes,
      isExecuting,
      clearExecution,
      setNodeExecutionStatus,
      addExecutionLog,
      handleStreamEvent,
      handleNodeStatusEvent,
    ]
  );

  /**
   * Stop the current execution
   */
  const stopExecution = useCallback(async () => {
    // Call backend to stop the workflow
    if (currentInvocationId) {
      try {
        await invoke('stop_workflow', { invocationId: currentInvocationId });
        console.log('[Workflow] Stop command sent for:', currentInvocationId);
      } catch (error) {
        console.error('[Workflow] Failed to stop execution:', error);
      }
    }

    // Clean up listeners
    listenersRef.current.forEach(unlisten => unlisten());
    listenersRef.current = [];
    setIsExecuting(false);
    setCurrentInvocationId(null);

    addExecutionLog({
      level: 'warn',
      message: 'Workflow execution stopped by user',
    });
  }, [currentInvocationId, addExecutionLog]);

  /**
   * Handle a stream event from the legacy agent execution system
   * (for compatibility with existing agent chat)
   */
  const handleEvent = useCallback(
    (event: AgentStreamEvent) => {
      switch (event.type) {
        case 'metadata':
          // Agent started - mark orchestrator/start as running
          const startOrOrchNode = nodes.find(n => n.type === 'orchestrator' || n.type === 'start');
          if (startOrOrchNode) {
            setNodeExecutionStatus(startOrOrchNode.id, 'running');
          }
          break;

        case 'tool_call_start': {
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
          execution.nodeStates &&
            Object.entries(execution.nodeStates).forEach(([nodeId, state]) => {
              if (state.status === 'running') {
                setNodeExecutionStatus(nodeId, 'completed');
              }
            });

          const orchNode = nodes.find(n => n.type === 'orchestrator' || n.type === 'start');
          if (orchNode) {
            setNodeExecutionStatus(orchNode.id, 'completed');
          }
          break;

        case 'error':
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
    // Execution control
    executeWorkflow,
    stopExecution,

    // State
    isExecuting,
    currentInvocationId,
    streamOutput,
    executionError,
    execution,

    // Legacy compatibility
    handleEvent,
    clearExecution,
  };
}
