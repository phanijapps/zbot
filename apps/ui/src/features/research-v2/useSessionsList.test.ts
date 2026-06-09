// ============================================================================
// useSessionsList — drawer list filter regression coverage
//
// The hook fans out two things research-v2's drawer relies on:
//   1. Drop chat-mode rows via the shared `isChatSession` predicate
//   2. Drop rows whose conversation_id is missing/empty
//
// Each axis used to be one inline-logic branch. These tests lock the
// behavior in so the next refactor of the predicate / shape can't drop
// chat-mode rows back into the research drawer.
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { createElement, StrictMode, type PropsWithChildren } from "react";
import type { MissionControlSessionSummary } from "@/services/transport/types";

// ---------------------------------------------------------------------------
// Transport mock — captures bounded summary + deleteSession invocations.
// ---------------------------------------------------------------------------

const listMissionControlSessionsMock = vi.fn<() => Promise<{ success: boolean; data?: MissionControlSessionSummary[]; error?: string }>>();
const deleteSessionMock = vi.fn<(id: string) => Promise<{ success: boolean; error?: string }>>();

vi.mock("@/services/transport", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("@/services/transport");
  return {
    ...actual,
    getTransport: async () => ({
      listMissionControlSessions: listMissionControlSessionsMock,
      deleteSession: deleteSessionMock,
    }),
  };
});

import { useSessionsList } from "./useSessionsList";

// ---------------------------------------------------------------------------
// Row factory — minimal shape the hook needs.
// ---------------------------------------------------------------------------

function row(overrides: Partial<MissionControlSessionSummary> & { conversation_id: string }): MissionControlSessionSummary {
  return {
    root_execution_id: `exec-${overrides.conversation_id}`,
    root_agent_id: "root",
    source: "web",
    title: "test session",
    created_at: "2026-04-24T09:59:00Z",
    started_at: "2026-04-24T10:00:00Z",
    status: "completed",
    total_tokens_in: 0,
    total_tokens_out: 0,
    subagent_count: 0,
    ...overrides,
  };
}

beforeEach(() => {
  listMissionControlSessionsMock.mockReset();
  deleteSessionMock.mockReset();
});

