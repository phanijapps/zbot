// ============================================================================
// VISUAL FLOW BUILDER - AUTO-SAVE HOOK
// Hook for debounced auto-save functionality
// ============================================================================

import { useState, useRef, useCallback, useEffect } from "react";
import type { CanvasState } from "../types";
import { SAVE_CONFIG } from "../constants";

// -----------------------------------------------------------------------------
// Hook Return Type
// -----------------------------------------------------------------------------

interface UseAutoSaveReturn {
  saveStatus: "saved" | "saving" | "unsaved";
  scheduleSave: () => void;
  forceSave: () => Promise<void>;
  clearUnsavedStatus: () => void;
}

// -----------------------------------------------------------------------------
// Hook Options
// -----------------------------------------------------------------------------

interface UseAutoSaveOptions {
  debounceMs?: number;
  onSave?: (state: CanvasState) => Promise<void> | void;
  enabled?: boolean;
}

// -----------------------------------------------------------------------------
// Hook
// -----------------------------------------------------------------------------

export function useAutoSave(
  getState: () => CanvasState,
  options: UseAutoSaveOptions = {}
): UseAutoSaveReturn {
  const {
    debounceMs = SAVE_CONFIG.DEBOUNCE_MS,
    onSave,
    enabled = true,
  } = options;

  const [saveStatus, setSaveStatus] = useState<"saved" | "saving" | "unsaved">("saved");
  const saveTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isSavingRef = useRef(false);

  // -----------------------------------------------------------------------------
  // Clear pending save timeout
  // -----------------------------------------------------------------------------

  const clearSaveTimeout = useCallback(() => {
    if (saveTimeoutRef.current) {
      clearTimeout(saveTimeoutRef.current);
      saveTimeoutRef.current = null;
    }
  }, []);

  // -----------------------------------------------------------------------------
  // Perform the actual save
  // -----------------------------------------------------------------------------

  const performSave = useCallback(async () => {
    if (isSavingRef.current || !enabled) {
      return;
    }

    isSavingRef.current = true;
    setSaveStatus("saving");

    try {
      const state = getState();

      // Call custom save handler if provided
      if (onSave) {
        await onSave(state);
      } else {
        // Default: save to localStorage
        const saveData = {
          nodes: state.nodes,
          connections: state.connections,
          viewport: state.viewport,
          orchestratorConfig: state.orchestratorConfig,
          timestamp: Date.now(),
        };
        localStorage.setItem(SAVE_CONFIG.STORAGE_KEY, JSON.stringify(saveData));
      }

      setSaveStatus("saved");
    } catch (error) {
      console.error("Auto-save failed:", error);
      setSaveStatus("unsaved");
    } finally {
      isSavingRef.current = false;
    }
  }, [getState, onSave, enabled]);

  // -----------------------------------------------------------------------------
  // Schedule a save (debounced)
  // -----------------------------------------------------------------------------

  const scheduleSave = useCallback(() => {
    if (!enabled) {
      return;
    }

    // Mark as unsaved
    setSaveStatus("unsaved");

    // Clear any existing timeout
    clearSaveTimeout();

    // Schedule new save
    saveTimeoutRef.current = setTimeout(() => {
      performSave();
    }, debounceMs);
  }, [enabled, debounceMs, clearSaveTimeout, performSave]);

  // -----------------------------------------------------------------------------
  // Force an immediate save
  // -----------------------------------------------------------------------------

  const forceSave = useCallback(async () => {
    clearSaveTimeout();
    await performSave();
  }, [clearSaveTimeout, performSave]);

  // -----------------------------------------------------------------------------
  // Clear unsaved status (e.g., after loading from storage)
  // -----------------------------------------------------------------------------

  const clearUnsavedStatus = useCallback(() => {
    setSaveStatus("saved");
  }, []);

  // -----------------------------------------------------------------------------
  // Save on unmount (if unsaved)
  // -----------------------------------------------------------------------------

  useEffect(() => {
    return () => {
      if (saveStatus === "unsaved" && enabled) {
        performSave();
      }
    };
  }, [saveStatus, enabled, performSave]);

  // -----------------------------------------------------------------------------
  // Auto-save with keyboard shortcut (Ctrl+S / Cmd+S)
  // -----------------------------------------------------------------------------

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault();
        forceSave();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [forceSave]);

  return {
    saveStatus,
    scheduleSave,
    forceSave,
    clearUnsavedStatus,
  };
}

// -----------------------------------------------------------------------------
// Helper: Load saved state from localStorage
// -----------------------------------------------------------------------------

export function loadSavedState(): Partial<CanvasState> | null {
  try {
    const saved = localStorage.getItem(SAVE_CONFIG.STORAGE_KEY);
    if (!saved) {
      return null;
    }

    const data = JSON.parse(saved);
    return {
      nodes: data.nodes || [],
      connections: data.connections || [],
      viewport: data.viewport || { x: 0, y: 0, zoom: 1 },
      orchestratorConfig: data.orchestratorConfig,
    };
  } catch (error) {
    console.error("Failed to load saved state:", error);
    return null;
  }
}

// -----------------------------------------------------------------------------
// Helper: Clear saved state from localStorage
// -----------------------------------------------------------------------------

export function clearSavedState(): void {
  localStorage.removeItem(SAVE_CONFIG.STORAGE_KEY);
}

// -----------------------------------------------------------------------------
// Helper: Check if there's a saved state
// -----------------------------------------------------------------------------

export function hasSavedState(): boolean {
  return localStorage.getItem(SAVE_CONFIG.STORAGE_KEY) !== null;
}
