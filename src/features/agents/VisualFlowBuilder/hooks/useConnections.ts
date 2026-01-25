// ============================================================================
// VISUAL FLOW BUILDER - CONNECTIONS HOOK
// Hook for managing node connections with validation
// ============================================================================

import { useCallback } from "react";
import type { Connection, BaseNode, CanvasState } from "../types";

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export interface PortInfo {
  nodeId: string;
  portType: "input" | "output";
  portIndex?: number;
}

export interface ConnectionValidation {
  isValid: boolean;
  reason?: string;
}

// -----------------------------------------------------------------------------
// Helper: Check if node type can have input ports
// -----------------------------------------------------------------------------

function canHaveInput(nodeType: string): boolean {
  return ["subagent", "conditional", "end"].includes(nodeType);
}

// -----------------------------------------------------------------------------
// Helper: Validate connection between two ports
// -----------------------------------------------------------------------------

function validateConnection(
  sourceNode: BaseNode,
  targetNode: BaseNode,
  existingConnections: Connection[]
): ConnectionValidation {
  // Can't connect to same node
  if (sourceNode.id === targetNode.id) {
    return { isValid: false, reason: "Cannot connect to same node" };
  }

  // Start nodes only have outputs
  if (sourceNode.type === "start") {
    // Can connect to any node with input
    if (!canHaveInput(targetNode.type)) {
      return { isValid: false, reason: "Target node cannot receive input" };
    }
  }

  // Subagent nodes can connect to most nodes
  if (sourceNode.type === "subagent") {
    if (!canHaveInput(targetNode.type)) {
      return { isValid: false, reason: "Target node cannot receive input" };
    }
  }

  // Conditional nodes connect to different branches
  if (sourceNode.type === "conditional") {
    if (!canHaveInput(targetNode.type)) {
      return { isValid: false, reason: "Target node cannot receive input" };
    }
  }

  // Check for duplicate connections
  const duplicate = existingConnections.some(
    (c) => c.sourceNodeId === sourceNode.id && c.targetNodeId === targetNode.id
  );
  if (duplicate) {
    return { isValid: false, reason: "Connection already exists" };
  }

  return { isValid: true };
}

// -----------------------------------------------------------------------------
// Hook
// -----------------------------------------------------------------------------

export function useConnections(state: CanvasState) {
  // ---------------------------------------------------------------------------
  // Add connection
  // ---------------------------------------------------------------------------

  const addConnection = useCallback(
    (sourceNodeId: string, targetNodeId: string): ConnectionValidation => {
      const sourceNode = state.nodes.find((n) => n.id === sourceNodeId);
      const targetNode = state.nodes.find((n) => n.id === targetNodeId);

      if (!sourceNode || !targetNode) {
        return { isValid: false, reason: "Node not found" };
      }

      const validation = validateConnection(sourceNode, targetNode, state.connections);
      if (!validation.isValid) {
        return validation;
      }

      // This would normally dispatch an action
      // For now, return success
      return { isValid: true };
    },
    [state.nodes, state.connections]
  );

  // ---------------------------------------------------------------------------
  // Remove connection
  // ---------------------------------------------------------------------------

  const removeConnection = useCallback(
    (_connectionId: string): boolean => {
      // This would normally dispatch an action
      // For now, return success
      return true;
    },
    []
  );

  // ---------------------------------------------------------------------------
  // Get connections for a node
  // ---------------------------------------------------------------------------

  const getNodeConnections = useCallback(
    (nodeId: string): { incoming: Connection[]; outgoing: Connection[] } => {
      const incoming = state.connections.filter((c) => c.targetNodeId === nodeId);
      const outgoing = state.connections.filter((c) => c.sourceNodeId === nodeId);
      return { incoming, outgoing };
    },
    [state.connections]
  );

  // ---------------------------------------------------------------------------
  // Check if node can connect to another
  // ---------------------------------------------------------------------------

  const canConnect = useCallback(
    (sourceNodeId: string, targetNodeId: string): ConnectionValidation => {
      const sourceNode = state.nodes.find((n) => n.id === sourceNodeId);
      const targetNode = state.nodes.find((n) => n.id === targetNodeId);

      if (!sourceNode || !targetNode) {
        return { isValid: false, reason: "Node not found" };
      }

      return validateConnection(sourceNode, targetNode, state.connections);
    },
    [state.nodes, state.connections]
  );

  // ---------------------------------------------------------------------------
  // Get compatible target nodes for a source node
  // ---------------------------------------------------------------------------

  const getCompatibleTargets = useCallback(
    (sourceNodeId: string): BaseNode[] => {
      const sourceNode = state.nodes.find((n) => n.id === sourceNodeId);
      if (!sourceNode) return [];

      return state.nodes.filter((target) => {
        if (target.id === sourceNodeId) return false;
        const validation = validateConnection(sourceNode, target, state.connections);
        return validation.isValid;
      });
    },
    [state.nodes, state.connections]
  );

  // ---------------------------------------------------------------------------
  // Get connection path points for rendering
  // ---------------------------------------------------------------------------

  const getConnectionPath = useCallback(
    (connection: Connection): { startX: number; startY: number; endX: number; endY: number } | null => {
      const sourceNode = state.nodes.find((n) => n.id === connection.sourceNodeId);
      const targetNode = state.nodes.find((n) => n.id === connection.targetNodeId);

      if (!sourceNode || !targetNode) return null;

      // Start and End event nodes are circular (64x64)
      // Subagent and Conditional nodes are rectangular (240x120)
      const isSourceEventNode = sourceNode.type === "start" || sourceNode.type === "end";
      const isTargetEventNode = targetNode.type === "start" || targetNode.type === "end";

      const sourceWidth = isSourceEventNode ? 64 : 240;
      const sourceHeight = isSourceEventNode ? 64 : 120;
      const targetHeight = isTargetEventNode ? 64 : 120;

      // Calculate port positions (right side of source, left side of target)
      const startX = sourceNode.position.x + sourceWidth;
      const startY = sourceNode.position.y + sourceHeight / 2;
      const endX = targetNode.position.x;
      const endY = targetNode.position.y + targetHeight / 2;

      return { startX, startY, endX, endY };
    },
    [state.nodes]
  );

  return {
    addConnection,
    removeConnection,
    getNodeConnections,
    canConnect,
    getCompatibleTargets,
    getConnectionPath,
  };
}
