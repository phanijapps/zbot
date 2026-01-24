// ============================================================================
// VISUAL FLOW BUILDER - CANVAS PAN/ZOOM HOOK
// Hook for handling canvas panning and zooming
// ============================================================================

import { useState, useCallback, useRef, useEffect } from "react";
import type { Viewport } from "../types";
import { CANVAS_CONFIG } from "../constants";

// -----------------------------------------------------------------------------
// Hook Return Type
// -----------------------------------------------------------------------------

interface UseCanvasPanZoomReturn {
  viewport: Viewport;
  isPanning: boolean;
  panStart: (startX: number, startY: number) => void;
  panMove: (currentX: number, currentY: number) => void;
  panEnd: () => void;
  zoom: (delta: number, centerX: number, centerY: number) => void;
  zoomIn: (centerX?: number, centerY?: number) => void;
  zoomOut: (centerX?: number, centerY?: number) => void;
  reset: () => void;
  screenToCanvas: (screenX: number, screenY: number) => { x: number; y: number };
  canvasToScreen: (canvasX: number, canvasY: number) => { x: number; y: number };
}

// -----------------------------------------------------------------------------
// Hook Options
// -----------------------------------------------------------------------------

interface UseCanvasPanZoomOptions {
  initialViewport?: Partial<Viewport>;
  onViewportChange?: (viewport: Viewport) => void;
  enableWheelZoom?: boolean;
  enableMiddleClickPan?: boolean;
}

// -----------------------------------------------------------------------------
// Hook
// -----------------------------------------------------------------------------

