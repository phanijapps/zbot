// ============================================================================
// ZERO IDE - INFINITE CANVAS
// Main canvas component with pan/zoom, grid, nodes, and connections
// ============================================================================

import React, { memo, useRef, useCallback, useEffect, useState } from "react";
import { GridPattern } from "./BackgroundGrid";
import { ConnectionLines, ConnectionCreationLine } from "./ConnectionLines";
import { BaseNode } from "../Nodes/BaseNode";
import { StartNode } from "../Nodes/StartNode";
import { EndNode } from "../Nodes/EndNode";
import { SubagentNode } from "../Nodes/SubagentNode";
import { ConditionalNode } from "../Nodes/ConditionalNode";
import type { CanvasState, BaseNode as BaseNodeType, NodeType, Connection } from "../types";

interface InfiniteCanvasProps {
  state: CanvasState;
  addNode: (node: BaseNodeType) => void;
  deleteNode: (id: string) => void;
  updateNode: (id: string, updates: Partial<BaseNodeType>) => void;
  selectNode: (id: string | null) => void;
  setViewport: (viewport: { x: number; y: number; zoom: number }) => void;
  addConnection: (connection: Connection) => void;
  deleteConnection: (id: string) => void;
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
  addConnection,
  deleteConnection,
  onAddNode,
}: Omit<InfiniteCanvasProps, "className">) => {
  const containerRef = useRef<HTMLDivElement>(null);

  // Track space bar for panning
  const isSpacePressedRef = useRef(false);

  // -----------------------------------------------------------------------------
  // Connection Creation State
  // -----------------------------------------------------------------------------

  const [connectionCreation, setConnectionCreation] = useState<{
    isCreating: boolean;
    sourceNodeId: string | null;
    sourcePort: string | null;
    startX: number;
    startY: number;
    currentX: number;
    currentY: number;
    connectionsToReplace: string[]; // IDs of connections to replace when rerouting
  }>({
    isCreating: false,
    sourceNodeId: null,
    sourcePort: null,
    startX: 0,
    startY: 0,
    currentX: 0,
    currentY: 0,
    connectionsToReplace: [],
  });

  const [selectedConnectionId, setSelectedConnectionId] = useState<string | null>(null);

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
  // Connection Creation Handlers
  // -----------------------------------------------------------------------------

  // Start creating a connection from a port
  const handlePortMouseDown = useCallback((
    nodeId: string,
    port: string,
    portType: "input" | "output",
    position: { x: number; y: number }
  ) => {
    // Only allow starting from output ports
    if (portType !== "output") return;

    const startX = position.x;
    const startY = position.y;

    // Find existing connections from this output port (for rerouting)
    const existingConnections = state.connections.filter(
      c => c.sourceNodeId === nodeId && c.sourcePort === port
    );
    const connectionsToReplace = existingConnections.map(c => c.id);

    setConnectionCreation({
      isCreating: true,
      sourceNodeId: nodeId,
      sourcePort: port,
      startX,
      startY,
      currentX: startX,
      currentY: startY,
      connectionsToReplace,
    });
  }, [state.connections]);

  // Update connection creation line while dragging
  useEffect(() => {
    if (!connectionCreation.isCreating) return;

    const handleMouseMove = (e: MouseEvent) => {
      const rect = containerRef.current?.getBoundingClientRect();
      if (!rect) return;

      // Convert screen coordinates to canvas coordinates
      const x = (e.clientX - rect.left - state.viewport.x) / state.viewport.zoom;
      const y = (e.clientY - rect.top - state.viewport.y) / state.viewport.zoom;

      setConnectionCreation(prev => ({
        ...prev,
        currentX: x,
        currentY: y,
      }));
    };

    const handleMouseUp = (e: MouseEvent) => {
      // Check if we released over a port
      const target = e.target as HTMLElement;
      const portData = target.dataset;

      if (portData.port === "true" && portData.nodeId && portData.portType && portData.portPosition) {
        const targetNodeId = portData.nodeId;
        const targetPort = portData.port;
        const portType = portData.portType as "input" | "output";

        // Only connect output to input
        if (portType === "input" && connectionCreation.sourceNodeId) {
          // Delete old connections if rerouting
          if (connectionCreation.connectionsToReplace.length > 0) {
            connectionCreation.connectionsToReplace.forEach(connId => {
              deleteConnection(connId);
            });
          }

          // Create the new connection
          const connectionId = `conn-${connectionCreation.sourceNodeId}-${targetNodeId}-${Date.now()}`;
          addConnection({
            id: connectionId,
            sourceNodeId: connectionCreation.sourceNodeId,
            sourcePort: connectionCreation.sourcePort || "output",
            targetNodeId: targetNodeId,
            targetPort: targetPort,
          });
        }
      }

      // Reset connection creation state
      setConnectionCreation({
        isCreating: false,
        sourceNodeId: null,
        sourcePort: null,
        startX: 0,
        startY: 0,
        currentX: 0,
        currentY: 0,
        connectionsToReplace: [],
      });
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);

    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [connectionCreation.isCreating, connectionCreation.connectionsToReplace, state.viewport, containerRef, addConnection, deleteConnection]);

  // Connection click handlers
  const handleConnectionClick = useCallback((connectionId: string) => {
    setSelectedConnectionId(connectionId);
    selectNode(null); // Deselect any node
  }, [selectNode]);

  const handleConnectionRightClick = useCallback((connectionId: string, e: React.MouseEvent) => {
    e.preventDefault();
    if (confirm("Delete this connection?")) {
      deleteConnection(connectionId);
      setSelectedConnectionId(null);
    }
  }, [deleteConnection]);

  const handleConnectionDelete = useCallback((connectionId: string) => {
    deleteConnection(connectionId);
    setSelectedConnectionId(null);
  }, [deleteConnection]);

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
      case "start":
        return (
          <StartNode
            key={nodeId}
            node={node}
            isSelected={isSelected}
            onSelect={handleSelect}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
            onPortMouseDown={handlePortMouseDown}
          />
        );
      case "end":
        return (
          <EndNode
            key={nodeId}
            node={node}
            isSelected={isSelected}
            onSelect={handleSelect}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
            onPortMouseDown={handlePortMouseDown}
          />
        );
      case "subagent":
        return (
          <SubagentNode
            key={nodeId}
            node={node}
            isSelected={isSelected}
            onSelect={handleSelect}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
            onPortMouseDown={handlePortMouseDown}
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
            onPortMouseDown={handlePortMouseDown}
          />
        );
      default:
        // Fallback for unknown/deprecated node types
        return (
          <BaseNode
            key={nodeId}
            node={node}
            isSelected={isSelected}
            onSelect={handleSelect}
            onUpdate={handleUpdate}
            onDelete={handleDelete}
            onPortMouseDown={handlePortMouseDown}
          />
        );
    }
  }, [state.nodes, state.selectedNodeId, updateNode, deleteNode, selectNode, handlePortMouseDown]);

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

      {/* Connections Layer */}
      <ConnectionLines
        connections={state.connections}
        nodes={state.nodes}
        viewportX={state.viewport.x}
        viewportY={state.viewport.y}
        zoom={state.viewport.zoom}
        selectedConnectionId={selectedConnectionId}
        onConnectionClick={handleConnectionClick}
        onConnectionRightClick={handleConnectionRightClick}
        onConnectionDelete={handleConnectionDelete}
      />

      {/* Connection Creation Line (when dragging) */}
      {connectionCreation.isCreating && (
        <ConnectionCreationLine
          startX={connectionCreation.startX}
          startY={connectionCreation.startY}
          currentX={connectionCreation.currentX}
          currentY={connectionCreation.currentY}
          viewportX={state.viewport.x}
          viewportY={state.viewport.y}
          zoom={state.viewport.zoom}
        />
      )}

      {/* Nodes Layer */}
      <div
        className="absolute inset-0 pointer-events-none"
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
