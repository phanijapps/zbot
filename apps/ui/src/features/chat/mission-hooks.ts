// =============================================================================
// mission-hooks — recent-sessions side-rail loader
//
// What's left after the legacy MissionControl removal:
// the recent-sessions sidebar in research-v2 still needs a tiny hook to
// fetch the latest few root sessions for the drawer / hero. That is
// `useRecentSessions` + its options. Everything else in this file's
// previous life — the giant `useMissionControl` reducer, the WS event
// handlers, the `NarrativeBlock` / `MissionState` / `IntentAnalysis`
// data model, the `__testInternals` namespace — was driven by the old
// `chat/MissionControl.tsx` page and went away when it did.
//
// If you find yourself reaching for any of those exports in new code,
// that's a smell — research-v2 owns its own session state machine
// (`useResearchSession.ts`) and chat-v2 owns its own (`useQuickChat.ts`).
// =============================================================================

import { useEffect, useState } from "react";
import { getTransport } from "@/services/transport";
import type { LogSession } from "@/services/transport/types";

const WEB_CONV_ID_KEY = "agentzero_web_conv_id";
const WEB_SESSION_ID_KEY = "agentzero_web_session_id";
const WEB_LOG_SESSION_ID_KEY = "agentzero_web_log_session_id";

const SECONDS_PER_MINUTE = 60;
const SECONDS_PER_HOUR = 60 * 60;
const SECONDS_PER_DAY = 60 * 60 * 24;

/**
 * Formats an ISO timestamp as a coarse relative-time string ("just now",
 * "5 m", "3 h", "2 d"). Used by the recent-sessions rail in the hero
 * card. Sub-minute and zero/future deltas collapse to "just now".
 */
export function timeAgo(iso: string): string {
  const then = Date.parse(iso);
  if (!Number.isFinite(then)) return "just now";
  const seconds = Math.floor((Date.now() - then) / 1000);
  if (seconds < SECONDS_PER_MINUTE) return "just now";
  if (seconds < SECONDS_PER_HOUR) return `${Math.floor(seconds / SECONDS_PER_MINUTE)} m`;
  if (seconds < SECONDS_PER_DAY) return `${Math.floor(seconds / SECONDS_PER_HOUR)} h`;
  return `${Math.floor(seconds / SECONDS_PER_DAY)} d`;
}

/**
 * Default click-handler for a recent-session card on the hero. Persists
 * the session/conversation ids to localStorage and reloads the page so
 * the hook stack rebinds. Override by passing `onSelectSession` to
 * `HeroInput` if you need a SPA-style transition.
 */
export function switchToSession(sessionId: string, conversationId: string): void {
  try {
    localStorage.setItem(WEB_CONV_ID_KEY, conversationId);
    localStorage.setItem(WEB_SESSION_ID_KEY, conversationId);
    localStorage.setItem(WEB_LOG_SESSION_ID_KEY, sessionId);
  } catch {
    // Private mode / quota — nothing useful to do; reload still happens.
  }
  if (typeof window !== "undefined") window.location.reload();
}

export interface UseRecentSessionsOptions {
  /**
   * Optional predicate. When provided, sessions matching it are filtered
   * out *after* the fetch — used by research-v2 to hide chat-v2 sessions
   * (`isChatSession`) from the research recents rail.
   */
  exclude?: (row: LogSession) => boolean;
}

const RAW_LIMIT_WITH_EXCLUDE = 20;
const RAW_LIMIT_PLAIN = 5;
const FINAL_CAP = 5;

/**
 * Loads up to 5 recent root sessions from the gateway log endpoint.
 * Re-runs whenever the `exclude` predicate identity changes; over-fetches
 * (20 rows) when filtering so the post-filter slice still has 5
 * candidates in the typical case.
 */
export function useRecentSessions(options: UseRecentSessionsOptions = {}) {
  const [sessions, setSessions] = useState<LogSession[]>([]);
  const { exclude } = options;

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const transport = await getTransport();
        const limit = exclude ? RAW_LIMIT_WITH_EXCLUDE : RAW_LIMIT_PLAIN;
        const res = await transport.listLogSessions({ limit, root_only: true });
        if (cancelled || !res.success || !res.data) return;
        const filtered = exclude ? res.data.filter((r) => !exclude(r)) : res.data;
        setSessions(filtered.slice(0, FINAL_CAP));
      } catch (err) {
        console.error("[useRecentSessions] Failed to load sessions:", err);
      }
    };
    load();
    return () => {
      cancelled = true;
    };
  }, [exclude]);

  return { sessions };
}
