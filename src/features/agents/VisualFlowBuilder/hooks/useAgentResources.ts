// ============================================================================
// VISUAL FLOW BUILDER - AGENT RESOURCES HOOKS
// Load actual providers, MCPs, and skills from the backend
// ============================================================================

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// -----------------------------------------------------------------------------
// Types from backend
// -----------------------------------------------------------------------------

export interface Provider {
  id?: string;
  name: string;
  description: string;
  api_key: string;
  base_url: string;
  models: string[];
  embedding_models?: string[];
  verified?: boolean;
  created_at?: string;
}

export interface MCPServer {
  id?: string;
  name: string;
  description: string;
  type: string;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
  headers?: Record<string, string>;
  enabled: boolean;
  status: string;
  validated?: boolean;
  created_at?: string;
}

export interface Skill {
  id?: string;
  name: string;
  display_name: string;
  description: string;
  category: string;
  instructions: string;
  created_at?: string;
}

// -----------------------------------------------------------------------------
// Hook: Load all providers
// -----------------------------------------------------------------------------

export function useProviders() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const loadProviders = async () => {
      try {
        setLoading(true);
        const result = await invoke<Provider[]>("list_providers");
        setProviders(result);
        setError(null);
      } catch (e) {
        console.error("Failed to load providers:", e);
        setError(String(e));
        setProviders([]);
      } finally {
        setLoading(false);
      }
    };

    loadProviders();
  }, []);

  return { providers, loading, error };
}

// -----------------------------------------------------------------------------
// Hook: Load all MCP servers
// -----------------------------------------------------------------------------

export function useMcpServers() {
  const [mcps, setMcps] = useState<MCPServer[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const loadMcps = async () => {
      try {
        setLoading(true);
        const result = await invoke<MCPServer[]>("list_mcp_servers");
        setMcps(result);
        setError(null);
      } catch (e) {
        console.error("Failed to load MCP servers:", e);
        setError(String(e));
        setMcps([]);
      } finally {
        setLoading(false);
      }
    };

    loadMcps();
  }, []);

  return { mcps, loading, error };
}

// -----------------------------------------------------------------------------
// Hook: Load all skills
// -----------------------------------------------------------------------------

export function useSkills() {
  const [skills, setSkills] = useState<Skill[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const loadSkills = async () => {
      try {
        setLoading(true);
        const result = await invoke<Skill[]>("list_skills");
        setSkills(result);
        setError(null);
      } catch (e) {
        console.error("Failed to load skills:", e);
        setError(String(e));
        setSkills([]);
      } finally {
        setLoading(false);
      }
    };

    loadSkills();
  }, []);

  return { skills, loading, error };
}

// -----------------------------------------------------------------------------
// Hook: Load all agent resources at once
// -----------------------------------------------------------------------------

export function useAgentResources() {
  const providers = useProviders();
  const mcps = useMcpServers();
  const skills = useSkills();

  const loading = providers.loading || mcps.loading || skills.loading;
  const error = providers.error || mcps.error || skills.error;

  return {
    providers: providers.providers,
    mcps: mcps.mcps,
    skills: skills.skills,
    loading,
    error,
  };
}
