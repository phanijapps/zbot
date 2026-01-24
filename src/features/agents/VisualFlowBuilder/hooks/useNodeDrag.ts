// ============================================================================
// VISUAL FLOW BUILDER - NODE DRAG HOOK
// Hook for handling node dragging functionality
// ============================================================================

import { useState, useCallback, useRef } from "react";
import type { BaseNode } from "../types";
import { CANVAS_CONFIG, NODE_DIMENSIONS } from "../constants";

// -----------------------------------------------------------------------------
// Hook Return Type
// -----------------------------------------------------------------------------

interface UseNodeDragReturn {
  isDragging: boolean;
  draggedNodeId: string | null;
  dragStart: (nodeId: string, startX: number, startY: number) => void;
  dragMove: (currentX: number, currentY: number) => void;
  dragEnd: () => void;
  setInitialPosition: (position: { x: number; y: number }) => void;
}

// -----------------------------------------------------------------------------
// Hook
// -----------------------------------------------------------------------------

export function useNodeDrag(
  onNodeUpdate: (nodeId: string, position: { x: number; y: number }) => void
): UseNodeDragReturn {
  const [isDragging, setIsDragging] = useState(false);
  const [draggedNodeId, setDraggedNodeId] = useState<string | null>(null);

  // Store initial position during drag
  const initialPositionRef = useRef<{ x: number; y: number } | null>(null);
  const dragStartPositionRef = useRef<{ x: number; y: number } | null>(null);

  // -----------------------------------------------------------------------------
  // Start Drag
  // -----------------------------------------------------------------------------

  const dragStart = useCallback(
    (nodeId: string, startX: number, startY: number) => {
      setIsDragging(true);
      setDraggedNodeId(nodeId);
      dragStartPositionRef.current = { x: startX, y: startY };

      // Get current node position (will be set by the component)
      initialPositionRef.current = null;
    },
    []
  );

  // -----------------------------------------------------------------------------
  // During Drag
  // -----------------------------------------------------------------------------

  const dragMove = useCallback(
    (currentX: number, currentY: number) => {
      if (!isDragging || !draggedNodeId || !initialPositionRef.current) {
        // First move - store initial position from current node state
        if (isDragging && draggedNodeId) {
          // This will be called by the component with the initial position
        }
        return;
      }

      const deltaX = currentX - dragStartPositionRef.current!.x;
      const deltaY = currentY - dragStartPositionRef.current!.y;

      const newPosition = {
        x: initialPositionRef.current.x + deltaX,
        y: initialPositionRef.current.y + deltaY,
      };

      // Optional: Snap to grid
      const snappedPosition = {
        x: Math.round(newPosition.x / CANVAS_CONFIG.GRID_SIZE) * CANVAS_CONFIG.GRID_SIZE,
        y: Math.round(newPosition.y / CANVAS_CONFIG.GRID_SIZE) * CANVAS_CONFIG.GRID_SIZE,
      };

      onNodeUpdate(draggedNodeId, snappedPosition);
    },
    [isDragging, draggedNodeId, onNodeUpdate]
  );

  // -----------------------------------------------------------------------------
  // End Drag
  // -----------------------------------------------------------------------------

  const dragEnd = useCallback(() => {
    setIsDragging(false);
    setDraggedNodeId(null);
    initialPositionRef.current = null;
    dragStartPositionRef.current = null;
  }, []);

  // -----------------------------------------------------------------------------
  // Set Initial Position (called by component on drag start)
  // -----------------------------------------------------------------------------

  const setInitialPosition = useCallback((position: { x: number; y: number }) => {
    initialPositionRef.current = position;
  }, []);

  return {
    isDragging,
    draggedNodeId,
    dragStart,
    dragMove,
    dragEnd,
    setInitialPosition,
  };
}

// -----------------------------------------------------------------------------
// Helper: Calculate drag delta
// -----------------------------------------------------------------------------

export function calculateDragDelta(
  startX: number,
  startY: number,
  currentX: number,
  currentY: number
): { deltaX: number; deltaY: number } {
  return {
    deltaX: currentX - startX,
    deltaY: currentY - startY,
  };
}

// -----------------------------------------------------------------------------
// Helper: Snap position to grid
// -----------------------------------------------------------------------------

export function snapToGrid(
  x: number,
  y: number,
  gridSize: number = CANVAS_CONFIG.GRID_SIZE
): { x: number; y: number } {
  return {
    x: Math.round(x / gridSize) * gridSize,
    y: Math.round(y / gridSize) * gridSize,
  };
}

// -----------------------------------------------------------------------------
// Helper: Check if point is inside node
// -----------------------------------------------------------------------------

export function isPointInNode(
  pointX: number,
  pointY: number,
  node: BaseNode,
  nodeWidth: number = NODE_DIMENSIONS.WIDTH,
  nodeHeight: number = NODE_DIMENSIONS.HEIGHT
): boolean {
  return (
    pointX >= node.position.x &&
    pointX <= node.position.x + nodeWidth &&
    pointY >= node.position.y &&
    pointY <= node.position.y + nodeHeight
  );
}

// -----------------------------------------------------------------------------
// Helper: Get node center
// -----------------------------------------------------------------------------

export function getNodeCenter(
  node: BaseNode,
  nodeWidth: number = NODE_DIMENSIONS.WIDTH,
  nodeHeight: number = NODE_DIMENSIONS.HEIGHT
): { x: number; y: number } {
  return {
    x: node.position.x + nodeWidth / 2,
    y: node.position.y + nodeHeight / 2,
  };
}
