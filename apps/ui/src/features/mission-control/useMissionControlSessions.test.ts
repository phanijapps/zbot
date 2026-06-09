import { renderHook, waitFor } from "@testing-library/react";
import { createElement, StrictMode, type PropsWithChildren } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  buildTokenIndexFromSummaries,
  summariesToLogSessions,
  useMissionControlSessions,
} from "./useMissionControlSessions";
import type { MissionControlSessionSummary } from "@/services/transport/types";
import type { Transport } from "@/services/transport/interface";

const listMissionControlSessions = vi.fn<Transport["listMissionControlSessions"]>();
const listSessionsFull = vi.fn<Transport["listSessionsFull"]>();

vi.mock("@/services/transport", () => ({
  getTransport: async () => ({
    listMissionControlSessions,
    listSessionsFull,
  }),
}));

function makeSummary(overrides: Partial<MissionControlSessionSummary> = {}): MissionControlSessionSummary {
  return {
    conversation_id: "sess-1",
    root_execution_id: "exec-root-1",
    status: "running",
    source: "web",
    root_agent_id: "root-agent",
    title: "Investigate performance",
    created_at: "2026-06-09T10:00:00Z",
    started_at: "2026-06-09T10:01:00Z",
    completed_at: undefined,
    total_tokens_in: 1200,
    total_tokens_out: 300,
    subagent_count: 1,
    mode: "deep",
    ...overrides,
  };
}

beforeEach(() => {
  listMissionControlSessions.mockReset();
  listSessionsFull.mockReset();
});

describe("summariesToLogSessions", () => {
  it("maps summary rows to LogSession-compatible root execution rows", () => {
    const rows = summariesToLogSessions([makeSummary()]);
    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({
      session_id: "exec-root-1",
      conversation_id: "sess-1",
      agent_id: "root-agent",
      status: "running",
      token_count: 1500,
      child_session_ids: [],
      subagent_count: 1,
    });
  });

  it("maps crashed summaries to error rows", () => {
    const rows = summariesToLogSessions([makeSummary({ status: "crashed" })]);
    expect(rows[0].status).toBe("error");
    expect(rows[0].error_count).toBe(1);
  });
});

describe("buildTokenIndexFromSummaries", () => {
  it("keys aggregate totals by root execution id without list payload execution slices", () => {
    const index = buildTokenIndexFromSummaries([makeSummary()]);
    expect(index.byRootExecId.get("exec-root-1")).toEqual({
      in: 1200,
      out: 300,
      total: 1500,
      status: "running",
    });
    expect(index.executionsByRootExecId.get("exec-root-1")).toEqual([]);
  });
});

describe("useMissionControlSessions", () => {
  it("loads bounded mission-control summaries and does not call sessions/full", async () => {
    listMissionControlSessions.mockResolvedValue({
      success: true,
      data: [makeSummary()],
    });

    const { result } = renderHook(() => useMissionControlSessions(50));

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(listMissionControlSessions).toHaveBeenCalledWith({ limit: 50 });
    expect(listSessionsFull).not.toHaveBeenCalled();
    expect(result.current.sessions[0].session_id).toBe("exec-root-1");
    expect(result.current.tokenIndex.byRootExecId.get("exec-root-1")?.total).toBe(1500);
  });

  it("coalesces StrictMode mount into one bounded summary request", async () => {
    listMissionControlSessions.mockResolvedValue({
      success: true,
      data: [makeSummary()],
    });
    const wrapper = ({ children }: PropsWithChildren) => createElement(StrictMode, null, children);

    const { result } = renderHook(() => useMissionControlSessions(50), { wrapper });

    await waitFor(() => expect(result.current.sessions).toHaveLength(1));
    expect(listMissionControlSessions).toHaveBeenCalledTimes(1);
    expect(listMissionControlSessions).toHaveBeenCalledWith({ limit: 50 });
  });
});
