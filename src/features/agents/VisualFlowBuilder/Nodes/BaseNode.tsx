// ============================================================================
// VISUAL FLOW BUILDER - BASE NODE
// Shared wrapper component for all node types
// ============================================================================

import React, { memo, useRef, useState, useCallback } from "react";
import type { NodeProps } from "../types";
import { NODE_COLORS, NODE_ICONS, NODE_DIMENSIONS, CANVAS_CONFIG } from "../constants";
import { NodeActions } from "./NodeActions";

// -----------------------------------------------------------------------------
// Icons (using Lucide React style components)
// -----------------------------------------------------------------------------

const Icons: Record<string, React.FC<{ className?: string }>> = {
  Play: ({ className }) => (
    <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
      <polygon points="5 3 19 12 5 21 5 3" />
    </svg>
  ),
  Circle: ({ className }) => (
    <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="10" />
    </svg>
  ),
  Bot: ({ className }) => (
    <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
      <path d="M12 8V4H8" /><rect width="16" height="12" x="4" y="8" rx="2" /><path d="M2 14h2" /><path d="M20 14h2" /><path d="M15 13v2" /><path d="M9 13v2" />
    </svg>
  ),
  Zap: ({ className }) => (
    <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
      <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
    </svg>
  ),
  ArrowRight: ({ className }) => (
    <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
      <path d="M5 12h14" /><path d="m12 5 7 7-7 7" />
    </svg>
  ),
  GitBranch: ({ className }) => (
    <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
      <path d="M6 3v12" /><circle cx="18" cy="6" r="3" /><circle cx="6" cy="18" r="3" /><path d="M18 9a9 9 0 0 1-9 9" />
    </svg>
  ),
  Repeat: ({ className }) => (
    <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
      <path d="m17 2 4 4-4 4" /><path d="M3 11V9a4 4 0 0 1 4-4h14" /><path d="m7 22-4-4 4-4" /><path d="M21 13v2a4 4 0 0 1-4 4H3" />
    </svg>
  ),
  Merge: ({ className }) => (
    <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
      <path d="m6 8 6 6-6 6" /><path d="m18 8-6 6 6 6" />
    </svg>
  ),
  ListChecks: ({ className }) => (
    <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
      <path d="M9 11 3 17l-2-2" /><path d="m21 9-5-5-5 5" /><path d="M11 14h10" /><path d="M11 18h7" />
    </svg>
  ),
};

// -----------------------------------------------------------------------------
// Port Component
// -----------------------------------------------------------------------------

interface PortProps {
  type: "input" | "output";
  nodeId: string;
  port: string;
  onClick?: (e: React.MouseEvent) => void;
  onHover?: (isHovering: boolean) => void;
  onMouseDown?: (e: React.MouseEvent) => void;
}

const Port = memo(({ type, nodeId, port, onClick, onHover, onMouseDown }: PortProps) => {
  return (
    <div
      className={`absolute top-1/2 -translate-y-1/2 w-3 h-3 rounded-full border-2 border-white/30 bg-white/80 hover:bg-violet-400 hover:border-violet-300 hover:scale-125 cursor-crosshair transition-all duration-150 ${
        type === "input" ? "-left-1.5" : "-right-1.5"
      }`}
      data-port="true"
      data-node-id={nodeId}
      data-port-type={type}
      data-port-position={type === "input" ? "left" : "right"}
      data-port-id={port}
      onClick={onClick}
      onMouseDown={onMouseDown}
      onMouseEnter={() => onHover?.(true)}
      onMouseLeave={() => onHover?.(false)}
    />
  );
});

Port.displayName = "Port";

// -----------------------------------------------------------------------------
// Base Node Component
// -----------------------------------------------------------------------------

