// ============================================================================
// VISUAL FLOW BUILDER - CONNECTION MANAGER
// Component for creating and managing node connections
// ============================================================================

import { memo, useRef, useCallback, useState, useEffect } from "react";
import { calculateBezierPath } from "../utils";
import { CONNECTION_CONFIG } from "../constants";
import type { Connection, BaseNode, CanvasState } from "../types";

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface ConnectionManagerProps {
  state: CanvasState;
  connections: Connection[];
  onAddConnection: (sourceNodeId: string, targetNodeId: string) => void;
  onDeleteConnection: (connectionId: string) => void;
  viewport: { x: number; y: number; zoom: number };
}

interface DragState {
  sourceNodeId: string;
  mouseX: number;
  mouseY: number;
}

// -----------------------------------------------------------------------------
// Helper: Get port position
// -----------------------------------------------------------------------------

function getPortPosition(
  node: BaseNode,
  isSource: boolean,
  nodeWidth: number = 240,
  nodeHeight: number = 120
): { x: number; y: number } {
  if (isSource) {
    // Output port (right side)
    return {
      x: node.position.x + nodeWidth,
      y: node.position.y + nodeHeight / 2,
    };
  } else {
    // Input port (left side)
    return {
      x: node.position.x,
      y: node.position.y + nodeHeight / 2,
    };
  }
}

// -----------------------------------------------------------------------------
// Connection Manager Component
// -----------------------------------------------------------------------------

