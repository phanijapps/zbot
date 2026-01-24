// ============================================================================
// VISUAL FLOW BUILDER - INFINITE CANVAS
// Main canvas component with pan/zoom, grid, nodes, and connections
// ============================================================================

import React, { memo, useRef, useCallback, useEffect } from "react";
import { GridPattern } from "./BackgroundGrid";
import { BaseNode } from "../Nodes/BaseNode";
import { AgentNode } from "../Nodes/AgentNode";
import { TriggerNode } from "../Nodes/TriggerNode";
import { ParallelNode } from "../Nodes/ParallelNode";
import { SequentialNode } from "../Nodes/SequentialNode";
import { ConditionalNode } from "../Nodes/ConditionalNode";
import { LoopNode } from "../Nodes/LoopNode";
import { AggregatorNode } from "../Nodes/AggregatorNode";
import { SubtaskNode } from "../Nodes/SubtaskNode";
import type { CanvasState, BaseNode as BaseNodeType, NodeType } from "../types";

interface InfiniteCanvasProps {
  state: CanvasState;
  addNode: (node: BaseNodeType) => void;
  deleteNode: (id: string) => void;
  updateNode: (id: string, updates: Partial<BaseNodeType>) => void;
  selectNode: (id: string | null) => void;
  setViewport: (viewport: { x: number; y: number; zoom: number }) => void;
  onAddNode?: (type: NodeType, position: { x: number; y: number }) => void;
  className?: string;
}

// -----------------------------------------------------------------------------
// Canvas Content Component
// -----------------------------------------------------------------------------

