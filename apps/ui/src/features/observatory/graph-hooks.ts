// ============================================================================
// OBSERVATORY DATA HOOKS
// Data fetching hooks for the Observatory knowledge graph visualization.
// ============================================================================

import { useState, useEffect, useCallback, useMemo } from "react";
import { getTransport } from "@/services/transport";
import type {
  GraphEntity,
  GraphRelationship,
  GraphEntityListResponse,
  GraphNeighborResponse,
  GraphRelationshipListResponse,
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
  governance?: {
    supported: boolean;
    ontologyIds: string[];
    taxonomySchemeIds: string[];
    validationMode: string;
    allowUnclassified: string;
    skosExpansion: {
      maxDepth: number;
      maxFanOut: number;
      maxCandidates: number;
    };
    bootstrap?: {
      ontologyId: string;
      taxonomySchemeId: string;
      classCount: number;
      propertyCount: number;
      conceptCount: number;
      relationCount: number;
    };
    findingCount: number;
    findingCodes: string[];
  } | null;
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
  /** Server-exact scope totals from the same reads as the pages (live). */
  totals: { entities: number; relationships: number };
  /** True when every page in scope has been loaded (exhausted). */
  complete: boolean;
  /** True when traversal stopped early: totals changed or a page made no
   * progress (all rows already merged). The view offers refresh. */
  stale: boolean;
  /** True when the memory-admission cap stopped further pages. */
  capped: boolean;
  /** Loopback-only denial (403 from the graph boundary). */
  loopbackOnly: boolean;
  /** Edges whose endpoint entities are not (yet) loaded — counted, never
   * silently discarded (AC2). */
  unresolvedEndpoints: number;
}

/** UI memory admission caps (spec AC3). */
const MAX_ENTITIES = 50_000;
const MAX_RELATIONSHIPS = 100_000;
const MAX_SERIALIZED_BYTES = 64 * 1024 * 1024;

/** Page sizes for progressive traversal (contract max is 1000; fewer,
 * larger pages cut the sequential round-trip chain ~5x). */
const ENTITY_PAGE = 1000;
const RELATIONSHIP_PAGE = 1000;

/** Qualified identity: agent-local IDs cannot collide across agents (AC2). */
const qualifiedKey = (agentId: string, id: string) => `${agentId}:${id}`;

interface PagedFetch<T> {
  rows: T[];
  total: number;
  nextOffset: number | null;
}

/** Progressively traverse a paged endpoint until exhausted, capped, or
 * invalidated — merging by qualified identity so no row appears twice and a
 * no-progress page (all rows already merged) stops traversal as stale. */
async function traversePages<T>(
  isCancelled: () => boolean,
  fetchPage: (offset: number) => Promise<PagedFetch<T>>,
  rowKey: (row: T) => string,
  merged: Map<string, T>,
  totals: { current: number | null },
  cap: (count: number) => boolean,
  onPage?: () => void,
): Promise<{ stale: boolean; capped: boolean }> {
  let offset = 0;
  let stale = false;
  let capped = false;
  let expectedTotal: number | null = null;
  loop: while (true) {
    if (isCancelled()) return { stale, capped };
    const page = await fetchPage(offset);
    if (isCancelled()) return { stale, capped };
    totals.current = page.total;
    if (expectedTotal === null) expectedTotal = page.total;
    else if (page.total !== expectedTotal) {
      // Live view: the dataset changed under traversal.
      stale = true;
      break;
    }
    let added = 0;
    for (const row of page.rows) {
      const key = rowKey(row);
      if (!merged.has(key)) {
        merged.set(key, row);
        added += 1;
      }
    }
    if (added === 0 && page.rows.length > 0) {
      // No-progress page (AC1): all rows were already merged.
      stale = true;
      break;
    }
    if (cap(merged.size)) {
      capped = true;
      break;
    }
    onPage?.();
    if (page.nextOffset === null || page.nextOffset === undefined) {
      totals.current = page.total;
      break loop;
    }
    offset = page.nextOffset;
  }
  return { stale, capped };
}