export const BaseNode = memo(({
  node,
  isSelected,
  onSelect,
  onUpdate,
  onDelete,
  onPortMouseDown,
  children,
}: NodeProps & { children?: React.ReactNode }) => {
  const nodeRef = useRef<HTMLDivElement>(null);

  const [isDragging, setIsDragging] = useState(false);
  const [dragStartPos, setDragStartPos] = useState<{ x: number; y: number } | null>(null);
  const [initialNodePos, setInitialNodePos] = useState<{ x: number; y: number } | null>(null);

  // Get node styling based on type
  const nodeStyle = NODE_COLORS[node.type] || NODE_COLORS.subagent;
  const IconComponent = Icons[NODE_ICONS[node.type]] || Icons.Bot;

  // -----------------------------------------------------------------------------
  // Mouse event handlers
  // -----------------------------------------------------------------------------

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    // Only drag with left mouse button
    if (e.button !== 0) return;

    // Don't drag if clicking on ports
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

    // Snap to grid
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

  // Register global mouse events for dragging
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
    if (confirm(`Delete "${node.data.displayName || "Node"}"?`)) {
      onDelete();
    }
  }, [node.data.displayName, onDelete]);

  // Handle Delete key
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

  const nodeClass = `
    absolute rounded-xl cursor-pointer pointer-events-auto transition-all duration-200
    bg-gradient-to-br ${nodeStyle.bg}
    border ${nodeStyle.border}
    ${isSelected ? "ring-2 ring-violet-500 ring-offset-2 ring-offset-[#0d0d0d]" : ""}
    ${isDragging ? "scale-105 shadow-2xl shadow-violet-500/20" : "shadow-lg"}
    hover:shadow-xl hover:border-white/20
  `;

  return (
    <div
      ref={nodeRef}
      className={nodeClass}
      style={{
        left: node.position.x,
        top: node.position.y,
        width: NODE_DIMENSIONS.WIDTH,
        height: NODE_DIMENSIONS.HEIGHT,
      }}
      onMouseDown={handleMouseDown}
      onClick={(e) => {
        e.stopPropagation();
        onSelect();
      }}
    >
      {/* Input Port */}
      <Port
        type="input"
        nodeId={node.id}
        port="input"
        onMouseDown={(e) => {
          e.stopPropagation();
          onPortMouseDown?.(node.id, "input", "input", {
            x: node.position.x,
            y: node.position.y + NODE_DIMENSIONS.HEIGHT / 2,
          });
        }}
      />

      {/* Output Port */}
      <Port
        type="output"
        nodeId={node.id}
        port="output"
        onMouseDown={(e) => {
          e.stopPropagation();
          onPortMouseDown?.(node.id, "output", "output", {
            x: node.position.x + NODE_DIMENSIONS.WIDTH,
            y: node.position.y + NODE_DIMENSIONS.HEIGHT / 2,
          });
        }}
      />

      {/* Node Header */}
      <div
        className={`
          absolute top-0 left-0 right-0 h-[40px] rounded-t-xl
          flex items-center gap-2 px-3
          border-b border-white/10
          bg-black/20 backdrop-blur-sm
        `}
      >
        <div className={`p-1.5 rounded-lg bg-gradient-to-br from-white/10 to-white/5 ${nodeStyle.icon}`}>
          <IconComponent className="w-3.5 h-3.5" />
        </div>
        <span className="text-sm font-medium text-white truncate flex-1">
          {node.data.displayName || "Untitled Node"}
        </span>
        <span className={`text-[10px] px-1.5 py-0.5 rounded uppercase font-semibold ${nodeStyle.icon} bg-white/10`}>
          {node.type}
        </span>
      </div>

      {/* Node Body (custom content) */}
      <div className="absolute top-[40px] left-0 right-0 bottom-0 p-3">
        {children || (
          <div className="text-xs text-gray-400">
            {node.type === "start" && (
              <div className="space-y-1">
                <p className="text-green-400">Workflow starts here</p>
              </div>
            )}
            {node.type === "end" && (
              <div className="space-y-1">
                <p className="text-red-400">Workflow ends here</p>
              </div>
            )}
            {node.type === "subagent" && (
              <div className="space-y-1">
                <p className="text-indigo-400">Task definition</p>
              </div>
            )}
            {node.type === "conditional" && (
              <div className="space-y-1">
                <p className="text-pink-400">Route by condition</p>
              </div>
            )}
          </div>
        )}
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

BaseNode.displayName = "BaseNode";