const CanvasContent = memo(({
  state,
  deleteNode,
  updateNode,
  selectNode,
  setViewport,
  onAddNode,
}: Omit<InfiniteCanvasProps, "className">) => {
  const containerRef = useRef<HTMLDivElement>(null);

  // Track space bar for panning
  const isSpacePressedRef = useRef(false);

  // -----------------------------------------------------------------------------
  // Mouse wheel handling for zoom
  // -----------------------------------------------------------------------------

  const handleWheel = useCallback((e: React.WheelEvent) => {
    if (!containerRef.current) return;

    const rect = containerRef.current.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    // Ctrl/Cmd + wheel = zoom
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      const zoomDelta = -Math.sign(e.deltaY) * 0.1;
      const newZoom = Math.max(0.25, Math.min(3, state.viewport.zoom + zoomDelta));

      // Calculate position to keep mouse point stable
      const mouseXInCanvas = (mouseX - state.viewport.x) / state.viewport.zoom;
      const mouseYInCanvas = (mouseY - state.viewport.y) / state.viewport.zoom;

      const newX = mouseX - mouseXInCanvas * newZoom;
      const newY = mouseY - mouseYInCanvas * newZoom;

      setViewport({
        x: newX,
        y: newY,
        zoom: newZoom,
      });
    } else {
      // Regular wheel = pan
      e.preventDefault();
      setViewport({
        ...state.viewport,
        x: state.viewport.x + (-e.deltaX) / state.viewport.zoom,
        y: state.viewport.y + (-e.deltaY) / state.viewport.zoom,
      });
    }
  }, [state.viewport.zoom, state.viewport, setViewport]);

  // -----------------------------------------------------------------------------
  // Mouse drag for panning (space + drag or middle click)
  // -----------------------------------------------------------------------------

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    // Middle click or space + left click = pan
    if (e.button === 1 || (e.button === 0 && isSpacePressedRef.current)) {
      e.preventDefault();
      const startX = e.clientX;
      const startY = e.clientY;

      const handleMouseMove = (moveEvent: MouseEvent) => {
        setViewport({
          ...state.viewport,
          x: state.viewport.x + (moveEvent.clientX - startX) / state.viewport.zoom,
          y: state.viewport.y + (moveEvent.clientY - startY) / state.viewport.zoom,
        });
      };

      const handleMouseUp = () => {
        window.removeEventListener("mousemove", handleMouseMove);
        window.removeEventListener("mouseup", handleMouseUp);
      };

      window.addEventListener("mousemove", handleMouseMove);
      window.addEventListener("mouseup", handleMouseUp);
    }

    // Click on empty space = deselect
    if (e.button === 0 && (e.target as HTMLElement).dataset.canvas === "true") {
      selectNode(null);
    }
  }, [state.viewport, state.viewport.zoom, setViewport, selectNode]);

  // -----------------------------------------------------------------------------
  // Handle drop from sidebar
  // -----------------------------------------------------------------------------

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
  }, []);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();

    const nodeType = e.dataTransfer.getData("application/node-type");
    if (!nodeType || !onAddNode) return;

    const rect = containerRef.current!.getBoundingClientRect();
    const x = (e.clientX - rect.left - state.viewport.x) / state.viewport.zoom;
    const y = (e.clientY - rect.top - state.viewport.y) / state.viewport.zoom;

    onAddNode(nodeType as NodeType, { x, y });
  }, [onAddNode, state.viewport]);

  // -----------------------------------------------------------------------------
  // Keyboard handling
  // -----------------------------------------------------------------------------

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.code === "Space" && !isSpacePressedRef.current) {
        isSpacePressedRef.current = true;
        const el = containerRef.current as HTMLElement | null;
        if (el) el.style.cursor = "grab";
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      if (e.code === "Space") {
        isSpacePressedRef.current = false;
        const el = containerRef.current as HTMLElement | null;
        if (el) el.style.cursor = "default";
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  }, []);

  // -----------------------------------------------------------------------------
  // Render nodes
  // -----------------------------------------------------------------------------

  const renderNode = useCallback((nodeId: string) => {
    const node = state.nodes.find((n) => n.id === nodeId);
    if (!node) return null;

    const isSelected = state.selectedNodeId === nodeId;

    const handleUpdate = (updates: Partial<typeof node>) => {
      updateNode(nodeId, updates);
    };

    const handleDelete = () => {
      deleteNode(nodeId);
    };

    const handleSelect = () => {
      selectNode(nodeId);
    };

    // Render based on node type
    switch (node.type) {
      case "agent":
        return (
          <AgentNode
            key={nodeId}
            node={node}
            isSelected={isSelected}
            onSelect={handleSelect}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
          />
        );
      case "trigger":
        return (
          <TriggerNode
            key={nodeId}
            node={node}
            isSelected={isSelected}
            onSelect={handleSelect}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
          />
        );
      case "parallel":
        return (
          <ParallelNode
            key={nodeId}
            node={node}
            isSelected={isSelected}
            onSelect={handleSelect}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
          />
        );
      case "sequential":
        return (
          <SequentialNode
            key={nodeId}
            node={node}
            isSelected={isSelected}
            onSelect={handleSelect}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
          />
        );
      case "conditional":
        return (
          <ConditionalNode
            key={nodeId}
            node={node}
            isSelected={isSelected}
            onSelect={handleSelect}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
          />
        );
      case "loop":
        return (
          <LoopNode
            key={nodeId}
            node={node}
            isSelected={isSelected}
            onSelect={handleSelect}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
          />
        );
      case "aggregator":
        return (
          <AggregatorNode
            key={nodeId}
            node={node}
            isSelected={isSelected}
            onSelect={handleSelect}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
          />
        );
      case "subtask":
        return (
          <SubtaskNode
            key={nodeId}
            node={node}
            isSelected={isSelected}
            onSelect={handleSelect}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
          />
        );
      default:
        return (
          <BaseNode
            key={nodeId}
            node={node}
            isSelected={isSelected}
            onSelect={handleSelect}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
          />
        );
    }
  }, [state.nodes, state.selectedNodeId, updateNode, deleteNode, selectNode]);

  // -----------------------------------------------------------------------------
  // Render
  // -----------------------------------------------------------------------------

  return (
    <div
      ref={containerRef}
      className="relative w-full h-full overflow-hidden bg-[#0d0d0d]"
      onWheel={handleWheel}
      onMouseDown={handleMouseDown}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
      data-canvas="true"
    >
      {/* Background Grid */}
      <GridPattern
        x={state.viewport.x}
        y={state.viewport.y}
        zoom={state.viewport.zoom}
        width={containerRef.current?.clientWidth || 0}
        height={containerRef.current?.clientHeight || 0}
      />

      {/* SVG layer for connections */}
      <svg
        className="absolute inset-0 pointer-events-none"
        width="100%"
        height="100%"
        style={{ overflow: "visible" }}
      >
        <g
          transform={`translate(${state.viewport.x}, ${state.viewport.y}) scale(${state.viewport.zoom})`}
        >
          {/* Connections will be rendered here */}
        </g>
      </svg>

      {/* Nodes Layer */}
      <div
        className="absolute inset-0"
        style={{
          transform: `translate(${state.viewport.x}px, ${state.viewport.y}px) scale(${state.viewport.zoom})`,
          transformOrigin: "0 0",
        }}
      >
        {state.nodes.map((node) => renderNode(node.id))}
      </div>

      {/* Zoom indicator */}
      <div className="absolute bottom-4 right-4 flex items-center gap-2 px-3 py-1.5 bg-black/50 backdrop-blur-sm rounded-lg border border-white/10">
        <span className="text-xs text-gray-400">
          {Math.round(state.viewport.zoom * 100)}%
        </span>
      </div>

      {/* Canvas info hint */}
      {state.nodes.length === 0 && (
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
          <div className="text-center">
            <p className="text-lg font-medium text-gray-500 mb-2">
              Your canvas is empty
            </p>
            <p className="text-sm text-gray-600">
              Drag nodes from the sidebar to get started
            </p>
          </div>
        </div>
      )}
    </div>
  );
});

CanvasContent.displayName = "CanvasContent";

// -----------------------------------------------------------------------------
// Main Infinite Canvas Component
// -----------------------------------------------------------------------------

export const InfiniteCanvas = memo((props: InfiniteCanvasProps) => {
  return (
    <div className={`relative flex-1 overflow-hidden ${props.className || ""}`}>
      <CanvasContent {...props} />
    </div>
  );
});

InfiniteCanvas.displayName = "InfiniteCanvas";
