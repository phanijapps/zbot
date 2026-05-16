// ============================================================================
// Belief Network data hooks — fetch /api/belief-network/{stats,activity}
// ============================================================================
//
// Mirrors the data-fetching pattern in `../graph-hooks.ts`: same transport
// resolution, same 10s abort, same `useEffect` cancellation guard. No
// external cache library is used so the hook stays consistent with the
// rest of the observatory feature.

import { useCallback, useEffect, useState } from "react";

import { getTransport } from "@/services/transport";

import type {
  BeliefActivityEvent,
  BeliefNetworkStatsResponse,
} from "../types.beliefNetwork";

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

export async function getBeliefNetworkStats(): Promise<BeliefNetworkStatsResponse> {
  return fetchJson<BeliefNetworkStatsResponse>("/api/belief-network/stats");
}

export async function getBeliefNetworkActivity(
  limit = 50,
): Promise<BeliefActivityEvent[]> {
  return fetchJson<BeliefActivityEvent[]>(
    `/api/belief-network/activity?limit=${limit}`,
  );
}

/** Result shape returned by {@link useBeliefNetworkStats}. */
export interface UseBeliefNetworkStatsResult {
  stats: BeliefNetworkStatsResponse | null;
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

/** Polls `/api/belief-network/stats` once on mount; expose a `refetch`. */
export function useBeliefNetworkStats(): UseBeliefNetworkStatsResult {
  const [stats, setStats] = useState<BeliefNetworkStatsResponse | null>(null);
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
        const data = await getBeliefNetworkStats();
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
  }, [tick]);

  return { stats, loading, error, refetch };
}

/** Result shape returned by {@link useBeliefNetworkActivity}. */
export interface UseBeliefNetworkActivityResult {
  events: BeliefActivityEvent[];
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

/** Polls `/api/belief-network/activity`. Default limit mirrors the API. */
export function useBeliefNetworkActivity(
  limit = 50,
): UseBeliefNetworkActivityResult {
  const [events, setEvents] = useState<BeliefActivityEvent[]>([]);
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
        const data = await getBeliefNetworkActivity(limit);
        if (!cancelled) setEvents(data);
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
  }, [limit, tick]);

  return { events, loading, error, refetch };
}
