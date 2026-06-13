// ============================================================================
// useRecallTrace — Observatory v2 Phase 3
// ============================================================================
//
// Subscribes to the global event channel and collects the last N
// `recall_trace` events. Each trace carries seed entity ids, the
// LCA-walk aggregate ids, and the surfaced-item count from a recall
// that just happened anywhere in the daemon (any session, any agent).
//
// The Observatory v2 canvas consumes the most recent traces to
// trigger targeted pulses on the matching L1 aggregates and flash
// the central apex — turning the static hierarchy into a live map
// of what the agent's mind just touched.
// ============================================================================

import { useEffect, useRef, useState } from "react";
import { getTransport } from "@/services/transport";
import type { GlobalEvent } from "@/services/transport/types";

/** Maximum traces kept in memory. Old ones drop off the tail. */
const MAX_TRACES = 16;

/**
 * One recall-pipeline execution. Mirrors `ServerMessage::RecallTrace`
 * on the wire, with a client-side `at` wall-clock for animation
 * sequencing.
 */
export interface RecallTraceRecord {
  /** Wall-clock when the trace arrived (used as the animation seed). */
  at: number;
  /** Monotonic id so React keys never collide on rapid re-emits. */
  id: string;
  agentId: string;
  conversationId?: string;
  seedEntityIds: string[];
  seedAggregateIds: string[];
  lcaAggregateId?: string;
  surfacedItemCount: number;
}

let nextTraceId = 0;

export interface UseRecallTraceResult {
  /** Most-recent-first list of traces, capped at `MAX_TRACES`. */
  traces: RecallTraceRecord[];
  /** The single most recent trace, or null if none have arrived. */
  latest: RecallTraceRecord | null;
}

/**
 * Subscribes to the global event stream and exposes recall_trace events.
 *
 * Mount-once: keeps the subscription alive for the lifetime of the
 * component. The transport caches the connection so multiple consumers
 * are cheap. Bad events (missing required fields) are dropped silently
 * to keep the canvas resilient to wire-format drift.
 */
export function useRecallTrace(): UseRecallTraceResult {
  const [traces, setTraces] = useState<RecallTraceRecord[]>([]);
  // Keep a ref for the unsub fn so cleanup is robust against React
  // strict-mode double-invocation.
  const unsubRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const transport = await getTransport();
      if (cancelled) return;
      const off = transport.onGlobalEvent((event: GlobalEvent) => {
        if (event.type !== "recall_trace") return;
        const raw = event as unknown as Record<string, unknown>;
        const agentId = raw["agent_id"];
        if (typeof agentId !== "string") return;
        const conversationId = raw["conversation_id"];
        const seedEntityIds = raw["seed_entity_ids"];
        const seedAggregateIds = raw["seed_aggregate_ids"];
        const lcaAggregateId = raw["lca_aggregate_id"];
        const surfacedItemCount = raw["surfaced_item_count"];
        const trace: RecallTraceRecord = {
          at: Date.now(),
          id: `rt-${nextTraceId++}`,
          agentId,
          conversationId:
            typeof conversationId === "string" ? conversationId : undefined,
          seedEntityIds: Array.isArray(seedEntityIds)
            ? seedEntityIds.filter((x): x is string => typeof x === "string")
            : [],
          seedAggregateIds: Array.isArray(seedAggregateIds)
            ? seedAggregateIds.filter(
                (x): x is string => typeof x === "string",
              )
            : [],
          lcaAggregateId:
            typeof lcaAggregateId === "string" ? lcaAggregateId : undefined,
          surfacedItemCount:
            typeof surfacedItemCount === "number" ? surfacedItemCount : 0,
        };
        setTraces((prev) => [trace, ...prev].slice(0, MAX_TRACES));
      });
      unsubRef.current = off;
    })();
    return () => {
      cancelled = true;
      unsubRef.current?.();
      unsubRef.current = null;
    };
  }, []);

  return { traces, latest: traces[0] ?? null };
}
