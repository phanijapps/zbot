// =============================================================================
// useLogSessions — defensive filter for /logs sidebar.
//
// The gateway query was observed leaking subagent rows when their
// `agent_executions` row was missing (LEFT JOIN → NULL → filter passed).
// The Rust fix lives in `services/api-logs/src/repository.rs` (HAVING
// MAX(parent_session_id) IS NULL); this test locks the UI's defense-in-depth
// filter so a future gateway regression can't leak subagents into the list.
//
// See `memory-bank/defects/logs_root_only_subagents_leak.md`.
// =============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import type { Transport } from "@/services/transport";
import type { LogSession } from "@/services/transport/types";

const listLogSessions = vi.fn<Transport["listLogSessions"]>();

vi.mock("@/services/transport", () => ({
  getTransport: async () => ({ listLogSessions }),
}));

import { useLogSessions } from "./log-hooks";

function makeRow(overrides: Partial<LogSession> = {}): LogSession {
  return {
    session_id: "exec-root",
    conversation_id: "conv-root",
    agent_id: "root",
    agent_name: "root",
    title: null,
    started_at: new Date().toISOString(),
    ended_at: new Date().toISOString(),
    status: "completed",
    token_count: 0,
    tool_call_count: 0,
    error_count: 0,
    duration_ms: 0,
    parent_session_id: undefined,
    child_session_ids: [],
    mode: null,
    ...overrides,
  } as LogSession;
}

describe("useLogSessions root_only defensive filter", () => {
  beforeEach(() => {
    listLogSessions.mockReset();
  });

  it("drops subagent rows whose parent_session_id is set when root_only is on", async () => {
    listLogSessions.mockResolvedValue({
      success: true,
      data: [
        makeRow({ session_id: "exec-root", parent_session_id: undefined }),
        makeRow({
          session_id: "exec-child",
          agent_id: "builder-agent",
          parent_session_id: "exec-root",
        }),
        makeRow({
          session_id: "exec-child-2",
          agent_id: "planner-agent",
          parent_session_id: "exec-root",
        }),
      ],
    });

    const { result } = renderHook(() => useLogSessions({ root_only: true }));

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    const ids = result.current.sessions.map((s) => s.session_id);
    expect(ids).toEqual(["exec-root"]);
  });

  it("keeps subagent rows when root_only is off (no defensive filter applied)", async () => {
    listLogSessions.mockResolvedValue({
      success: true,
      data: [
        makeRow({ session_id: "exec-root" }),
        makeRow({ session_id: "exec-child", parent_session_id: "exec-root" }),
      ],
    });

    const { result } = renderHook(() => useLogSessions({ root_only: false }));

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    const ids = result.current.sessions.map((s) => s.session_id);
    expect(ids.sort()).toEqual(["exec-child", "exec-root"]);
  });

  it("treats empty-string parent_session_id as a root (matches gateway nullability)", async () => {
    listLogSessions.mockResolvedValue({
      success: true,
      data: [
        makeRow({ session_id: "exec-root-a", parent_session_id: undefined }),
        makeRow({ session_id: "exec-root-b", parent_session_id: "" }),
        makeRow({ session_id: "exec-child", parent_session_id: "exec-root-a" }),
      ],
    });

    const { result } = renderHook(() => useLogSessions({ root_only: true }));

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    const ids = result.current.sessions.map((s) => s.session_id).sort();
    expect(ids).toEqual(["exec-root-a", "exec-root-b"]);
  });
});
