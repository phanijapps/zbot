// ============================================================================
// VISUAL FLOW BUILDER - FLOW BUILDER MODAL
// Full-screen modal wrapper for VisualFlowBuilder
// ============================================================================

import { memo, useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { BaseNode, CanvasState } from "./types";
import { ModalOverlay } from "@/shared/ui/modal-overlay";
import { VisualFlowBuilder } from "./index";
import type { Agent } from "@/shared/types";

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

export interface FlowBuilderModalProps {
  open: boolean;
  onClose: () => void;
  agent: Agent | null;
}

// -----------------------------------------------------------------------------
// Flow Builder Modal Component
// -----------------------------------------------------------------------------

export const FlowBuilderModal = memo(({ open, onClose, agent }: FlowBuilderModalProps) => {
  const [initialNodes, setInitialNodes] = useState<BaseNode[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [saveStatus, setSaveStatus] = useState<"idle" | "saving" | "saved" | "error">("idle");

  // Load existing flow when agent changes or modal opens
  useEffect(() => {
    if (!open || !agent) {
      setInitialNodes([]);
      return;
    }

    const loadFlow = async () => {
      setIsLoading(true);
      try {
        // Try to load existing flow configuration from agent
        const flowConfig = await invoke<string | null>("get_agent_flow_config", {
          agentId: agent.id,
        });

        if (flowConfig) {
          const parsed = JSON.parse(flowConfig) as { nodes: BaseNode[]; connections: CanvasState["connections"] };
          setInitialNodes(parsed.nodes || []);
        } else {
          // No existing flow - start fresh
          setInitialNodes([]);
        }
      } catch (error) {
        console.error("Failed to load flow config:", error);
        // Start with empty flow on error
        setInitialNodes([]);
      } finally {
        setIsLoading(false);
      }
    };

    loadFlow();
  }, [open, agent]);

  // Handle save
  const handleSave = useCallback(async (state: CanvasState) => {
    if (!agent) return;

    setSaveStatus("saving");
    try {
      const flowConfig = JSON.stringify({
        nodes: state.nodes,
        connections: state.connections,
        viewport: state.viewport,
      });

      await invoke("save_agent_flow_config", {
        agentId: agent.id,
        config: flowConfig,
      });

      setSaveStatus("saved");
      setTimeout(() => setSaveStatus("idle"), 2000);
    } catch (error) {
      console.error("Failed to save flow config:", error);
      setSaveStatus("error");
      setTimeout(() => setSaveStatus("idle"), 3000);
    }
  }, [agent]);

  // Handle close
  const handleClose = useCallback(() => {
    // Don't prevent close on unsaved changes - let user decide
    onClose();
  }, [onClose]);

  if (!agent) return null;

  return (
    <ModalOverlay
      open={open}
      onClose={handleClose}
      title="Visual Workflow Builder"
      subtitle={agent.displayName}
      closeOnEscape={true}
      closeOnBackdropClick={false}
      className="!p-0 !max-h-screen"
    >
      {isLoading ? (
        <div className="flex items-center justify-center h-full">
          <div className="text-center">
            <div className="w-12 h-12 border-4 border-violet-500 border-t-transparent rounded-full animate-spin mx-auto mb-4" />
            <p className="text-gray-400">Loading workflow...</p>
          </div>
        </div>
      ) : (
        <VisualFlowBuilder
          agentId={agent.id}
          onSave={handleSave}
          initialNodes={initialNodes}
          readOnly={false}
        />
      )}

      {/* Save Status Indicator */}
      {saveStatus !== "idle" && (
        <div className="absolute bottom-4 right-4 flex items-center gap-2 px-4 py-2 rounded-lg bg-[#141414] border border-white/10 shadow-xl z-10">
          {saveStatus === "saving" && (
            <>
              <div className="w-4 h-4 border-2 border-blue-500 border-t-transparent rounded-full animate-spin" />
              <span className="text-sm text-blue-400">Saving...</span>
            </>
          )}
          {saveStatus === "saved" && (
            <>
              <div className="w-4 h-4 text-green-400">
                <svg className="w-full h-full" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
                  <path d="M20 6 9 17l-5-5" />
                </svg>
              </div>
              <span className="text-sm text-green-400">Saved</span>
            </>
          )}
          {saveStatus === "error" && (
            <>
              <div className="w-4 h-4 text-red-400">
                <svg className="w-full h-full" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
                  <circle cx="12" cy="12" r="10" />
                  <path d="M12 8v4M12 16h.01" />
                </svg>
              </div>
              <span className="text-sm text-red-400">Save failed</span>
            </>
          )}
        </div>
      )}
    </ModalOverlay>
  );
});

FlowBuilderModal.displayName = "FlowBuilderModal";
