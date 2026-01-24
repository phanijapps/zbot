// ============================================================================
// VISUAL FLOW BUILDER - CONNECTION LINES
// Renders bezier curve connections between nodes
// ============================================================================

import React, { memo, useMemo } from "react";
import type { Connection, BaseNode } from "../types";
import { CONNECTION_CONFIG, NODE_DIMENSIONS } from "../constants";
import { calculateBezierPath } from "../utils";

interface ConnectionLinesProps {
  connections: Connection[];
  nodes: BaseNode[];
  viewportX: number;
  viewportY: number;
  zoom: number;
  selectedConnectionId: string | null;
  onConnectionClick?: (connectionId: string) => void;
  onConnectionDoubleClick?: (connectionId: string) => void;
  onConnectionRightClick?: (connectionId: string, e: React.MouseEvent) => void;
}

// -----------------------------------------------------------------------------
// Get port position on a node
// -----------------------------------------------------------------------------

function getPortPosition(
  node: BaseNode,
  _port: string,
  isSource: boolean
): { x: number; y: number } {
  const width = NODE_DIMENSIONS.WIDTH;
  const height = NODE_DIMENSIONS.HEIGHT;

  // Default positions (left side for input, right side for output)
  if (isSource) {
    // Output port on the right side
    return {
      x: node.position.x + width,
      y: node.position.y + height / 2,
    };
  } else {
    // Input port on the left side
    return {
      x: node.position.x,
      y: node.position.y + height / 2,
    };
  }
}

// -----------------------------------------------------------------------------
// Connection Line Component
// -----------------------------------------------------------------------------

interface ConnectionLineProps {
  connection: Connection;
  sourceNode: BaseNode;
  targetNode: BaseNode;
  isSelected: boolean;
  viewportX: number;
  viewportY: number;
  zoom: number;
  onClick?: () => void;
  onDoubleClick?: () => void;
  onRightClick?: (e: React.MouseEvent) => void;
}

const ConnectionLine = memo(({
  connection,
  sourceNode,
  targetNode,
  isSelected,
  viewportX,
  viewportY,
  zoom,
  onClick,
  onDoubleClick,
  onRightClick,
}: ConnectionLineProps) => {
  // Calculate port positions
  const sourcePos = getPortPosition(sourceNode, connection.sourcePort, true);
  const targetPos = getPortPosition(targetNode, connection.targetPort, false);

  // Apply viewport transform
  const startX = sourcePos.x * zoom + viewportX;
  const startY = sourcePos.y * zoom + viewportY;
  const endX = targetPos.x * zoom + viewportX;
  const endY = targetPos.y * zoom + viewportY;

  // Calculate bezier path
  const path = calculateBezierPath(startX, startY, endX, endY, CONNECTION_CONFIG.CONTROL_POINT_RATIO * zoom);
  const lineWidth = isSelected ? CONNECTION_CONFIG.LINE_WIDTH_SELECTED : CONNECTION_CONFIG.LINE_WIDTH;
  const color = isSelected ? CONNECTION_CONFIG.COLOR_SELECTED : CONNECTION_CONFIG.COLOR_DEFAULT;

  return (
    <g className="connection-line">
      {/* Invisible wider path for easier clicking */}
      <path
        d={path}
        fill="none"
        stroke="transparent"
        strokeWidth={lineWidth + 10}
        style={{ cursor: "pointer" }}
        onClick={onClick}
        onDoubleClick={onDoubleClick}
        onContextMenu={onRightClick}
      />

      {/* Visible connection line */}
      <path
        d={path}
        fill="none"
        stroke={color}
        strokeWidth={lineWidth}
        strokeLinecap="round"
        style={{
          transition: "stroke 0.2s, stroke-width 0.2s",
          pointerEvents: "none",
        }}
      />

      {/* Optional: Connection label */}
      {connection.label && (
        <g
          transform={`translate(${(startX + endX) / 2}, ${(startY + endY) / 2})`}
          style={{ pointerEvents: "none" }}
        >
          <rect
            x={-connection.label.length * 3}
            y={-CONNECTION_CONFIG.LABEL_FONT_SIZE - CONNECTION_CONFIG.LABEL_PADDING}
            width={connection.label.length * 6}
            height={CONNECTION_CONFIG.LABEL_FONT_SIZE + CONNECTION_CONFIG.LABEL_PADDING * 2}
            fill="rgba(0, 0, 0, 0.8)"
            rx={4}
          />
          <text
            fill="rgba(255, 255, 255, 0.9)"
            fontSize={CONNECTION_CONFIG.LABEL_FONT_SIZE}
            textAnchor="middle"
            dominantBaseline="text-before-edge"
          >
            {connection.label}
          </text>
        </g>
      )}

      {/* Animated flow indicator (for active connections) */}
      {/* {isSelected && <FlowIndicator path={path} />} */}
    </g>
  );
});

