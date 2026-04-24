// ============================================================================
// useSessionsList — drawer list filter regression coverage
//
// The hook fans out three things research-v2's drawer relies on:
//   1. Drop subagent rows (parent_session_id non-empty)
//   2. Drop chat-mode rows via the shared `isChatSession` predicate
//   3. Drop rows whose conversation_id is missing/empty (rowToSummary → null)
//
// Each axis used to be one inline-logic branch. These tests lock the
// behavior in so the next refactor of the predicate / shape can't drop
// chat-mode rows back into the research drawer.
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import type { LogSession } from "@/services/transport/types";

// ---------------------------------------------------------------------------
// Transport mock — captures listLogSessions + deleteSession invocations.
// ---------------------------------------------------------------------------

const listLogSessionsMock = vi.fn<() => Promise<{ success: boolean; data?: LogSession[]; error?: string }>>();
const deleteSessionMock = vi.fn<(id: string) => Promise<{ success: boolean; error?: string }>>();

vi.mock("@/services/transport", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("@/services/transport");
  return {
    ...actual,
    getTransport: async () => ({
      listLogSessions: listLogSessionsMock,
      deleteSession: deleteSessionMock,
    }),
  };
});

import { useSessionsList } from "./useSessionsList";

// ---------------------------------------------------------------------------
// Row factory — minimal shape the hook needs.
// ---------------------------------------------------------------------------

function row(overrides: Partial<LogSession> & { conversation_id: string }): LogSession {
  return {
    session_id: `exec-${overrides.conversation_id}`,
    agent_id: "root",
    agent_name: "root",
    title: "test session",
    started_at: "2026-04-24T10:00:00Z",
    status: "completed",
    token_count: 0,
    tool_call_count: 0,
    error_count: 0,
    child_session_ids: [],
    ...overrides,
  };
}

beforeEach(() => {
  listLogSessionsMock.mockReset();
  deleteSessionMock.mockReset();
});

describe("useSessionsList — research drawer filter", () => {
  it("returns empty list when transport fails", async () => {
    listLogSessionsMock.mockResolvedValue({ success: false, error: "boom" });
    const { result } = renderHook(() => useSessionsList());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.sessions).toEqual([]);
  });

  it("excludes chat-mode sessions (mode='fast')", async () => {
    listLogSessionsMock.mockResolvedValue({
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
    listLogSessionsMock.mockResolvedValue({
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
    listLogSessionsMock.mockResolvedValue({
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

  it("excludes subagent (child) rows", async () => {
    listLogSessionsMock.mockResolvedValue({
      success: true,
      data: [
        row({ conversation_id: "sess-root" }),
        row({
          conversation_id: "sess-root-sub-abc",
          parent_session_id: "sess-root",
        }),
      ],
    });
    const { result } = renderHook(() => useSessionsList());
    await waitFor(() => expect(result.current.sessions.length).toBe(1));
    expect(result.current.sessions[0].id).toBe("sess-root");
  });

  it("drops rows with empty conversation_id (rowToSummary → null)", async () => {
    listLogSessionsMock.mockResolvedValue({
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
    listLogSessionsMock.mockResolvedValue({
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

  it("maps wire status 'error' / 'completed' / 'paused' onto the SessionSummary status enum", async () => {
    listLogSessionsMock.mockResolvedValue({
      success: true,
      data: [
        row({ conversation_id: "s-running", status: "running" }),
        row({ conversation_id: "s-complete", status: "completed" }),
        row({ conversation_id: "s-paused", status: "paused" }),
        row({ conversation_id: "s-error", status: "error" }),
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
    listLogSessionsMock.mockResolvedValue({ success: true, data: [] });
    window.confirm = vi.fn(() => false);
    const { result } = renderHook(() => useSessionsList());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.deleteSession("sess-x");
    });

    expect(deleteSessionMock).not.toHaveBeenCalled();
  });

  it("deleteSession fires onAfterDelete only on successful transport delete", async () => {
    listLogSessionsMock.mockResolvedValue({ success: true, data: [] });
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
    listLogSessionsMock.mockResolvedValue({ success: true, data: [] });
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
