// ============================================================================
// ZERO IDE - CANVAS STATE HOOK
// Hook for managing canvas state with reducer
// ============================================================================

import { useReducer, useCallback, useEffect } from "react";
import type { CanvasState, CanvasAction, BaseNode, Connection, ValidationResult, OrchestratorConfig } from "../types";
import { CANVAS_CONFIG } from "../constants";
import { DEFAULT_ORCHESTRATOR_CONFIG } from "../types";

// -----------------------------------------------------------------------------
// Initial State
// -----------------------------------------------------------------------------

const createInitialState = (): CanvasState => ({
  nodes: [],
  connections: [],
  selectedNodeId: null,
  viewport: { ...CANVAS_CONFIG.DEFAULT_VIEWPORT },
  orchestratorConfig: DEFAULT_ORCHESTRATOR_CONFIG,
  validation: [],
});

// -----------------------------------------------------------------------------
// Reducer
// -----------------------------------------------------------------------------

function canvasReducer(state: CanvasState, action: CanvasAction): CanvasState {
  switch (action.type) {
    case "ADD_NODE": {
      return {
        ...state,
        nodes: [...state.nodes, action.node],
      };
    }

    case "DELETE_NODE": {
      const nodeId = action.id;
      return {
        ...state,
        nodes: state.nodes.filter((n) => n.id !== nodeId),
        // Remove connections associated with this node
        connections: state.connections.filter(
          (c) => c.sourceNodeId !== nodeId && c.targetNodeId !== nodeId
        ),
        // Clear selection if this node was selected
        selectedNodeId: state.selectedNodeId === nodeId ? null : state.selectedNodeId,
      };
    }

    case "UPDATE_NODE": {
      return {
        ...state,
        nodes: state.nodes.map((node) =>
          node.id === action.id
            ? { ...node, ...action.updates, lastModified: Date.now() }
            : node
        ),
      };
    }

    case "SELECT_NODE": {
      return {
        ...state,
        selectedNodeId: action.id,
      };
    }

    case "ADD_CONNECTION": {
      // Check for duplicates
      const exists = state.connections.some(
        (c) =>
          c.sourceNodeId === action.connection.sourceNodeId &&
          c.sourcePort === action.connection.sourcePort &&
          c.targetNodeId === action.connection.targetNodeId &&
          c.targetPort === action.connection.targetPort
      );

      if (exists) return state;

      return {
        ...state,
        connections: [...state.connections, action.connection],
      };
    }

    case "DELETE_CONNECTION": {
      return {
        ...state,
        connections: state.connections.filter((c) => c.id !== action.id),
      };
    }

    case "SET_VIEWPORT": {
      return {
        ...state,
        viewport: action.viewport,
      };
    }

    case "PAN_VIEWPORT": {
      const newZoom = state.viewport.zoom;
      return {
        ...state,
        viewport: {
          ...state.viewport,
          x: state.viewport.x + action.deltaX / newZoom,
          y: state.viewport.y + action.deltaY / newZoom,
        },
      };
    }

    case "ZOOM_VIEWPORT": {
      const newZoom = Math.min(
        Math.max(action.zoom, CANVAS_CONFIG.MIN_ZOOM),
        CANVAS_CONFIG.MAX_ZOOM
      );

      // If center point provided, adjust x/y to zoom toward that point
      let x = state.viewport.x;
      let y = state.viewport.y;

      if (action.centerX !== undefined && action.centerY !== undefined) {
        const zoomRatio = newZoom / state.viewport.zoom;
        x = action.centerX - (action.centerX - x) * zoomRatio;
        y = action.centerY - (action.centerY - y) * zoomRatio;
      }

      return {
        ...state,
        viewport: { x, y, zoom: newZoom },
      };
    }

    case "SET_VALIDATION": {
      return {
        ...state,
        validation: action.validation,
      };
    }

    case "UPDATE_ORCHESTRATOR": {
      return {
        ...state,
        orchestratorConfig: {
          ...state.orchestratorConfig,
          ...action.updates,
        },
      };
    }

    default:
      return state;
  }
}

// -----------------------------------------------------------------------------
// Hook
// -----------------------------------------------------------------------------