export const ConnectionManager = memo(({
  state,
  connections,
  onAddConnection,
  onDeleteConnection,
  viewport,
}: ConnectionManagerProps) => {
  const [dragState, setDragState] = useState<DragState | null>(null);
  const svgRef = useRef<SVGSVGElement>(null);

  // ---------------------------------------------------------------------------
  // Handle port click to start connection
  // ---------------------------------------------------------------------------

  const handlePortClick = useCallback((nodeId: string, isSource: boolean) => {
    if (!isSource) return; // Only start from output ports

    const node = state.nodes.find((n) => n.id === nodeId);
    if (!node) return;

    setDragState({
      sourceNodeId: nodeId,
      mouseX: 0,
      mouseY: 0,
    });
  }, [state.nodes]);

  // ---------------------------------------------------------------------------
  // Handle mouse move during connection drag
  // ---------------------------------------------------------------------------

  useEffect(() => {
    if (!dragState) return;

    const handleMouseMove = (e: MouseEvent) => {
      setDragState((prev) => {
        if (!prev) return null;
        return {
          ...prev,
          mouseX: e.clientX,
          mouseY: e.clientY,
        };
      });
    };

    const handleMouseUp = (e: MouseEvent) => {
      // Check if released over a compatible target
      const target = document.elementFromPoint(e.clientX, e.clientY);
      const targetPort = target?.closest("[data-port-type='input']");

      if (targetPort) {
        const targetNodeId = targetPort.getAttribute("data-node-id");
        if (targetNodeId && targetNodeId !== dragState.sourceNodeId) {
          onAddConnection(dragState.sourceNodeId, targetNodeId);
        }
      }

      setDragState(null);
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);

    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [dragState, onAddConnection]);

  // ---------------------------------------------------------------------------
  // Handle connection right-click to delete
  // ---------------------------------------------------------------------------

  const handleConnectionContextMenu = useCallback((e: React.MouseEvent, connectionId: string) => {
    e.preventDefault();
    if (confirm("Delete this connection?")) {
      onDeleteConnection(connectionId);
    }
  }, [onDeleteConnection]);

  // ---------------------------------------------------------------------------
  // Calculate connection path
  // ---------------------------------------------------------------------------

  const getConnectionPath = useCallback((connection: Connection): string | null => {
    const sourceNode = state.nodes.find((n) => n.id === connection.sourceNodeId);
    const targetNode = state.nodes.find((n) => n.id === connection.targetNodeId);

    if (!sourceNode || !targetNode) return null;

    const sourcePos = getPortPosition(sourceNode, true);
    const targetPos = getPortPosition(targetNode, false);

    return calculateBezierPath(
      sourcePos.x,
      sourcePos.y,
      targetPos.x,
      targetPos.y,
      CONNECTION_CONFIG.CONTROL_POINT_RATIO * viewport.zoom
    );
  }, [state.nodes, viewport.zoom]);

  // ---------------------------------------------------------------------------
  // Calculate drag line path
  // ---------------------------------------------------------------------------

  const getDragLinePath = useCallback((): string | null => {
    if (!dragState) return null;

    const sourceNode = state.nodes.find((n) => n.id === dragState.sourceNodeId);
    if (!sourceNode || !svgRef.current) return null;

    const sourcePos = getPortPosition(sourceNode, true);

    // Convert mouse coordinates to canvas coordinates
    const rect = svgRef.current.getBoundingClientRect();
    const endX = (dragState.mouseX - rect.left - viewport.x) / viewport.zoom;
    const endY = (dragState.mouseY - rect.top - viewport.y) / viewport.zoom;

    return calculateBezierPath(
      sourcePos.x,
      sourcePos.y,
      endX,
      endY,
      CONNECTION_CONFIG.CONTROL_POINT_RATIO * viewport.zoom
    );
  }, [dragState, state.nodes, viewport]);

  // ---------------------------------------------------------------------------
  // Register port click handlers
  // ---------------------------------------------------------------------------

  useEffect(() => {
    // This would set up click listeners on all ports
    // For now, ports are handled within node components
    const handlePortClickEvent = (e: Event) => {
      const target = e.currentTarget as HTMLElement;
      const nodeId = target.getAttribute("data-node-id");
      const portType = target.getAttribute("data-port-type");

      if (nodeId && portType === "output") {
        handlePortClick(nodeId, true);
      }
    };

    // Set up listeners on ports
    document.querySelectorAll("[data-port-type='output']").forEach((port) => {
      port.addEventListener("click", handlePortClickEvent);
    });

    return () => {
      document.querySelectorAll("[data-port-type='output']").forEach((port) => {
        port.removeEventListener("click", handlePortClickEvent);
      });
    };
  }, [handlePortClick, state.nodes]);

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  return (
    <svg
      ref={svgRef}
      className="absolute inset-0 pointer-events-none"
      width="100%"
      height="100%"
      style={{ overflow: "visible" }}
    >
      <g
        transform={`translate(${viewport.x}, ${viewport.y}) scale(${viewport.zoom})`}
      >
        {/* Arrow marker definition */}
        <defs>
          <marker
            id="arrowhead"
            markerWidth="10"
            markerHeight="7"
            refX="9"
            refY="3.5"
            orient="auto"
          >
            <polygon
              points="0 0, 10 3.5, 0 7"
              fill={CONNECTION_CONFIG.COLOR_DEFAULT}
            />
          </marker>
        </defs>

        {/* Existing connections */}
        {connections.map((connection) => {
          const path = getConnectionPath(connection);
          if (!path) return null;

          return (
            <g key={connection.id} className="pointer-events-auto">
              {/* Invisible wider path for easier clicking */}
              <path
                d={path}
                stroke="transparent"
                strokeWidth={CONNECTION_CONFIG.LINE_WIDTH * 4}
                fill="none"
                onContextMenu={(e) => handleConnectionContextMenu(e, connection.id)}
              />

              {/* Visible connection line */}
              <path
                d={path}
                stroke={CONNECTION_CONFIG.COLOR_DEFAULT}
                strokeWidth={CONNECTION_CONFIG.LINE_WIDTH * viewport.zoom}
                fill="none"
                markerEnd="url(#arrowhead)"
                className="cursor-context-menu"
                onContextMenu={(e) => handleConnectionContextMenu(e, connection.id)}
              />
            </g>
          );
        })}

        {/* Drag line when creating new connection */}
        {dragState && getDragLinePath() && (
          <path
            d={getDragLinePath()!}
            stroke={CONNECTION_CONFIG.COLOR_ACTIVE}
            strokeWidth={CONNECTION_CONFIG.LINE_WIDTH * viewport.zoom}
            fill="none"
            strokeDasharray={`${5 * viewport.zoom}`}
            className="pointer-events-none"
          />
        )}
      </g>
    </svg>
  );
});

ConnectionManager.displayName = "ConnectionManager";