ConnectionLine.displayName = "ConnectionLine";

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export const ConnectionLines = memo(({
  connections,
  nodes,
  viewportX,
  viewportY,
  zoom,
  selectedConnectionId,
  onConnectionClick,
  onConnectionDoubleClick,
  onConnectionRightClick,
}: ConnectionLinesProps) => {
  // Create a map of nodes for quick lookup
  const nodeMap = useMemo(() => {
    const map = new Map<string, BaseNode>();
    nodes.forEach((node) => map.set(node.id, node));
    return map;
  }, [nodes]);

  // Filter connections to only those where both nodes exist
  const validConnections = useMemo(() => {
    return connections.filter((conn) => {
      const hasSource = nodeMap.has(conn.sourceNodeId);
      const hasTarget = nodeMap.has(conn.targetNodeId);
      return hasSource && hasTarget;
    });
  }, [connections, nodeMap]);

  if (validConnections.length === 0) {
    return null;
  }

  return (
    <svg
      className="absolute inset-0 pointer-events-none"
      width="100%"
      height="100%"
      style={{ overflow: "visible" }}
    >
      {/* Define arrow marker */}
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
        <marker
          id="arrowhead-selected"
          markerWidth="10"
          markerHeight="7"
          refX="9"
          refY="3.5"
          orient="auto"
        >
          <polygon
            points="0 0, 10 3.5, 0 7"
            fill={CONNECTION_CONFIG.COLOR_SELECTED}
          />
        </marker>
      </defs>

      {validConnections.map((connection) => {
        const sourceNode = nodeMap.get(connection.sourceNodeId)!;
        const targetNode = nodeMap.get(connection.targetNodeId)!;

        return (
          <ConnectionLine
            key={connection.id}
            connection={connection}
            sourceNode={sourceNode}
            targetNode={targetNode}
            isSelected={selectedConnectionId === connection.id}
            viewportX={viewportX}
            viewportY={viewportY}
            zoom={zoom}
            onClick={() => onConnectionClick?.(connection.id)}
            onDoubleClick={() => onConnectionDoubleClick?.(connection.id)}
            onRightClick={(e) => onConnectionRightClick?.(connection.id, e)}
          />
        );
      })}
    </svg>
  );
});

ConnectionLines.displayName = "ConnectionLines";

// -----------------------------------------------------------------------------
// Connection Creation Line (when dragging to create connection)
// -----------------------------------------------------------------------------

interface ConnectionCreationLineProps {
  startX: number;
  startY: number;
  currentX: number;
  currentY: number;
  viewportX: number;
  viewportY: number;
  zoom: number;
}

export const ConnectionCreationLine = memo(({
  startX,
  startY,
  currentX,
  currentY,
  viewportX,
  viewportY,
  zoom,
}: ConnectionCreationLineProps) => {
  // Transform coordinates
  const screenStartX = startX * zoom + viewportX;
  const screenStartY = startY * zoom + viewportY;
  const screenEndX = currentX * zoom + viewportX;
  const screenEndY = currentY * zoom + viewportY;

  const path = calculateBezierPath(
    screenStartX,
    screenStartY,
    screenEndX,
    screenEndY,
    0.4 * zoom
  );

  return (
    <svg
      className="absolute inset-0 pointer-events-none"
      width="100%"
      height="100%"
      style={{ overflow: "visible" }}
    >
      <path
        d={path}
        fill="none"
        stroke={CONNECTION_CONFIG.COLOR_ACTIVE}
        strokeWidth={CONNECTION_CONFIG.LINE_WIDTH}
        strokeDasharray={`${5 * zoom},${5 * zoom}`}
        strokeLinecap="round"
        opacity={0.7}
      />

      {/* End point indicator */}
      <circle
        cx={screenEndX}
        cy={screenEndY}
        r={CONNECTION_CONFIG.FLOW_INDICATOR_SIZE * zoom}
        fill={CONNECTION_CONFIG.COLOR_ACTIVE}
        opacity={0.5}
      />
    </svg>
  );
});

ConnectionCreationLine.displayName = "ConnectionCreationLine";