// ============================================================================
// INTERNAL HELPERS
// ============================================================================

/**
 * Resolve the gateway base URL from the transport layer.
 *
 * Returns "" (empty string) for same-origin browser requests — the default
 * transport config in a browser sets `httpUrl: ""` so `fetch("${base}/api/x")`
 * becomes `fetch("/api/x")`, resolved against `window.location.origin`. That
 * keeps the gateway port out of the wire URL so mobile clients hitting the
 * daemon on its LAN address don't run into a port mismatch.
 *
 * The localhost fallback is reserved for non-browser callers (SSR, unit
 * tests where window is undefined).
 */
async function getBaseUrl(): Promise<string> {
  try {
    const transport = await getTransport();
    // The HttpTransport exposes the base URL via its config.
    // We cast because the internal `config` property isn't part of the
    // public Transport interface.
    const cfg = (transport as unknown as { config?: { httpUrl: string } }).config;
    // Empty string is a valid same-origin signal — keep it as-is.
    if (cfg && typeof cfg.httpUrl === "string") return cfg.httpUrl;
  } catch {
    // swallow
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
      const error = new Error(text || `HTTP ${res.status}`) as Error & { status?: number };
      error.status = res.status;
      throw error;
    }
    return (await res.json()) as T;
  } catch (err) {
    clearTimeout(timeoutId);
    throw err;
  }
}