describe("useSessionsList — research drawer filter", () => {
  it("does not auto-load while disabled", async () => {
    listMissionControlSessionsMock.mockResolvedValue({ success: true, data: [] });
    const { result } = renderHook(() => useSessionsList({ enabled: false }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(listMissionControlSessionsMock).not.toHaveBeenCalled();
  });

  it("coalesces StrictMode mount refresh into one bounded request", async () => {
    listMissionControlSessionsMock.mockResolvedValue({
      success: true,
      data: [row({ conversation_id: "sess-root" })],
    });
    const wrapper = ({ children }: PropsWithChildren) =>
      createElement(StrictMode, null, children);

    const { result } = renderHook(() => useSessionsList(), { wrapper });
    await waitFor(() => expect(result.current.sessions.length).toBe(1));

    expect(listMissionControlSessionsMock).toHaveBeenCalledTimes(1);
    expect(listMissionControlSessionsMock).toHaveBeenCalledWith({ limit: 100 });
  });

  it("returns empty list when transport fails", async () => {
    listMissionControlSessionsMock.mockResolvedValue({ success: false, error: "boom" });
    const { result } = renderHook(() => useSessionsList());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.sessions).toEqual([]);
  });

  it("excludes chat-mode sessions (mode='fast')", async () => {
    listMissionControlSessionsMock.mockResolvedValue({
      success: true,
      data: [
        row({ conversation_id: "sess-research-1", title: "Q4 analysis", mode: null }),
        row({ conversation_id: "sess-chat-abc", title: "hi", mode: "fast" }),
      ],
    });
    const { result } = renderHook(() => useSessionsList());
    await waitFor(() => expect(result.current.sessions.length).toBe(1));
    expect(result.current.sessions[0].id).toBe("sess-research-1");
    expect(result.current.sessions[0].title).toBe("Q4 analysis");
  });

  it("excludes chat-mode sessions (mode='chat')", async () => {
    listMissionControlSessionsMock.mockResolvedValue({
      success: true,
      data: [
        row({ conversation_id: "sess-r", mode: "deep" }),
        row({ conversation_id: "sess-c", mode: "chat" }),
      ],
    });
    const { result } = renderHook(() => useSessionsList());
    await waitFor(() => expect(result.current.sessions.length).toBe(1));
    expect(result.current.sessions[0].id).toBe("sess-r");
  });

  it("falls back to sess-chat- prefix when mode field is absent (legacy daemon)", async () => {
    listMissionControlSessionsMock.mockResolvedValue({
      success: true,
      data: [
        row({ conversation_id: "sess-research-old" }), // no mode
        row({ conversation_id: "sess-chat-old" }),     // no mode but legacy prefix
      ],
    });
    const { result } = renderHook(() => useSessionsList());
    await waitFor(() => expect(result.current.sessions.length).toBe(1));
    expect(result.current.sessions[0].id).toBe("sess-research-old");
  });

  it("loads bounded root summaries from the mission-control endpoint", async () => {
    listMissionControlSessionsMock.mockResolvedValue({
      success: true,
      data: [
        row({ conversation_id: "sess-root" }),
      ],
    });
    const { result } = renderHook(() => useSessionsList());
    await waitFor(() => expect(result.current.sessions.length).toBe(1));
    expect(result.current.sessions[0].id).toBe("sess-root");
    expect(listMissionControlSessionsMock).toHaveBeenCalledWith({ limit: 100 });
  });

  it("drops rows with empty conversation_id", async () => {
    listMissionControlSessionsMock.mockResolvedValue({
      success: true,
      data: [
        row({ conversation_id: "sess-good" }),
        row({ conversation_id: "" }),
      ],
    });
    const { result } = renderHook(() => useSessionsList());
    await waitFor(() => expect(result.current.sessions.length).toBe(1));
    expect(result.current.sessions[0].id).toBe("sess-good");
  });

  it("synthesizes a 'New research · HH:MM' title when row has no title", async () => {
    listMissionControlSessionsMock.mockResolvedValue({
      success: true,
      data: [
        row({
          conversation_id: "sess-untitled",
          title: undefined,
          started_at: "2026-04-24T15:42:00Z",
        }),
      ],
    });
    const { result } = renderHook(() => useSessionsList());
    await waitFor(() => expect(result.current.sessions.length).toBe(1));
    expect(result.current.sessions[0].title).toMatch(/^New research · \d{2}:\d{2}$/);
  });

  it("maps wire status 'crashed' / 'completed' / 'paused' onto the SessionSummary status enum", async () => {
    // The wire `SessionStatus` enum is narrower than the strings the
    // hook accepts at runtime (it tolerates 'paused' and 'crashed' too).
    // Cast through `as unknown` to feed the broader strings without
    // tightening the SessionStatus type just for this test.
    listMissionControlSessionsMock.mockResolvedValue({
      success: true,
      data: [
        row({ conversation_id: "s-running", status: "running" }),
        row({ conversation_id: "s-complete", status: "completed" }),
        row({ conversation_id: "s-paused", status: "paused" }),
        row({ conversation_id: "s-error", status: "crashed" }),
      ],
    });
    const { result } = renderHook(() => useSessionsList());
    await waitFor(() => expect(result.current.sessions.length).toBe(4));
    const byId = Object.fromEntries(
      result.current.sessions.map((s) => [s.id, s.status]),
    );
    expect(byId).toEqual({
      "s-running": "running",
      "s-complete": "complete",
      "s-paused": "paused",
      "s-error": "crashed", // 'error' wire string maps to 'crashed' summary
    });
  });

  it("deleteSession honors the confirm dialog and skips the network call when cancelled", async () => {
    listMissionControlSessionsMock.mockResolvedValue({ success: true, data: [] });
    window.confirm = vi.fn(() => false);
    const { result } = renderHook(() => useSessionsList());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.deleteSession("sess-x");
    });

    expect(deleteSessionMock).not.toHaveBeenCalled();
  });

  it("deleteSession fires onAfterDelete only on successful transport delete", async () => {
    listMissionControlSessionsMock.mockResolvedValue({ success: true, data: [] });
    deleteSessionMock.mockResolvedValue({ success: true });
    window.confirm = vi.fn(() => true);
    const onAfterDelete = vi.fn();

    const { result } = renderHook(() => useSessionsList({ onAfterDelete }));
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.deleteSession("sess-x");
    });

    expect(deleteSessionMock).toHaveBeenCalledWith("sess-x");
    expect(onAfterDelete).toHaveBeenCalledWith("sess-x");
  });

  it("deleteSession does NOT fire onAfterDelete when transport returns failure", async () => {
    listMissionControlSessionsMock.mockResolvedValue({ success: true, data: [] });
    deleteSessionMock.mockResolvedValue({ success: false, error: "denied" });
    window.confirm = vi.fn(() => true);
    const onAfterDelete = vi.fn();

    const { result } = renderHook(() => useSessionsList({ onAfterDelete }));
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.deleteSession("sess-x");
    });

    expect(onAfterDelete).not.toHaveBeenCalled();
  });
});
