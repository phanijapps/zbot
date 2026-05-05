// =============================================================================
// mission-hooks.test — useRecentSessions only
//
// The previous file's other ~30 describes covered the legacy
// `useMissionControl` reducer + `__testInternals` handlers, which were
// deleted along with the rest of the dead chat/ subsystem.
// =============================================================================

import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import type { LogSession } from "@/services/transport/types";
import { useRecentSessions } from "./mission-hooks";

const listLogSessionsMock = vi.fn<
  (filter?: { limit?: number; root_only?: boolean }) => Promise<{
    success: boolean;
    data?: LogSession[];
    error?: string;
  }>
>();

vi.mock("@/services/transport", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("@/services/transport");
  return {
    ...actual,
    getTransport: async () => ({
      listLogSessions: listLogSessionsMock,
    }),
  };
});

function makeRow(overrides: Partial<LogSession> & { conversation_id: string }): LogSession {
  return {
    session_id: `exec-${overrides.conversation_id}`,
    agent_id: "root",
    agent_name: "root",
    title: "test",
    started_at: "2026-04-24T10:00:00Z",
    status: "completed",
    token_count: 0,
    tool_call_count: 0,
    error_count: 0,
    child_session_ids: [],
    ...overrides,
  } as unknown as LogSession;
}

describe("useRecentSessions", () => {
  beforeEach(() => listLogSessionsMock.mockReset());

  it("requests limit=5 when no exclude predicate is provided", async () => {
    listLogSessionsMock.mockResolvedValue({
      success: true,
      data: Array.from({ length: 3 }, (_, i) => makeRow({ conversation_id: `s-${i}` })),
    });

    const { result } = renderHook(() => useRecentSessions());
    await waitFor(() => expect(result.current.sessions.length).toBe(3));
    expect(listLogSessionsMock).toHaveBeenCalledWith({ limit: 5, root_only: true });
  });

  it("over-fetches limit=20 when an exclude predicate is provided", async () => {
    listLogSessionsMock.mockResolvedValue({ success: true, data: [] });

    renderHook(() => useRecentSessions({ exclude: () => false }));
    await waitFor(() => expect(listLogSessionsMock).toHaveBeenCalled());
    expect(listLogSessionsMock).toHaveBeenCalledWith({ limit: 20, root_only: true });
  });

  it("filters via the exclude predicate and caps the final slice at 5", async () => {
    // 12 rows; exclude every odd-indexed one. Survivors = 6 → must cap to 5.
    listLogSessionsMock.mockResolvedValue({
      success: true,
      data: Array.from({ length: 12 }, (_, i) => makeRow({ conversation_id: `s-${i}` })),
    });
    const exclude = (row: LogSession) =>
      Number.parseInt(row.conversation_id.split("-")[1], 10) % 2 === 1;

    const { result } = renderHook(() => useRecentSessions({ exclude }));
    await waitFor(() => expect(result.current.sessions.length).toBe(5));
    for (const s of result.current.sessions) {
      const idx = Number.parseInt(s.conversation_id.split("-")[1], 10);
      expect(idx % 2).toBe(0); // only even-indexed survived
    }
  });

  it("keeps sessions empty when transport reports failure", async () => {
    listLogSessionsMock.mockResolvedValue({ success: false, error: "boom" });

    const { result } = renderHook(() => useRecentSessions());
    await waitFor(() => expect(listLogSessionsMock).toHaveBeenCalled());
    expect(result.current.sessions).toEqual([]);
  });

  it("keeps sessions empty when transport returns success but no data", async () => {
    listLogSessionsMock.mockResolvedValue({ success: true });

    const { result } = renderHook(() => useRecentSessions());
    await waitFor(() => expect(listLogSessionsMock).toHaveBeenCalled());
    expect(result.current.sessions).toEqual([]);
  });
});
