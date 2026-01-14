/**
 * useStreamEvents - Hook for handling agent streaming events
 *
 * Processes AgentStreamEvents from the AgentExecutor and maintains
 * the ThinkingPanel state.
 *
 * Features:
 * - Auto-opens panel when agent starts working
 * - Auto-collapses when agent finishes
 * - Accumulates plan items, tool calls, and reasoning
 * - Only one panel open at a time (collapses old ones)
 */

import { useCallback, useState } from "react";
import type { AgentStreamEvent } from "@/shared/types/agent";
import type {
  ThinkingPanelState,
  PlanItem,
  ToolCallDisplay,
  UseStreamEventsReturn,
} from "./types";

/**
 * Hook for managing thinking panel state from stream events
 */
export function useStreamEvents(
  autoOpen = true,
  autoCollapse = true
): UseStreamEventsReturn {
  const [state, setState] = useState<ThinkingPanelState>({
    isOpen: false,
    isActive: false,
    hasPlan: false,
    planItems: [],
    toolCalls: [],
    reasoning: [],
    currentMessageId: null,
  });

  /**
   * Reset state for a new message
   */
  const reset = useCallback(() => {
    setState({
      isOpen: false,
      isActive: false,
      hasPlan: false,
      planItems: [],
      toolCalls: [],
      reasoning: [],
      currentMessageId: null,
    });
  }, []);

  /**
   * Set the current message ID
   */
  const setCurrentMessage = useCallback((messageId: string) => {
    setState((prev) => ({
      ...prev,
      currentMessageId: messageId,
      // Reset state for new message
      planItems: [],
      toolCalls: [],
      reasoning: [],
    }));
  }, []);

  /**
   * Handle a single stream event
   */
  const handleEvent = useCallback(
    (event: AgentStreamEvent) => {
      setState((prev) => {
        switch (event.type) {
          case "metadata":
            // Agent started working - auto-open panel
            return {
              ...prev,
              isActive: true,
              isOpen: autoOpen ? true : prev.isOpen,
              currentMessageId: event.agentId,
            };

          case "token":
            // Still active
            return { ...prev, isActive: true };

          case "reasoning":
            // Add to reasoning blocks
            return {
              ...prev,
              reasoning: [...prev.reasoning, event.content],
            };

          case "tool_call_start":
            // New tool call starting
            const newTool: ToolCallDisplay = {
              id: event.toolId,
              name: event.toolName,
              status: "running",
            };
            return {
              ...prev,
              toolCalls: [...prev.toolCalls, newTool],
            };

          case "tool_call_chunk":
            // Update existing tool call with partial args (for display)
            return {
              ...prev,
              toolCalls: prev.toolCalls.map((t) =>
                t.id === event.toolId
                  ? { ...t, status: "running" as const }
                  : t
              ),
            };

          case "tool_call_end":
            // Tool call complete with args
            return {
              ...prev,
              toolCalls: prev.toolCalls.map((t) =>
                t.id === event.toolId
                  ? {
                      ...t,
                      status: "completed" as const,
                      args: event.args,
                    }
                  : t
              ),
            };

          case "tool_result":
            // Tool execution finished
            return {
              ...prev,
              toolCalls: prev.toolCalls.map((t) =>
                t.id === event.toolId
                  ? {
                      ...t,
                      status: event.error ? "failed" : "completed",
                      result: event.result,
                      error: event.error,
                    }
                  : t
              ),
            };

          case "done":
            // Agent finished - auto-collapse if enabled
            return {
              ...prev,
              isActive: false,
              isOpen: autoCollapse ? false : prev.isOpen,
            };

          case "error":
            // Error occurred
            return {
              ...prev,
              isActive: false,
              isOpen: true, // Keep open to show error
            };

          default:
            return prev;
        }
      });
    },
    [autoOpen, autoCollapse]
  );

  /**
   * Manually toggle panel open/closed
   */
  const togglePanel = useCallback(() => {
    setState((prev) => ({ ...prev, isOpen: !prev.isOpen }));
  }, []);

  /**
   * Open panel (for clicking on historical "Used N tools" badge)
   */
  const openPanel = useCallback(() => {
    setState((prev) => ({ ...prev, isOpen: true }));
  }, []);

  /**
   * Close panel
   */
  const closePanel = useCallback(() => {
    setState((prev) => ({ ...prev, isOpen: false }));
  }, []);

  return {
    state,
    handleEvent,
    reset,
    setCurrentMessage,
    togglePanel,
    openPanel,
    closePanel,
  };
}

// ============================================================================
// PLAN MODULE INTEGRATION
// ============================================================================

/**
 * Hook for planning module integration
 * Call this when the planning module is active to set plan items
 */
export function usePlanItems() {
  const [planItems, setPlanItems] = useState<PlanItem[]>([]);

  const setPlan = useCallback((items: Omit<PlanItem, "id">[]) => {
    setPlanItems(
      items.map((item, index) => ({
        ...item,
        id: `plan-${index}-${Date.now()}`,
        order: index,
      }))
    );
  }, []);

  const updatePlanItem = useCallback(
    (index: number, status: PlanItem["status"]) => {
      setPlanItems((prev) =>
        prev.map((item, i) =>
          i === index ? { ...item, status } : item
        )
      );
    },
    []
  );

  const completePlanItem = useCallback((index: number) => {
    updatePlanItem(index, "completed");
  }, [updatePlanItem]);

  const failPlanItem = useCallback((index: number) => {
    updatePlanItem(index, "failed");
  }, [updatePlanItem]);

  return {
    planItems,
    setPlan,
    updatePlanItem,
    completePlanItem,
    failPlanItem,
  };
}