export function useCanvasPanZoom(
  containerRef: React.RefObject<HTMLElement>,
  options: UseCanvasPanZoomOptions = {}
): UseCanvasPanZoomReturn {
  const {
    initialViewport,
    onViewportChange,
    enableWheelZoom = true,
    enableMiddleClickPan = true,
  } = options;

  const [viewport, setViewport] = useState<Viewport>(() => ({
    ...CANVAS_CONFIG.DEFAULT_VIEWPORT,
    ...initialViewport,
  }));

  const [isPanning, setIsPanning] = useState(false);

  // Refs to track pan state
  const panStateRef = useRef<{
    startX: number;
    startY: number;
    initialViewportX: number;
    initialViewportY: number;
  } | null>(null);

  // -----------------------------------------------------------------------------
  // Update viewport with callback
  // -----------------------------------------------------------------------------

  const updateViewport = useCallback(
    (newViewport: Viewport) => {
      setViewport(newViewport);
      onViewportChange?.(newViewport);
    },
    [onViewportChange]
  );

  // -----------------------------------------------------------------------------
  // Pan operations
  // -----------------------------------------------------------------------------

  const panStart = useCallback((startX: number, startY: number) => {
    setIsPanning(true);
    panStateRef.current = {
      startX,
      startY,
      initialViewportX: viewport.x,
      initialViewportY: viewport.y,
    };
  }, [viewport.x, viewport.y]);

  const panMove = useCallback((currentX: number, currentY: number) => {
    if (!panStateRef.current) return;

    const deltaX = currentX - panStateRef.current.startX;
    const deltaY = currentY - panStateRef.current.startY;

    // Adjust for zoom level
    const adjustedDeltaX = deltaX / viewport.zoom;
    const adjustedDeltaY = deltaY / viewport.zoom;

    updateViewport({
      ...viewport,
      x: panStateRef.current.initialViewportX + adjustedDeltaX,
      y: panStateRef.current.initialViewportY + adjustedDeltaY,
    });
  }, [viewport, updateViewport]);

  const panEnd = useCallback(() => {
    setIsPanning(false);
    panStateRef.current = null;
  }, []);

  // -----------------------------------------------------------------------------
  // Zoom operations
  // -----------------------------------------------------------------------------

  const zoom = useCallback((delta: number, centerX: number, centerY: number) => {
    const newZoom = Math.min(
      Math.max(viewport.zoom + delta, CANVAS_CONFIG.MIN_ZOOM),
      CANVAS_CONFIG.MAX_ZOOM
    );

    if (newZoom === viewport.zoom) return;

    // Calculate canvas position of the zoom center point
    const canvasX = (centerX - viewport.x) / viewport.zoom;
    const canvasY = (centerY - viewport.y) / viewport.zoom;

    // Calculate new viewport position to keep the center point stable
    const newX = centerX - canvasX * newZoom;
    const newY = centerY - canvasY * newZoom;

    updateViewport({
      x: newX,
      y: newY,
      zoom: newZoom,
    });
  }, [viewport, updateViewport]);

  const zoomIn = useCallback((centerX?: number, centerY?: number) => {
    const cx = centerX ?? (containerRef.current?.clientWidth ?? 0) / 2;
    const cy = centerY ?? (containerRef.current?.clientHeight ?? 0) / 2;
    zoom(CANVAS_CONFIG.ZOOM_STEP, cx, cy);
  }, [zoom, containerRef]);

  const zoomOut = useCallback((centerX?: number, centerY?: number) => {
    const cx = centerX ?? (containerRef.current?.clientWidth ?? 0) / 2;
    const cy = centerY ?? (containerRef.current?.clientHeight ?? 0) / 2;
    zoom(-CANVAS_CONFIG.ZOOM_STEP, cx, cy);
  }, [zoom, containerRef]);

  // -----------------------------------------------------------------------------
  // Reset viewport
  // -----------------------------------------------------------------------------

  const reset = useCallback(() => {
    updateViewport({
      ...CANVAS_CONFIG.DEFAULT_VIEWPORT,
      ...initialViewport,
    });
  }, [updateViewport, initialViewport]);

  // -----------------------------------------------------------------------------
  // Coordinate transformations
  // -----------------------------------------------------------------------------

  const screenToCanvas = useCallback((screenX: number, screenY: number) => {
    return {
      x: (screenX - viewport.x) / viewport.zoom,
      y: (screenY - viewport.y) / viewport.zoom,
    };
  }, [viewport]);

  const canvasToScreen = useCallback((canvasX: number, canvasY: number) => {
    return {
      x: canvasX * viewport.zoom + viewport.x,
      y: canvasY * viewport.zoom + viewport.y,
    };
  }, [viewport]);

  // -----------------------------------------------------------------------------
  // Mouse wheel zoom
  // -----------------------------------------------------------------------------

  useEffect(() => {
    if (!enableWheelZoom || !containerRef.current) return;

    const container = containerRef.current;

    const handleWheel = (e: WheelEvent) => {
      // Only zoom if Ctrl/Cmd key is pressed
      if (!(e.ctrlKey || e.metaKey)) {
        // Otherwise, pan with the wheel
        panMove(e.clientX, e.clientY);
        return;
      }

      e.preventDefault();

      const rect = container.getBoundingClientRect();
      const centerX = e.clientX - rect.left;
      const centerY = e.clientY - rect.top;

      // Calculate zoom delta based on wheel delta
      const delta = -Math.sign(e.deltaY) * CANVAS_CONFIG.ZOOM_STEP;
      zoom(delta, centerX, centerY);
    };

    container.addEventListener("wheel", handleWheel, { passive: false });
    return () => container.removeEventListener("wheel", handleWheel);
  }, [enableWheelZoom, panMove, zoom]);

  // -----------------------------------------------------------------------------
  // Middle-click panning
  // -----------------------------------------------------------------------------

  useEffect(() => {
    if (!enableMiddleClickPan || !containerRef.current) return;

    const container = containerRef.current;

    const handleMouseDown = (e: MouseEvent) => {
      if (e.button === 1) { // Middle mouse button
        e.preventDefault();
        const rect = container.getBoundingClientRect();
        panStart(
          e.clientX - rect.left,
          e.clientY - rect.top
        );
      }
    };

    const handleMouseMove = (e: MouseEvent) => {
      if (isPanning) {
        const rect = container.getBoundingClientRect();
        panMove(
          e.clientX - rect.left,
          e.clientY - rect.top
        );
      }
    };

    const handleMouseUp = (e: MouseEvent) => {
      if (e.button === 1) {
        panEnd();
      }
    };

    container.addEventListener("mousedown", handleMouseDown);
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);

    return () => {
      container.removeEventListener("mousedown", handleMouseDown);
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [enableMiddleClickPan, isPanning, panStart, panMove, panEnd]);

  return {
    viewport,
    isPanning,
    panStart,
    panMove,
    panEnd,
    zoom,
    zoomIn,
    zoomOut,
    reset,
    screenToCanvas,
    canvasToScreen,
  };
}

// -----------------------------------------------------------------------------
// Helper: Clamp value between min and max
// -----------------------------------------------------------------------------

export function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

// -----------------------------------------------------------------------------
// Helper: Lerp between two values
// -----------------------------------------------------------------------------

export function lerp(a: number, b: number, t: number): number {
  return a + (b - a) * t;
}
