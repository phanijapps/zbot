// ============================================================================
// OBSERVATORY DATA HOOKS
// Data fetching hooks for the Observatory knowledge graph visualization.
// ============================================================================

import { useState, useEffect, useCallback } from "react";
import { getTransport } from "@/services/transport";
import type {
  GraphEntity,
  GraphRelationship,
  GraphEntityListResponse,
  GraphNeighborResponse,
} from "@/services/transport/types";

// ============================================================================
// TYPES
// ============================================================================

/** Aggregate statistics for the Observatory health bar. */
export interface GraphStats {
  entities: number;
  relationships: number;
  facts: number;
  episodes: number;
  distillation: {
    success_count: number;
    failed_count: number;
    skipped_count: number;
    permanently_failed_count: number;
    total_facts: number;
    total_entities: number;
    total_relationships: number;
    total_episodes: number;
  } | null;
}

/** Distillation status response from /api/distillation/status. */
export interface DistillationStatus {
  success_count: number;
  failed_count: number;
  skipped_count: number;
  permanently_failed_count: number;
  total_facts: number;
  total_entities: number;
  total_relationships: number;
  total_episodes: number;
}

/** Combined graph data (entities + relationships) for visualization. */
export interface GraphData {
  entities: GraphEntity[];
  relationships: GraphRelationship[];
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

// ============================================================================
// INTERNAL HELPERS
// ============================================================================

/**
 * Resolve the gateway base URL from the transport layer.
 * Falls back to the default if transport hasn't been initialised yet.
 */
async function getBaseUrl(): Promise<string> {
  try {
    // The transport stores the config; we reach through to its HTTP URL.
    // getTransport() initialises with defaults if needed.
    const transport = await getTransport();
    // The HttpTransport exposes the base URL via its config.
    // We cast to `any` because the internal `config` property isn't
    // part of the public Transport interface.
    const cfg = (transport as unknown as { config?: { httpUrl: string } }).config;
    if (cfg?.httpUrl) return cfg.httpUrl;
  } catch {
    // swallow
  }
  return "http://localhost:18791";
}

async function fetchJson<T>(path: string): Promise<T> {
  const base = await getBaseUrl();
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), 10_000);
  try {
    const res = await fetch(`${base}${path}`, {
      method: "GET",
      headers: { "Content-Type": "application/json" },
      signal: controller.signal,
    });
    clearTimeout(timeoutId);
    if (!res.ok) {
      const text = await res.text().catch(() => res.statusText);
      throw new Error(text || `HTTP ${res.status}`);
    }
    return (await res.json()) as T;
  } catch (err) {
    clearTimeout(timeoutId);
    throw err;
  }
}

// ============================================================================
// HOOKS
// ============================================================================

/**
 * Fetch entities + relationships for a single agent (or cross-agent when
 * agentId is omitted). Returns combined graph data for the D3 canvas.
 */
export function useGraphData(agentId?: string): GraphData {
  const [entities, setEntities] = useState<GraphEntity[]>([]);
  const [relationships, setRelationships] = useState<GraphRelationship[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  const refetch = useCallback(() => setTick((t) => t + 1), []);

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      setLoading(true);
      setError(null);
      try {
        if (agentId) {
          // Per-agent: use the existing transport methods
          const transport = await getTransport();
          const [entRes, relRes] = await Promise.all([
            transport.getGraphEntities(agentId, { limit: 200 }),
            transport.getGraphRelationships(agentId, { limit: 500 }),
          ]);

          if (cancelled) return;

          if (!entRes.success || !entRes.data) {
            throw new Error(entRes.error || "Failed to fetch entities");
          }
          if (!relRes.success || !relRes.data) {
            throw new Error(relRes.error || "Failed to fetch relationships");
          }

          setEntities(entRes.data.entities);
          setRelationships(relRes.data.relationships);
        } else {
          // Cross-agent: hit the new /api/graph/all/entities endpoint
          const data = await fetchJson<GraphEntityListResponse>(
            "/api/graph/all/entities?limit=200"
          );
          if (cancelled) return;
          setEntities(data.entities);
          // Cross-agent relationships aren't available via a single endpoint,
          // so clear them. The Observatory page can load per-agent relationships
          // when a specific agent is selected.
          setRelationships([]);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    };

    load();
    return () => { cancelled = true; };
  }, [agentId, tick]);

  return { entities, relationships, loading, error, refetch };
}

/**
 * Fetch aggregate graph statistics for the Observatory health bar.
 * Hits GET /api/graph/stats which returns cross-agent totals.
 */
export function useGraphStats() {
  const [stats, setStats] = useState<GraphStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      setLoading(true);
      setError(null);
      try {
        const data = await fetchJson<GraphStats>("/api/graph/stats");
        if (!cancelled) setStats(data);
      } catch (err) {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      } finally {
        if (!cancelled) setLoading(false);
      }
    };

    load();
    return () => { cancelled = true; };
  }, []);

  return { stats, loading, error };
}

/**
 * Fetch distillation pipeline status from GET /api/distillation/status.
 */
export function useDistillationStatus() {
  const [status, setStatus] = useState<DistillationStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      setLoading(true);
      setError(null);
      try {
        const data = await fetchJson<DistillationStatus>(
          "/api/distillation/status"
        );
        if (!cancelled) setStatus(data);
      } catch (err) {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      } finally {
        if (!cancelled) setLoading(false);
      }
    };

    load();
    return () => { cancelled = true; };
  }, []);

  return { status, loading, error };
}

/**
 * Fetch entity detail with its neighbors (connections) for the
 * entity detail panel.
 */
export function useEntityConnections(agentId: string, entityId: string) {
  const [data, setData] = useState<GraphNeighborResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      if (!agentId || !entityId) {
        setData(null);
        setLoading(false);
        return;
      }

      setLoading(true);
      setError(null);
      try {
        const transport = await getTransport();
        const res = await transport.getEntityNeighbors(agentId, entityId, {
          limit: 50,
        });
        if (cancelled) return;

        if (!res.success || !res.data) {
          throw new Error(res.error || "Failed to fetch entity connections");
        }
        setData(res.data);
      } catch (err) {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      } finally {
        if (!cancelled) setLoading(false);
      }
    };

    load();
    return () => { cancelled = true; };
  }, [agentId, entityId]);

  return { data, loading, error };
}
