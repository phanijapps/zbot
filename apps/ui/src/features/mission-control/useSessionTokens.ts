// ============================================================================
// MISSION CONTROL — useSessionTokens
//
// `LogSession` (from /api/logs/sessions) doesn't carry token counts in any
// reliable shape — `token_count` returns 0 for everything in the current
// gateway. Worse, that endpoint also reports STALE STATUS — it can mark a
// session as "completed" while the v2 endpoint correctly shows "running".
// We treat the v2 endpoint as the source of truth for both tokens AND
// status, then merge it into the LogSession list at the page level.
//
// `/api/executions/v2/sessions/full` carries:
//
//   SessionWithExecutions:
//     id                  → "sess-..."
//     status              → SessionStateStatus (queued/running/paused/completed/crashed)
//     total_tokens_in/out → cumulative across all the session's executions
//     executions[]        → per-agent execution records, each carrying:
//        id              → "exec-..."   (this matches LogSession.session_id!)
//        session_id      → "sess-..."   (parent session id)
//        agent_id        → "root" | "research-agent" | …
//        tokens_in/out   → tokens for THIS agent's slice of the session
//
// We fetch the full list, then build keyed lookups:
//   • totals + canonical status by root execution id
//   • per-execution slices by root execution id
//
// The hook auto-refreshes every 5s while any session is in `running` /
// `queued` status (matches the cadence of useAutoRefresh on the log list).
// ============================================================================

import { useState, useEffect, useRef } from "react";
import { getTransport } from "@/services/transport";
import type { SessionStateStatus, SessionStatus } from "@/services/transport/types";

/** Cumulative session totals (root + all subagents) + canonical status. */
export interface SessionTokenSummary {
  in: number;
  out: number;
  total: number;
  /** Canonical status from the v2 endpoint (more reliable than LogSession.status). */
  status?: SessionStateStatus;
}

/** Per-execution (per-agent) token slice within a session. */
export interface ExecutionTokenEntry {
  /** AgentExecution.id (matches LogSession.session_id for the root exec). */
  executionId: string;
  /** Agent that produced these tokens. */
  agentId: string;
  in: number;
  out: number;
}

export interface SessionTokenIndex {
  /** Look up cumulative totals by **root execution id** (i.e. LogSession.session_id). */
  byRootExecId: Map<string, SessionTokenSummary>;
  /**
   * Look up per-execution slices by root exec id. Each agent that participated
   * in the session appears as one entry. Subagents that ran multiple times
   * appear as multiple entries.
   */
  executionsByRootExecId: Map<string, ExecutionTokenEntry[]>;
}

const EMPTY_INDEX: SessionTokenIndex = {
  byRootExecId: new Map(),
  executionsByRootExecId: new Map(),
};

/**
 * Hook: fetches /api/executions/v2/sessions/full and returns a token-lookup
 * index keyed by root execution id (so it joins cleanly with LogSession data).
 *
 * @param activelyRunning  When true, the hook polls every 5s. Pass `false`
 *   when no sessions are in flight to skip background traffic.
 */
export function useSessionTokens(activelyRunning: boolean): SessionTokenIndex {
  const [index, setIndex] = useState<SessionTokenIndex>(EMPTY_INDEX);
  const [tick, setTick] = useState(0);
  const tickRef = useRef(tick);
  tickRef.current = tick;

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const transport = await getTransport();
        const result = await transport.listSessionsFull({ limit: 200 });
        if (cancelled) return;
        if (!result.success || !result.data) return;
        setIndex(buildIndex(result.data));
      } catch {
        // Swallow — the token columns will simply stay empty until the next
        // successful refresh. Better than tearing down the page.
      }
    })();
    return () => { cancelled = true; };
  }, [tick]);

  useEffect(() => {
    if (!activelyRunning) return;
    const id = setInterval(() => setTick((t) => t + 1), 5000);
    return () => clearInterval(id);
  }, [activelyRunning]);

  return index;
}

/** Pure: turn the raw API list into the keyed lookup the UI needs. */
export function buildIndex(
  sessions: Array<{
    id: string;
    status?: SessionStateStatus;
    total_tokens_in: number;
    total_tokens_out: number;
    executions?: Array<{
      id: string;
      agent_id: string;
      tokens_in: number;
      tokens_out: number;
      delegation_type?: string;
    }>;
  }>,
): SessionTokenIndex {
  const byRootExecId = new Map<string, SessionTokenSummary>();
  const executionsByRootExecId = new Map<string, ExecutionTokenEntry[]>();

  for (const s of sessions) {
    const executions = s.executions ?? [];
    // The root execution is the one whose delegation_type is "root", or the
    // first one when delegation_type is unset.
    const rootExec =
      executions.find((e) => e.delegation_type === "root") ??
      executions[0] ??
      null;
    if (!rootExec) continue;

    const rootExecId = rootExec.id;
    const tokensIn = s.total_tokens_in ?? 0;
    const tokensOut = s.total_tokens_out ?? 0;
    byRootExecId.set(rootExecId, {
      in: tokensIn,
      out: tokensOut,
      total: tokensIn + tokensOut,
      status: s.status,
    });

    executionsByRootExecId.set(
      rootExecId,
      executions.map((e) => ({
        executionId: e.id,
        agentId: e.agent_id,
        in: e.tokens_in ?? 0,
        out: e.tokens_out ?? 0,
      })),
    );
  }

  return { byRootExecId, executionsByRootExecId };
}

/**
 * Map the wider v2 `SessionStateStatus` (queued/running/paused/completed/
 * crashed) onto the narrower `SessionStatus` used by LogSession (running/
 * completed/error/stopped). `queued` and `paused` have no LogSession
 * equivalent — we surface them as "running" so the UI keeps polling and
 * the Live indicator stays on.
 */
export function normalizeV2Status(v2: SessionStateStatus): SessionStatus {
  switch (v2) {
    case "completed": return "completed";
    case "crashed":   return "error";
    case "running":
    case "queued":
    case "paused":
    default:          return "running";
  }
}

/**
 * Merge the canonical v2 status into a list of LogSession objects. Returns
 * a NEW array so React's referential checks notice the change. When v2 has
 * no entry for a session, the LogSession value is preserved unchanged.
 */
export function applyV2Status<T extends { session_id: string; status: SessionStatus }>(
  sessions: T[],
  index: SessionTokenIndex,
): T[] {
  if (index.byRootExecId.size === 0) return sessions;
  return sessions.map((s) => {
    const entry = index.byRootExecId.get(s.session_id);
    if (!entry?.status) return s;
    const truth = normalizeV2Status(entry.status);
    if (truth === s.status) return s;
    return { ...s, status: truth };
  });
}

/** Aggregate per-agent totals when an agent ran multiple times in one session. */
export function sumExecutionTokensByAgent(
  entries: ExecutionTokenEntry[] | undefined,
): Map<string, { in: number; out: number }> {
  const out = new Map<string, { in: number; out: number }>();
  if (!entries) return out;
  for (const e of entries) {
    const cur = out.get(e.agentId) ?? { in: 0, out: 0 };
    cur.in += e.in;
    cur.out += e.out;
    out.set(e.agentId, cur);
  }
  return out;
}
