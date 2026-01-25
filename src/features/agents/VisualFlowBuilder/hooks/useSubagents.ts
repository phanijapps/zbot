// ============================================================================
// VISUAL FLOW BUILDER - USE SUBAGENTS HOOK
// Hook for managing subagents in the Zero IDE
// ============================================================================

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Agent } from "@/shared/types";

interface UseSubagentsResult {
  subagents: Agent[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  saveSubagent: (subagent: Agent) => Promise<Agent>;
  getSubagent: (subagentId: string) => Promise<Agent | null>;
  deleteSubagent: (subagentId: string) => Promise<void>;
}

export function useSubagents(agentId: string | null): UseSubagentsResult {
  const [subagents, setSubagents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Fetch all subagents for the given agent
  const refresh = useCallback(async () => {
    if (!agentId) {
      setSubagents([]);
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const result = await invoke<Agent[]>("list_subagents", { agentId });
      setSubagents(result);
    } catch (err) {
      console.error("Failed to load subagents:", err);
      setError(err instanceof Error ? err.message : "Failed to load subagents");
      setSubagents([]);
    } finally {
      setLoading(false);
    }
  }, [agentId]);

  // Save (create or update) a subagent
  const saveSubagent = useCallback(async (subagent: Agent): Promise<Agent> => {
    if (!agentId) {
      throw new Error("No agent ID provided");
    }

    setLoading(true);
    setError(null);

    try {
      const result = await invoke<Agent>("save_subagent", {
        agentId,
        subagent,
      });
      // Refresh the list after saving
      await refresh();
      return result;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : "Failed to save subagent";
      setError(errorMsg);
      throw new Error(errorMsg);
    } finally {
      setLoading(false);
    }
  }, [agentId, refresh]);

  // Get a specific subagent by ID
  const getSubagent = useCallback(async (subagentId: string): Promise<Agent | null> => {
    if (!agentId) {
      return null;
    }

    try {
      const result = await invoke<Agent>("get_subagent", {
        agentId,
        subagentId,
      });
      return result;
    } catch (err) {
      console.error(`Failed to get subagent ${subagentId}:`, err);
      return null;
    }
  }, [agentId]);

  // Delete a subagent
  const deleteSubagent = useCallback(async (subagentId: string): Promise<void> => {
    if (!agentId) {
      throw new Error("No agent ID provided");
    }

    setLoading(true);
    setError(null);

    try {
      await invoke("delete_subagent", {
        agentId,
        subagentId,
      });
      // Refresh the list after deleting
      await refresh();
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : "Failed to delete subagent";
      setError(errorMsg);
      throw new Error(errorMsg);
    } finally {
      setLoading(false);
    }
  }, [agentId, refresh]);

  // Load subagents when agentId changes
  useEffect(() => {
    refresh();
  }, [refresh]);

  return {
    subagents,
    loading,
    error,
    refresh,
    saveSubagent,
    getSubagent,
    deleteSubagent,
  };
}
