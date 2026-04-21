// ============================================================================
// Command Deck hooks — useWards, useWardContent, useHybridSearch, useTimewarp
// Task 9 of Memory Tab Command Deck plan.
// ============================================================================

import { useCallback, useEffect, useState } from "react";
import { getTransport } from "@/services/transport";
import type {
  WardContent,
  HybridSearchRequest,
  HybridSearchResponse,
} from "@/services/transport/types";

export interface WardListItem {
  id: string;
  count: number;
}

/** Lists available wards with fact counts. */
export function useWards(): WardListItem[] {
  const [wards, setWards] = useState<WardListItem[]>([]);

  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const transport = await getTransport();
        const result = await transport.listWards();
        if (alive && result.success && result.data) {
          setWards(result.data);
        }
      } catch {
        // Leave wards empty on transport failure.
      }
    })();
    return () => {
      alive = false;
    };
  }, []);

  return wards;
}

export interface UseWardContentResult {
  data: WardContent | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

/**
 * Fetches the full aggregated content for one ward. Pass `null` to skip
 * fetching (no active ward selected).
 */
export function useWardContent(wardId: string | null): UseWardContentResult {
  const [data, setData] = useState<WardContent | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!wardId) return;
    setLoading(true);
    try {
      const transport = await getTransport();
      const result = await transport.getWardContent(wardId);
      if (result.success) {
        setData(result.data ?? null);
        setError(null);
      } else {
        setError(result.error ?? "failed");
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [wardId]);

  useEffect(() => {
    if (!wardId) {
      setData(null);
      setError(null);
      setLoading(false);
      return;
    }
    refresh().catch(() => {});
  }, [wardId, refresh]);

  return { data, loading, error, refresh };
}

export interface UseHybridSearchResult {
  data: HybridSearchResponse | null;
  loading: boolean;
}

const DEBOUNCE_MS = 250;

/**
 * Debounced (250 ms) unified hybrid search. `opts` is serialized with
 * `JSON.stringify` for the effect dependency to avoid re-firing on reference
 * change when the shape is stable.
 */
export function useHybridSearch(
  query: string,
  opts: Omit<HybridSearchRequest, "query">,
): UseHybridSearchResult {
  const [data, setData] = useState<HybridSearchResponse | null>(null);
  const [loading, setLoading] = useState(false);

  const optsKey = JSON.stringify(opts);
  useEffect(() => {
    if (!query.trim()) {
      setData(null);
      setLoading(false);
      return;
    }
    let alive = true;
    const handle = setTimeout(async () => {
      setLoading(true);
      try {
        const transport = await getTransport();
        const parsedOpts = JSON.parse(optsKey) as Omit<
          HybridSearchRequest,
          "query"
        >;
        const result = await transport.searchMemoryHybrid({
          query,
          ...parsedOpts,
        });
        if (alive && result.success) {
          setData(result.data ?? null);
        }
      } catch {
        // Swallow: keep previous data, surface nothing.
      } finally {
        if (alive) setLoading(false);
      }
    }, DEBOUNCE_MS);
    return () => {
      alive = false;
      clearTimeout(handle);
    };
  }, [query, optsKey]);

  return { data, loading };
}

export interface UseTimewarpResult {
  days: number;
  setDays: (n: number) => void;
}

/** Local UI state for the timewarp slider (days back from now). */
export function useTimewarp(): UseTimewarpResult {
  const [days, setDays] = useState(0);
  return { days, setDays };
}
