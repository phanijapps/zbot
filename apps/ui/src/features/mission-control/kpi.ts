// ============================================================================
// MISSION CONTROL — KPI computation
// Pure functions for computing the live header strip from a session list.
// No side effects, no transport calls — feed it sessions, get back numbers.
//
// LogSession.status carries 4 canonical values (`SessionStatus`):
//   running · completed · error · stopped
// The KPI strip surfaces 5 buckets including `queued` and `paused`. Those
// extra buckets always read 0 here — the gateway tracks them on a different
// API (SessionStateStatus on /sessions, not /logs/sessions). When the
// gateway unifies them, plumb that source in alongside `sessions` here.
// ============================================================================

import type { LogSession, SessionStatus } from "@/services/transport/types";
import type { MissionKpis } from "./types";

const MS_PER_HOUR = 60 * 60 * 1000;
const MS_PER_DAY = 24 * MS_PER_HOUR;

type StatusBucket = "running" | "completed" | "failed" | "other";

/** Map LogSession.status to a UI bucket. */
function bucketOf(status: SessionStatus): StatusBucket {
  switch (status) {
    case "running": return "running";
    case "completed": return "completed";
    case "error":
    case "stopped": return "failed";
    default: return "other";
  }
}

/**
 * Compute the Mission Control KPI snapshot from a session list.
 *
 * @param sessions Latest session list (any order)
 * @param now Current time (defaults to Date.now()); injectable for tests
 */
export function computeKpis(sessions: LogSession[], now: number = Date.now()): MissionKpis {
  const last24hStart = now - MS_PER_DAY;
  const prev24hStart = now - 2 * MS_PER_DAY;

  let running = 0;
  let done24h = 0;
  let failed24h = 0;
  let donePrev24h = 0;
  let runningTokens = 0;

  for (const s of sessions) {
    const startedAt = new Date(s.started_at).getTime();
    const inLast24h = startedAt >= last24hStart;
    const inPrev24h = startedAt >= prev24hStart && startedAt < last24hStart;
    const bucket = bucketOf(s.status);

    if (bucket === "running") {
      running += 1;
      runningTokens += s.token_count || 0;
    } else if (bucket === "completed") {
      if (inLast24h) done24h += 1;
      if (inPrev24h) donePrev24h += 1;
    } else if (bucket === "failed" && inLast24h) {
      failed24h += 1;
    }
  }

  const denom = done24h + failed24h;
  const successRate = denom > 0 ? Math.round((done24h / denom) * 100) : null;

  let delta24h: number | null = null;
  if (donePrev24h > 0) {
    delta24h = Math.round(((done24h - donePrev24h) / donePrev24h) * 100);
  }

  return {
    running,
    queued: 0,   // not exposed on LogSession; stays 0 until API is unified
    done24h,
    failed24h,
    paused: 0,   // not exposed on LogSession; stays 0 until API is unified
    runningTokens,
    successRate,
    delta24h,
  };
}
