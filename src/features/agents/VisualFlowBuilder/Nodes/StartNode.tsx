// ============================================================================
// ZERO IDE - START EVENT NODE
// BPMN-style Start Event (thin circle)
// ============================================================================

import React, { memo, useRef, useState, useCallback } from "react";
import type { NodeProps } from "../types";
import { NODE_COLORS, CANVAS_CONFIG } from "../constants";
import { NodeActions } from "./NodeActions";

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const PlayIcon = ({ className }: { className?: string }) => (
  <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <polygon points="5 3 19 12 5 21 5 3" />
  </svg>
);

// -----------------------------------------------------------------------------
// Port Component (for BPMN events, placed at bottom for output)
// -----------------------------------------------------------------------------

interface PortProps {
  type: "input" | "output";
  nodeId: string;
  port: string;
  onHover?: (isHovering: boolean) => void;
  onMouseDown?: (e: React.MouseEvent) => void;
}

const EventPort = memo(({ type, nodeId, port, onHover, onMouseDown }: PortProps) => {
  return (
    <div
      className={`absolute right-0 top-1/2 -translate-y-1/2 w-3 h-3 rounded-full border-2 border-white/30 bg-white/80 hover:bg-green-400 hover:border-green-300 hover:scale-125 cursor-crosshair transition-all duration-150 ${
        type === "input" ? "mr-[-6px]" : "mr-[-6px]"
      }`}
      data-port="true"
      data-node-id={nodeId}
      data-port-type={type}
      data-port-position={type === "input" ? "left" : "right"}
      data-port-id={port}
      onMouseDown={onMouseDown}
      onMouseEnter={() => onHover?.(true)}
      onMouseLeave={() => onHover?.(false)}
    />
  );
});

EventPort.displayName = "EventPort";

// -----------------------------------------------------------------------------
// Start Event Node Component (BPMN thin circle)
// -----------------------------------------------------------------------------

export const StartNode = memo<NodeProps>(({
  node,
  isSelected,
  onSelect,
  onUpdate,
  onDelete,
  onPortMouseDown,
}) => {
  const nodeRef = useRef<HTMLDivElement>(null);

  const [isDragging, setIsDragging] = useState(false);
  const [dragStartPos, setDragStartPos] = useState<{ x: number; y: number } | null>(null);
  const [initialNodePos, setInitialNodePos] = useState<{ x: number; y: number } | null>(null);

  const nodeStyle = NODE_COLORS[node.type] || NODE_COLORS.start;

  // -----------------------------------------------------------------------------
  // Mouse event handlers
  // -----------------------------------------------------------------------------

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).dataset.port === "true") return;

    e.stopPropagation();
    onSelect();

    setIsDragging(true);
    setDragStartPos({ x: e.clientX, y: e.clientY });
    setInitialNodePos({ ...node.position });
  }, [node.position, onSelect]);

  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (!isDragging || !dragStartPos || !initialNodePos) return;

    const deltaX = e.clientX - dragStartPos.x;
    const deltaY = e.clientY - dragStartPos.y;

    const newPos = {
      x: Math.round((initialNodePos.x + deltaX) / CANVAS_CONFIG.GRID_SIZE) * CANVAS_CONFIG.GRID_SIZE,
      y: Math.round((initialNodePos.y + deltaY) / CANVAS_CONFIG.GRID_SIZE) * CANVAS_CONFIG.GRID_SIZE,
    };

    onUpdate({ ...node, position: newPos });
  }, [isDragging, dragStartPos, initialNodePos, node, onUpdate]);

  const handleMouseUp = useCallback(() => {
    setIsDragging(false);
    setDragStartPos(null);
    setInitialNodePos(null);
  }, []);

  React.useEffect(() => {
    if (isDragging) {
      window.addEventListener("mousemove", handleMouseMove);
      window.addEventListener("mouseup", handleMouseUp);
      return () => {
        window.removeEventListener("mousemove", handleMouseMove);
        window.removeEventListener("mouseup", handleMouseUp);
      };
    }
  }, [isDragging, handleMouseMove, handleMouseUp]);

  // -----------------------------------------------------------------------------
  // Delete handler
  // -----------------------------------------------------------------------------

  const handleDelete = useCallback(() => {
    if (confirm(`Delete "${node.data.displayName || "Start"}" event?`)) {
      onDelete();
    }
  }, [node.data.displayName, onDelete]);

  React.useEffect(() => {
    if (!isSelected) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        handleDelete();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isSelected, handleDelete]);

  // -----------------------------------------------------------------------------
  // Render
  // -----------------------------------------------------------------------------

  return (
    <div
      ref={nodeRef}
      className={`absolute cursor-pointer pointer-events-auto transition-all duration-200 ${
        isSelected ? "ring-2 ring-green-500 ring-offset-2 ring-offset-[#0d0d0d] rounded-full" : ""
      } ${isDragging ? "scale-105" : ""}`}
      style={{
        left: node.position.x,
        top: node.position.y,
        width: 64,
        height: 64,
      }}
      onMouseDown={handleMouseDown}
      onClick={(e) => {
        e.stopPropagation();
        onSelect();
      }}
    >
      {/* BPMN Start Event - Thin Circle */}
      <div
        className={`w-full h-full rounded-full flex items-center justify-center bg-green-500/10 border-2 border-green-500 hover:bg-green-500/20 shadow-lg hover:shadow-xl hover:shadow-green-500/20 transition-all duration-200`}
      >
        <div className={`p-1.5 rounded-full bg-green-500/20 ${nodeStyle.icon}`}>
          <PlayIcon className="w-3 h-3 text-green-400" />
        </div>
      </div>

      {/* Output Port (only output for start event) */}
      <EventPort
        type="output"
        nodeId={node.id}
        port="output"
        onMouseDown={(e) => {
          e.stopPropagation();
          onPortMouseDown?.(node.id, "output", "output", {
            x: node.position.x + 64,
            y: node.position.y + 32,
          });
        }}
      />

      {/* Label below the circle */}
      <div className="absolute -bottom-6 left-1/2 -translate-x-1/2 whitespace-nowrap">
        <span className="text-xs font-medium text-green-400">{node.data.displayName || "Start"}</span>
      </div>

      {/* Node Actions (shown when selected) */}
      {isSelected && (
        <NodeActions
          onDelete={handleDelete}
          className="absolute -top-2 -right-2"
        />
      )}
    </div>
  );
});

StartNode.displayName = "StartNode";
