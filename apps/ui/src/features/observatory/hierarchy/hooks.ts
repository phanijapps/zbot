// ============================================================================
// Hierarchical-memory data hook — fetches /api/hierarchy/stats
//
// Mirrors `../belief-network/hooks.ts` exactly so the LearningHealthBar
// can use both hooks with the same loading-state pattern. One hook, one
// fetch on mount, expose a `refetch`. No external cache library.
// ============================================================================

import { useCallback, useEffect, useState } from "react";

import { getTransport } from "@/services/transport";

import type { HierarchyStatsResponse } from "./types";

async function getBaseUrl(): Promise<string> {
  try {
    const transport = await getTransport();
    const cfg = (transport as unknown as { config?: { httpUrl: string } })
      .config;
    if (cfg && typeof cfg.httpUrl === "string") return cfg.httpUrl;
  } catch {
    // ignore — fall back below
  }
  return typeof window === "undefined" ? "http://localhost:18791" : "";
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

export async function getHierarchyStats(
  topN = 10,
): Promise<HierarchyStatsResponse> {
  return fetchJson<HierarchyStatsResponse>(
    `/api/hierarchy/stats?top_n=${topN}`,
  );
}

/** Result shape returned by {@link useHierarchyStats}. */
export interface UseHierarchyStatsResult {
  stats: HierarchyStatsResponse | null;
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

/**
 * Polls `/api/hierarchy/stats` once on mount; exposes a `refetch` so a
 * "trigger consolidate" button can refresh after firing the sleep cycle.
 */
export function useHierarchyStats(topN = 10): UseHierarchyStatsResult {
  const [stats, setStats] = useState<HierarchyStatsResponse | null>(null);
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
        const data = await getHierarchyStats(topN);
        if (!cancelled) setStats(data);
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    load();
    return () => {
      cancelled = true;
    };
  }, [topN, tick]);

  return { stats, loading, error, refetch };
}