async function postJson<T>(path: string): Promise<T> {
  const base = await getBaseUrl();
  // Distillation can be slow — use a generous 120 s timeout per session.
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), 120_000);
  try {
    const res = await fetch(`${base}${path}`, {
      method: "POST",
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
  const [totals, setTotals] = useState({ entities: 0, relationships: 0 });
  const [complete, setComplete] = useState(false);
  const [stale, setStale] = useState(false);
  const [capped, setCapped] = useState(false);
  const [loopbackOnly, setLoopbackOnly] = useState(false);
  const [entityMap, setEntityMap] = useState<Map<string, GraphEntity>>(new Map());
  const [relationshipMap, setRelationshipMap] = useState<Map<string, GraphRelationship>>(new Map());

  const refetch = useCallback(() => setTick((t) => t + 1), []);

  useEffect(() => {
    let cancelled = false;
    const isCancelled = () => cancelled;
    setLoading(true);
    setError(null);
    setStale(false);
    setCapped(false);
    setComplete(false);
    setLoopbackOnly(false);
    setEntities([]);
    setRelationships([]);
    setEntityMap(new Map());
    setRelationshipMap(new Map());
    setTotals({ entities: 0, relationships: 0 });

    const load = async () => {
      const nextEntities = new Map<string, GraphEntity>();
      const nextRelationships = new Map<string, GraphRelationship>();
      const entityTotals = { current: null as number | null };
      const relationshipTotals = { current: null as number | null };
      let staleResult = false;
      let cappedResult = false;
      let lastError: string | null = null;
      let denied = false;

      const serializedBudget = () => {
        let bytes = 0;
        for (const entity of nextEntities.values()) bytes += JSON.stringify(entity).length;
        for (const relationship of nextRelationships.values()) bytes += JSON.stringify(relationship).length;
        return bytes;
      };
      const entityCap = (count: number) =>
        count >= MAX_ENTITIES || serializedBudget() >= MAX_SERIALIZED_BYTES;
      const relationshipCap = (count: number) =>
        count >= MAX_RELATIONSHIPS || serializedBudget() >= MAX_SERIALIZED_BYTES;

      try {
        const transport = await getTransport();
        const entityPage = (offset: number): Promise<PagedFetch<GraphEntity>> =>
          agentId
            ? transport
                .getGraphEntities(agentId, { limit: ENTITY_PAGE, offset })
                .then((response) => {
                  if (!response.success || !response.data) throw new Error(response.error || "Failed to fetch entities");
                  return {
                    rows: response.data.entities,
                    total: response.data.total,
                    nextOffset: response.data.next_offset ?? null,
                  };
                })
            : fetchJson<GraphEntityListResponse>(
                `/api/graph/all/entities?limit=${ENTITY_PAGE}&offset=${offset}`
              ).then((data) => ({
                rows: data.entities,
                total: data.total,
                nextOffset: data.next_offset ?? null,
              }));
        const relationshipPage = (offset: number): Promise<PagedFetch<GraphRelationship>> =>
          agentId
            ? transport
                .getGraphRelationships(agentId, { limit: RELATIONSHIP_PAGE, offset })
                .then((response) => {
                  if (!response.success || !response.data) throw new Error(response.error || "Failed to fetch relationships");
                  return {
                    rows: response.data.relationships,
                    total: response.data.total,
                    nextOffset: response.data.next_offset ?? null,
                  };
                })
            : fetchJson<GraphRelationshipListResponse>(
                `/api/graph/all/relationships?limit=${RELATIONSHIP_PAGE}&offset=${offset}`
              ).then((data) => ({
                rows: data.relationships,
                total: data.total,
                nextOffset: data.next_offset ?? null,
              }));

        // Entities and relationships page in PARALLEL (independent chains)
        // and flush into React state per page so the graph grows visibly.
        const flush = () => {
          if (isCancelled()) return;
          setEntities([...nextEntities.values()]);
          setRelationships([...nextRelationships.values()]);
          setEntityMap(new Map(nextEntities));
          setRelationshipMap(new Map(nextRelationships));
          setTotals({
            entities: entityTotals.current ?? nextEntities.size,
            relationships: relationshipTotals.current ?? nextRelationships.size,
          });
        };

        const [entityResult, relationshipResult] = await Promise.all([
          traversePages(
            isCancelled,
            entityPage,
            (entity) => qualifiedKey(entity.agent_id, entity.id),
            nextEntities,
            entityTotals,
            entityCap,
            flush
          ),
          traversePages(
            isCancelled,
            relationshipPage,
            (relationship) => qualifiedKey(relationship.agent_id, relationship.id),
            nextRelationships,
            relationshipTotals,
            relationshipCap,
            flush
          ),
        ]);
        staleResult = entityResult.stale || relationshipResult.stale;
        cappedResult = entityResult.capped || relationshipResult.capped;
      } catch (err) {
        if (!isCancelled()) {
          lastError = err instanceof Error ? err.message : String(err);
          denied = (err as Error & { status?: number }).status === 403;
        }
      }

      if (isCancelled()) return;
      setEntities([...nextEntities.values()]);
      setRelationships([...nextRelationships.values()]);
      setEntityMap(nextEntities);
      setRelationshipMap(nextRelationships);
      setTotals({
        entities: entityTotals.current ?? nextEntities.size,
        relationships: relationshipTotals.current ?? nextRelationships.size,
      });
      setStale(staleResult);
      setCapped(cappedResult);
      setComplete(!staleResult && !cappedResult && lastError === null);
      setError(lastError);
      setLoopbackOnly(denied);
      setLoading(false);
    };

    load();
    return () => {
      cancelled = true;
    };
  }, [agentId, tick]);

  // Unresolved edge endpoints (AC2): counted, never silently discarded.
  const unresolvedEndpoints = useMemo(() => {
    let unresolved = 0;
    for (const relationship of relationshipMap.values()) {
      if (!entityMap.has(qualifiedKey(relationship.agent_id, relationship.source_entity_id))) unresolved += 1;
      else if (!entityMap.has(qualifiedKey(relationship.agent_id, relationship.target_entity_id))) unresolved += 1;
    }
    return unresolved;
  }, [entityMap, relationshipMap]);

  return {
    entities,
    relationships,
    loading,
    error,
    refetch,
    totals,
    complete,
    stale,
    capped,
    loopbackOnly,
    unresolvedEndpoints,
  };
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
  const [tick, setTick] = useState(0);

  const refetch = useCallback(() => setTick((t) => t + 1), []);

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
  }, [tick]);

  return { status, loading, error, refetch };
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

// ============================================================================
// BACKFILL HOOK
// ============================================================================

/** Shape returned by GET /api/distillation/undistilled */
interface UndistilledSession {
  session_id: string;
  agent_id: string;
}

/** Progress state for the backfill operation. */
export interface BackfillProgress {
  current: number;
  total: number;
}

/**
 * Hook to drive bulk-distillation ("backfill") from the UI.
 *
 * Fetches undistilled sessions, then triggers distillation for each one
 * sequentially, updating progress as it goes.
 */
export function useBackfill(onComplete?: () => void) {
  const [isRunning, setIsRunning] = useState(false);
  const [isDone, setIsDone] = useState(false);
  const [progress, setProgress] = useState<BackfillProgress>({ current: 0, total: 0 });
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(async () => {
    setIsRunning(true);
    setIsDone(false);
    setError(null);
    setProgress({ current: 0, total: 0 });

    try {
      const sessions = await fetchJson<UndistilledSession[]>(
        "/api/distillation/undistilled"
      );

      if (sessions.length === 0) {
        setIsDone(true);
        setIsRunning(false);
        onComplete?.();
        return;
      }

      setProgress({ current: 0, total: sessions.length });

      for (let i = 0; i < sessions.length; i++) {
        try {
          await postJson<unknown>(
            `/api/distillation/trigger/${sessions[i].session_id}`
          );
        } catch {
          // Individual failures are non-fatal — continue with next session.
        }
        setProgress({ current: i + 1, total: sessions.length });
      }

      setIsDone(true);
      onComplete?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsRunning(false);
    }
  }, [onComplete]);

  return { run, isRunning, isDone, progress, error };
}

// ============================================================================
// SERVER-BACKED SEARCH (AC4) — reaches entities beyond the loaded pages.
// ============================================================================

export interface GraphSearchHit extends GraphEntity {}

export interface GraphSearchState {
  query: string;
  results: GraphSearchHit[];
  searched: boolean;
  /** True when the search request itself failed — distinct from no matches. */
  failed: boolean;
  setQuery(query: string): void;
  open(hit: GraphSearchHit): void;
}

/** Debounced server search over the selected scope. Selecting a hit loads
 * the entity through the direct per-agent read (or the aggregate endpoint)
 * and hands it to the page's selection handler. */
export function useGraphSearch(
  agentId: string | undefined,
  onSelect: (entity: GraphEntity) => void,
): GraphSearchState {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<GraphSearchHit[]>([]);
  const [searched, setSearched] = useState(false);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (query.trim().length < 2) {
      setResults([]);
      setSearched(false);
      setFailed(false);
      return;
    }
    let cancelled = false;
    const timer = setTimeout(() => {
      void (async () => {
        try {
          const data = agentId
            ? await fetchJson<GraphEntityListResponse>(
                `/api/graph/${encodeURIComponent(agentId)}/search?q=${encodeURIComponent(query.trim())}&limit=20`
              )
            : await fetchJson<GraphEntityListResponse>(
                `/api/graph/all/search?q=${encodeURIComponent(query.trim())}&limit=20`
              );
          if (!cancelled) {
            setResults(data.entities);
            setSearched(true);
            setFailed(false);
          }
        } catch {
          // A failed request is not "no matches" (AC3): surface it distinctly.
          if (!cancelled) {
            setResults([]);
            setSearched(true);
            setFailed(true);
          }
        }
      })();
    }, 300);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [query, agentId]);

  const open = useCallback(
    (hit: GraphSearchHit) => {
      onSelect(hit);
    },
    [onSelect]
  );

  return { query, results, searched, failed, setQuery, open };
}