export function useCanvasState(initialState?: Partial<CanvasState>) {
  const [state, dispatch] = useReducer(
    canvasReducer,
    { ...createInitialState(), ...initialState },
    () => ({ ...createInitialState(), ...initialState })
  );

  // -----------------------------------------------------------------------------
  // Node Actions
  // -----------------------------------------------------------------------------

  const addNode = useCallback((node: BaseNode) => {
    dispatch({ type: "ADD_NODE", node });
  }, []);

  const deleteNode = useCallback((id: string) => {
    dispatch({ type: "DELETE_NODE", id });
  }, []);

  const updateNode = useCallback((id: string, updates: Partial<BaseNode>) => {
    dispatch({ type: "UPDATE_NODE", id, updates });
  }, []);

  const selectNode = useCallback((id: string | null) => {
    dispatch({ type: "SELECT_NODE", id });
  }, []);

  // -----------------------------------------------------------------------------
  // Connection Actions
  // -----------------------------------------------------------------------------

  const addConnection = useCallback((connection: Connection) => {
    dispatch({ type: "ADD_CONNECTION", connection });
  }, []);

  const deleteConnection = useCallback((id: string) => {
    dispatch({ type: "DELETE_CONNECTION", id });
  }, []);

  // -----------------------------------------------------------------------------
  // Viewport Actions
  // -----------------------------------------------------------------------------

  const setViewport = useCallback((viewport: { x: number; y: number; zoom: number }) => {
    dispatch({ type: "SET_VIEWPORT", viewport });
  }, []);

  const panViewport = useCallback((deltaX: number, deltaY: number) => {
    dispatch({ type: "PAN_VIEWPORT", deltaX, deltaY });
  }, []);

  const zoomViewport = useCallback((zoom: number, centerX?: number, centerY?: number) => {
    dispatch({ type: "ZOOM_VIEWPORT", zoom, centerX, centerY });
  }, []);

  const zoomIn = useCallback((centerX?: number, centerY?: number) => {
    dispatch({
      type: "ZOOM_VIEWPORT",
      zoom: state.viewport.zoom + CANVAS_CONFIG.ZOOM_STEP,
      centerX,
      centerY,
    });
  }, [state.viewport.zoom]);

  const zoomOut = useCallback((centerX?: number, centerY?: number) => {
    dispatch({
      type: "ZOOM_VIEWPORT",
      zoom: state.viewport.zoom - CANVAS_CONFIG.ZOOM_STEP,
      centerX,
      centerY,
    });
  }, [state.viewport.zoom]);

  const resetViewport = useCallback(() => {
    dispatch({ type: "SET_VIEWPORT", viewport: CANVAS_CONFIG.DEFAULT_VIEWPORT });
  }, []);

  // -----------------------------------------------------------------------------
  // Validation Actions
  // -----------------------------------------------------------------------------

  const setValidation = useCallback((validation: ValidationResult[]) => {
    dispatch({ type: "SET_VALIDATION", validation });
  }, []);

  // -----------------------------------------------------------------------------
  // Orchestrator Config Actions
  // -----------------------------------------------------------------------------

  const updateOrchestratorConfig = useCallback((updates: Partial<OrchestratorConfig>) => {
    dispatch({ type: "UPDATE_ORCHESTRATOR", updates });
  }, []);

  // -----------------------------------------------------------------------------
  // Utility Functions
  // -----------------------------------------------------------------------------

  const getNode = useCallback((id: string) => {
    return state.nodes.find((n) => n.id === id);
  }, [state.nodes]);

  const getNodeConnections = useCallback((nodeId: string) => {
    return state.connections.filter(
      (c) => c.sourceNodeId === nodeId || c.targetNodeId === nodeId
    );
  }, [state.connections]);

  const clearCanvas = useCallback(() => {
    dispatch({ type: "SELECT_NODE", id: null });
    dispatch({ type: "SET_VIEWPORT", viewport: CANVAS_CONFIG.DEFAULT_VIEWPORT });
  }, []);

  // -----------------------------------------------------------------------------
  // Keyboard Shortcuts
  // -----------------------------------------------------------------------------

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Ignore if typing in an input
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement
      ) {
        return;
      }

      // Delete selected node
      if (
        (e.key === "Delete" || e.key === "Backspace") &&
        state.selectedNodeId
      ) {
        e.preventDefault();
        deleteNode(state.selectedNodeId);
      }

      // Clear selection with Escape
      if (e.key === "Escape" && state.selectedNodeId) {
        e.preventDefault();
        selectNode(null);
      }

      // Zoom shortcuts
      if ((e.ctrlKey || e.metaKey) && e.key === "=") {
        e.preventDefault();
        zoomIn();
      }

      if ((e.ctrlKey || e.metaKey) && e.key === "-") {
        e.preventDefault();
        zoomOut();
      }

      if ((e.ctrlKey || e.metaKey) && e.key === "0") {
        e.preventDefault();
        resetViewport();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [state.selectedNodeId, deleteNode, selectNode, zoomIn, zoomOut, resetViewport]);

  return {
    state,

    // Node actions
    addNode,
    deleteNode,
    updateNode,
    selectNode,
    getNode,
    getNodeConnections,

    // Connection actions
    addConnection,
    deleteConnection,

    // Viewport actions
    setViewport,
    panViewport,
    zoomViewport,
    zoomIn,
    zoomOut,
    resetViewport,

    // Validation actions
    setValidation,

    // Orchestrator config actions
    updateOrchestratorConfig,

    // Utility
    clearCanvas,
  };
}
