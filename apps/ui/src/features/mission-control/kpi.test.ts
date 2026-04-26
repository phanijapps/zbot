// ============================================================================
// kpi.computeKpis — pure function tests
// LogSession.status is `SessionStatus` (running | completed | error | stopped).
// `queued` and `paused` are not exposed on LogSession yet, so those buckets
// always read 0 here — the corresponding tests assert that contract.
// ============================================================================

import { describe, it, expect } from "vitest";
import { computeKpis } from "./kpi";
import type { LogSession } from "@/services/transport/types";

const NOW = new Date("2026-04-25T20:00:00Z").getTime();
const HOUR = 3600_000;

function makeSession(overrides: Partial<LogSession> = {}): LogSession {
  return {
    session_id: `s-${Math.random()}`,
    conversation_id: `c-${Math.random()}`,
    agent_id: "agent:root",
    agent_name: "root",
    started_at: new Date(NOW - HOUR).toISOString(),
    status: "completed",
    token_count: 0,
    tool_call_count: 0,
    error_count: 0,
    child_session_ids: [],
    ...overrides,
  };
}

describe("computeKpis", () => {
  it("returns all zeros when there are no sessions", () => {
    const k = computeKpis([], NOW);
    expect(k).toEqual({
      running: 0,
      queued: 0,
      done24h: 0,
      failed24h: 0,
      paused: 0,
      runningTokens: 0,
      successRate: null,
      delta24h: null,
    });
  });

  it("counts running sessions and sums their tokens", () => {
    const k = computeKpis(
      [
        makeSession({ status: "running", token_count: 1200 }),
        makeSession({ status: "running", token_count: 800 }),
        makeSession({ status: "completed", token_count: 5000 }),
      ],
      NOW,
    );
    expect(k.running).toBe(2);
    expect(k.runningTokens).toBe(2000);
  });

  it("queued and paused are always zero (not exposed on LogSession)", () => {
    const k = computeKpis(
      [
        makeSession({ status: "running" }),
        makeSession({ status: "completed" }),
        makeSession({ status: "error" }),
      ],
      NOW,
    );
    expect(k.queued).toBe(0);
    expect(k.paused).toBe(0);
  });

  it("done24h includes completed sessions started in the last 24h, excludes older", () => {
    const k = computeKpis(
      [
        makeSession({ status: "completed", started_at: new Date(NOW - 2 * HOUR).toISOString() }),
        makeSession({ status: "completed", started_at: new Date(NOW - 12 * HOUR).toISOString() }),
        makeSession({ status: "completed", started_at: new Date(NOW - 30 * HOUR).toISOString() }),
      ],
      NOW,
    );
    expect(k.done24h).toBe(2);
  });

  it("failed24h counts error and stopped within last 24h", () => {
    const k = computeKpis(
      [
        makeSession({ status: "error", started_at: new Date(NOW - HOUR).toISOString() }),
        makeSession({ status: "stopped", started_at: new Date(NOW - 5 * HOUR).toISOString() }),
        makeSession({ status: "error", started_at: new Date(NOW - 25 * HOUR).toISOString() }),
      ],
      NOW,
    );
    expect(k.failed24h).toBe(2);
  });

  it("successRate = done / (done + failed), rounded to whole percent", () => {
    const k = computeKpis(
      [
        makeSession({ status: "completed" }),
        makeSession({ status: "completed" }),
        makeSession({ status: "completed" }),
        makeSession({ status: "error" }),
      ],
      NOW,
    );
    expect(k.successRate).toBe(75);
  });

  it("successRate is null when there are no completed/failed sessions in the window", () => {
    const k = computeKpis(
      [makeSession({ status: "running" })],
      NOW,
    );
    expect(k.successRate).toBeNull();
  });

  it("delta24h compares done count between current and previous 24h windows", () => {
    const k = computeKpis(
      [
        ...Array(9).fill(0).map(() =>
          makeSession({ status: "completed", started_at: new Date(NOW - 5 * HOUR).toISOString() })
        ),
        ...Array(6).fill(0).map(() =>
          makeSession({ status: "completed", started_at: new Date(NOW - 30 * HOUR).toISOString() })
        ),
      ],
      NOW,
    );
    expect(k.done24h).toBe(9);
    expect(k.delta24h).toBe(50);
  });

  it("delta24h is null when the previous 24h window was empty", () => {
    const k = computeKpis(
      [
        makeSession({ status: "completed", started_at: new Date(NOW - 2 * HOUR).toISOString() }),
      ],
      NOW,
    );
    expect(k.delta24h).toBeNull();
  });

  it("does not double-count: a running session is in `running` only, not done/failed", () => {
    const k = computeKpis(
      [makeSession({ status: "running", started_at: new Date(NOW - HOUR).toISOString() })],
      NOW,
    );
    expect(k.running).toBe(1);
    expect(k.done24h).toBe(0);
    expect(k.failed24h).toBe(0);
  });

  it("ignores token counts for non-running sessions", () => {
    const k = computeKpis(
      [
        makeSession({ status: "completed", token_count: 9999 }),
        makeSession({ status: "running", token_count: 1500 }),
      ],
      NOW,
    );
    expect(k.runningTokens).toBe(1500);
  });
});
