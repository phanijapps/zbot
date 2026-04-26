// ============================================================================
// MISSION CONTROL — types
// ============================================================================

import type { LogSession } from "@/services/transport/types";

/** Aggregate counts shown in the KPI strip. */
export interface MissionKpis {
  /** Sessions currently in `running` status. */
  running: number;
  /** Sessions currently in `queued` status. */
  queued: number;
  /** Sessions in `completed` status that started in the last 24h. */
  done24h: number;
  /** Sessions in `failed`/`error`/`crashed`/`stopped` status, last 24h. */
  failed24h: number;
  /** Sessions currently in `paused` status. */
  paused: number;
  /** Total tokens across `running` sessions (live workload indicator). */
  runningTokens: number;
  /** Done24h / (Done24h + Failed24h) — null when window is empty. */
  successRate: number | null;
  /** % delta in done count vs the previous 24h window. null when no prior data. */
  delta24h: number | null;
}

/** Filter state for the session list. */
export interface SessionFilters {
  /** Free-text search against title + agent name + session id. */
  search: string;
  /** Status types currently visible. */
  status: {
    running: boolean;
    queued: boolean;
    completed: boolean;
    failed: boolean;
    paused: boolean;
  };
}

/** Sources we recognize on the per-session source badge. */
export type SessionSource = "web" | "cli" | "cron" | "slack" | "api" | "unknown";

/** Returns the canonical source for a session. */
export function deriveSource(session: LogSession): SessionSource {
  // The current API doesn't carry a `source` field on LogSession yet, so
  // we infer from the agent_name / mode. This is a UI-only inference and
  // can be replaced when the backend stamps the source explicitly.
  const mode = session.mode?.toLowerCase();
  if (mode === "fast" || mode === "chat") return "web";
  // No reliable inference for cron / slack / cli at this layer — default to web.
  return "web";
}
