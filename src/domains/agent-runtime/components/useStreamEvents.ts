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
  AttachmentInfo,
  UseStreamEventsReturn,
} from "./types";

/**
 * Detect content type from file extension
 */
function detectContentType(filename: string): string {
  const ext = filename.split('.').pop()?.toLowerCase();
  switch (ext) {
    case "html": case "htm": return "html";
    case "pdf": return "pdf";
    case "md": case "markdown": return "markdown";
    case "png": case "jpg": case "jpeg": case "gif": case "svg": case "webp": return "image";
    case "txt": return "text";
    default: return "text";
  }
}

/**
 * Parse write tool result to extract attachment info
 */
function parseWriteAttachment(toolName: string, result: string): AttachmentInfo | null {
  if (toolName !== "write") return null;

  try {
    const parsed = JSON.parse(result);
    // WriteTool returns {path, bytes_written} - no success field needed
    if (!parsed.path) {
      console.warn("[parseWriteAttachment] No path in result:", result);
      return null;
    }

    const fullPath = parsed.path;
    const filename = fullPath.split('/').pop() || fullPath.split('\\').pop() || "file";
    const isOutput = fullPath.includes("/outputs/") || fullPath.includes("\\outputs\\");

    // Build relative path
    let relativePath: string;
    if (isOutput) {
      relativePath = `outputs/${filename}`;
    } else {
      // Extract conv_id/attachments/filename from full path
      const parts = fullPath.split('/');
      const attachmentsIdx = parts.indexOf('attachments');
      if (attachmentsIdx > 0 && attachmentsIdx + 1 < parts.length) {
        const convId = parts[attachmentsIdx - 1];
        relativePath = `${convId}/attachments/${filename}`;
      } else {
        relativePath = filename;
      }
    }

    return {
      filename,
      fullPath,
      relativePath,
      contentType: detectContentType(filename),
      size: parsed.bytes_written || 0,
      isOutput,
    };
  } catch {
    return null;
  }
}

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
    attachments: [],
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
      attachments: [],
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
      attachments: [],
    }));
  }, []);

  /**
   * Handle a single stream event
   */
  const handleEvent = useCallback(
    (event: AgentStreamEvent) => {
      setState((prev) => {
        console.log("[useStreamEvents] Processing event:", event.type, "Current state:", { isOpen: prev.isOpen, isActive: prev.isActive });

        switch (event.type) {
          case "metadata":
            // Agent started working - auto-open panel
            console.log("[useStreamEvents] Metadata event, opening panel");
            return {
              ...prev,
              isActive: true,
              isOpen: true, // Always open on metadata
              currentMessageId: event.agentId,
            };

          case "token":
            // Still active - also ensure panel is open
            console.log("[useStreamEvents] Token event, ensuring panel is open");
            return {
              ...prev,
              isActive: true,
              isOpen: true, // Keep panel open during streaming
            };

          case "reasoning":
            // Add to reasoning blocks
            return {
              ...prev,
              reasoning: [...prev.reasoning, event.content],
            };

          case "tool_call_start":
            // New tool call starting - ensure panel is open
            console.log("🔧 Tool call:", event.toolName);
            const newTool: ToolCallDisplay = {
              id: event.toolId,
              name: event.toolName,
              status: "running",
            };
            return {
              ...prev,
              isActive: true,
              isOpen: true, // Ensure panel is open when tools are running
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

          case "tool_result": {
            // Tool execution finished - check if it's a write tool that created an attachment
            const toolCall = prev.toolCalls.find(t => t.id === event.toolId);
            const attachment = toolCall && !event.error
              ? parseWriteAttachment(toolCall.name, event.result)
              : null;

            if (attachment) {
              console.log("📎 Attachment created:", attachment.filename);
            }

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
              ...(attachment ? { attachments: [...prev.attachments, attachment] } : {}),
            };
          }

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

          case "show_content":
            // Show content in canvas - this is handled at a higher level
            // Just log it for now
            console.log("[useStreamEvents] Show content event:", event.contentType, event.title);
            return prev;

          case "request_input":
            // Request input via form - this is handled at a higher level
            // Just log it for now
            console.log("[useStreamEvents] Request input event:", event.formId, event.title);
            return prev;

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
