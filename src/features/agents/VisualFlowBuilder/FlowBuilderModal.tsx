// ============================================================================
// ZERO IDE - FLOW BUILDER MODAL
// Full-screen modal wrapper for Zero IDE
// ============================================================================

import { memo, useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { BaseNode, CanvasState, OrchestratorConfig } from "./types";
import { ModalOverlay } from "@/shared/ui/modal-overlay";
import { ZeroIDE } from "./index";
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
  const [initialConnections, setInitialConnections] = useState<CanvasState["connections"]>([]);
  const [initialOrchestratorConfig, setInitialOrchestratorConfig] = useState<OrchestratorConfig | undefined>(undefined);
  const [isLoading, setIsLoading] = useState(false);

  // Load existing flow when agent changes or modal opens
  useEffect(() => {
    if (!open || !agent) {
      setInitialNodes([]);
      setInitialConnections([]);
      setInitialOrchestratorConfig(undefined);
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
          const parsed = JSON.parse(flowConfig) as {
            nodes: BaseNode[];
            connections: CanvasState["connections"];
            viewport?: { x: number; y: number; zoom: number };
            orchestratorConfig?: OrchestratorConfig;
          };
          setInitialNodes(parsed.nodes || []);
          setInitialConnections(parsed.connections || []);
          setInitialOrchestratorConfig(parsed.orchestratorConfig);
        } else {
          // No existing flow - start fresh
          setInitialNodes([]);
          setInitialConnections([]);
          setInitialOrchestratorConfig(undefined);
        }
      } catch (error) {
        console.error("Failed to load flow config:", error);
        // Start with empty flow on error
        setInitialNodes([]);
        setInitialConnections([]);
        setInitialOrchestratorConfig(undefined);
      } finally {
        setIsLoading(false);
      }
    };

    loadFlow();
  }, [open, agent]);

  // Handle save
  const handleSave = useCallback(async (state: CanvasState) => {
    if (!agent) return;

    try {
      const flowConfig = JSON.stringify({
        nodes: state.nodes,
        connections: state.connections,
        viewport: state.viewport,
        orchestratorConfig: state.orchestratorConfig,
      });

      await invoke("save_agent_flow_config", {
        agentId: agent.id,
        config: flowConfig,
      });
    } catch (error) {
      console.error("Failed to save flow config:", error);
      throw error;
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
      title=""
      closeOnEscape={true}
      closeOnBackdropClick={false}
      className="!p-0 !max-h-screen"
      showCloseButton={false}
      showHeader={false}
    >
      {isLoading ? (
        <div className="flex items-center justify-center h-full">
          <div className="text-center">
            <div className="w-12 h-12 border-4 border-violet-500 border-t-transparent rounded-full animate-spin mx-auto mb-4" />
            <p className="text-gray-400">Loading workflow...</p>
          </div>
        </div>
      ) : (
        <ZeroIDE
          agentId={agent.id}
          agentDisplayName={agent.displayName}
          onSave={handleSave}
          onClose={handleClose}
          initialNodes={initialNodes}
          initialConnections={initialConnections}
          initialOrchestratorConfig={initialOrchestratorConfig}
          readOnly={false}
        />
      )}
    </ModalOverlay>
  );
});

FlowBuilderModal.displayName = "FlowBuilderModal";
