import { create } from 'zustand';
import { devtools } from 'zustand/middleware';
import type { Node, Edge } from '@xyflow/react';
import type { OrchestratorConfig } from '@/services/workflow';

// ============================================================================
// HISTORY STATE TYPE
// ============================================================================

interface HistoryState {
  nodes: Node[];
  edges: Edge[];
  orchestratorConfig?: OrchestratorConfig;
  timestamp: number;
}

interface WorkflowHistoryState {
  // History stacks
  past: HistoryState[];
  present: HistoryState | null;
  future: HistoryState[];

  // Limits
  maxHistorySize: number;

  // Actions
  pushState: (nodes: Node[], edges: Edge[], orchestratorConfig?: OrchestratorConfig) => void;
  undo: () => { nodes: Node[]; edges: Edge[]; orchestratorConfig?: OrchestratorConfig } | null;
  redo: () => { nodes: Node[]; edges: Edge[]; orchestratorConfig?: OrchestratorConfig } | null;
  canUndo: () => boolean;
  canRedo: () => boolean;
  clear: () => void;
}

// ============================================================================
// HISTORY STORE
// ============================================================================

export const useWorkflowHistory = create<WorkflowHistoryState>()(
  devtools(
    (set, get) => ({
      past: [],
      present: null,
      future: [],
      maxHistorySize: 50,

      pushState: (nodes, edges, orchestratorConfig) => {
        const state = get();
        const currentState = state.present;

        // Create new state
        const newPresent: HistoryState = {
          nodes: JSON.parse(JSON.stringify(nodes)),
          edges: JSON.parse(JSON.stringify(edges)),
          orchestratorConfig: orchestratorConfig
            ? JSON.parse(JSON.stringify(orchestratorConfig))
            : undefined,
          timestamp: Date.now(),
        };

        // If current state is different from new state, push to past
        if (currentState) {
          const isNodesEqual =
            JSON.stringify(currentState.nodes) === JSON.stringify(nodes);
          const isEdgesEqual =
            JSON.stringify(currentState.edges) === JSON.stringify(edges);
          const isOrchestratorEqual =
            JSON.stringify(currentState.orchestratorConfig) ===
            JSON.stringify(orchestratorConfig);

          // Only push if state actually changed
          if (!isNodesEqual || !isEdgesEqual || !isOrchestratorEqual) {
            const newPast = [...state.past, currentState];

            // Limit history size
            if (newPast.length > state.maxHistorySize) {
              newPast.shift();
            }

            set({
              past: newPast,
              present: newPresent,
              future: [], // Clear future when new action is performed
            });
          }
        } else {
          // Initial state
          set({
            present: newPresent,
          });
        }
      },

      undo: () => {
        const state = get();
        if (state.past.length === 0) return null;

        const previous = state.past[state.past.length - 1];
        const newPast = state.past.slice(0, -1);
        const newFuture = state.present
          ? [state.present, ...state.future]
          : state.future;

        set({
          past: newPast,
          present: previous,
          future: newFuture,
        });

        return {
          nodes: JSON.parse(JSON.stringify(previous.nodes)),
          edges: JSON.parse(JSON.stringify(previous.edges)),
          orchestratorConfig: previous.orchestratorConfig
            ? JSON.parse(JSON.stringify(previous.orchestratorConfig))
            : undefined,
        };
      },

      redo: () => {
        const state = get();
        if (state.future.length === 0) return null;

        const next = state.future[0];
        const newPast = state.present
          ? [...state.past, state.present]
          : state.past;
        const newFuture = state.future.slice(1);

        set({
          past: newPast,
          present: next,
          future: newFuture,
        });

        return {
          nodes: JSON.parse(JSON.stringify(next.nodes)),
          edges: JSON.parse(JSON.stringify(next.edges)),
          orchestratorConfig: next.orchestratorConfig
            ? JSON.parse(JSON.stringify(next.orchestratorConfig))
            : undefined,
        };
      },

      canUndo: () => {
        const state = get();
        return state.past.length > 0;
      },

      canRedo: () => {
        const state = get();
        return state.future.length > 0;
      },

      clear: () => {
        set({
          past: [],
          present: null,
          future: [],
        });
      },
    }),
    { name: 'WorkflowHistory' }
  )
);

// ============================================================================
// HOOK FOR INTEGRATED UNDO/REDO
// ============================================================================

/**
 * Hook to integrate history with workflow store
 * Call this when loading a workflow to initialize history
 */
export function initializeHistory(
  nodes: Node[],
  edges: Edge[],
  orchestratorConfig?: OrchestratorConfig
) {
  useWorkflowHistory.getState().clear();
  useWorkflowHistory.getState().pushState(nodes, edges, orchestratorConfig);
}

/**
 * Hook to save current state to history
 * Call this after any modifying action
 */
export function saveToHistory(
  nodes: Node[],
  edges: Edge[],
  orchestratorConfig?: OrchestratorConfig
) {
  useWorkflowHistory.getState().pushState(nodes, edges, orchestratorConfig);
}
