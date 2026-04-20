// =============================================================================
// flows.integration.test.tsx — hook-level integration tests for research-v2.
//
// Drives `useResearchSession` (and `useSessionsList` for the delete paths)
// through `renderHook` with a hand-rolled Transport mock. Negative scenarios
// cover the failure modes the plan enumerates (silent crash, orphan respond,
// sticky ward, malformed events, delegation chain, delete flow).
// =============================================================================

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { createElement, type PropsWithChildren } from "react";
import { MemoryRouter, Routes, Route } from "react-router-dom";

import type { ConversationEvent } from "@/services/transport/types";

// ---------------------------------------------------------------------------
// Hoisted mock state — lets vi.mock() reference across the suite.
// ---------------------------------------------------------------------------

const mockState = vi.hoisted(() => {
  return {
    currentTransport: null as unknown as {
      __pushEvent: (e: ConversationEvent) => void;
      [k: string]: unknown;
    } | null,
  };
});

vi.mock("@/services/transport", () => ({
  getTransport: async () => {
    if (!mockState.currentTransport) {
      throw new Error(
        "integration test forgot to install a mock transport before rendering",
      );
    }
    return mockState.currentTransport;
  },
}));

vi.mock("sonner", () => ({ toast: { error: vi.fn() } }));

// Imports under test — AFTER the mock registration so the mock wins.
import { useResearchSession } from "../useResearchSession";
import { useSessionsList } from "../useSessionsList";
import { makeMockTransport, ev, MOCK_SESSION_ID } from "./transport-mock";

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

const INITIAL_PATH = "/research-v2";

function routerWrapper(initialPath: string) {
  return function Wrapper({ children }: PropsWithChildren) {
    return createElement(
      MemoryRouter,
      { initialEntries: [initialPath] },
      createElement(
        Routes,
        null,
        createElement(Route, { path: "/research-v2", element: children }),
        createElement(Route, {
          path: "/research-v2/:sessionId",
          element: children,
        }),
      ),
    );
  };
}

function installTransport(
  opts: Parameters<typeof makeMockTransport>[0] = {},
): ReturnType<typeof makeMockTransport> {
  const t = makeMockTransport(opts);
  mockState.currentTransport = t as unknown as typeof mockState.currentTransport;
  return t;
}

/** Flush React state + microtasks. The hook chains a couple of awaits inside
 *  the subscription effect so we tick several times to settle. */
async function flush(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  });
}

beforeEach(() => {
  mockState.currentTransport = null;
});

afterEach(() => {
  vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// 1. Happy path — full single-turn flow
// ---------------------------------------------------------------------------

describe("useResearchSession — full flow integration", () => {
  it("drives a full single-turn flow: start → tool → respond → complete", async () => {
    const transport = installTransport();
    const { result } = renderHook(() => useResearchSession(), {
      wrapper: routerWrapper(INITIAL_PATH),
    });

    await act(async () => {
      await result.current.sendMessage("analyze Q4");
    });
    await waitFor(() =>
      expect(transport.calls.executeAgent).toHaveBeenCalled(),
    );

    await act(async () => {
      transport.__pushEvent(ev.invokeAccepted());
      transport.__pushEvent(ev.agentStarted("exec-1"));
      transport.__pushEvent(ev.wardChanged("stock-analysis"));
      transport.__pushEvent(ev.thinking("exec-1", "recall Q4 fundamentals"));
      transport.__pushEvent(ev.toolCall("exec-1", "shell", { command: "cat data.csv" }));
      transport.__pushEvent(ev.toolResult("exec-1", "shell", "..."));
      transport.__pushEvent(ev.respond("exec-1", "Q4 revenue was $X"));
      transport.__pushEvent(ev.agentCompleted("exec-1"));
      transport.__pushEvent(ev.turnComplete("exec-1"));
    });
    await flush();

    expect(result.current.state.sessionId).toBe(MOCK_SESSION_ID);
    expect(result.current.state.wardName).toBe("stock-analysis");
    expect(result.current.state.turns).toHaveLength(1);
    const turn = result.current.state.turns[0];
    expect(turn.id).toBe("exec-1");
    expect(turn.agentId).toBe("root");
    expect(turn.respond).toBe("Q4 revenue was $X");
    expect(turn.timeline.some((e) => e.kind === "thinking")).toBe(true);
    expect(turn.timeline.some((e) => e.kind === "tool_call")).toBe(true);
    expect(turn.timeline.some((e) => e.kind === "tool_result")).toBe(true);
    expect(turn.status).toBe("completed");
  });

  // -------------------------------------------------------------------------
  // 2. Silent crash inference (chat-v2 backlog B3)
  // -------------------------------------------------------------------------
  it("infers error when agent_completed arrives with no content and no respond", async () => {
    const transport = installTransport();
    const { result } = renderHook(() => useResearchSession(), {
      wrapper: routerWrapper(INITIAL_PATH),
    });

    await act(async () => {
      await result.current.sendMessage("bad prompt");
    });
    await act(async () => {
      transport.__pushEvent(ev.invokeAccepted());
      transport.__pushEvent(ev.agentStarted("exec-1"));
      // NO thinking, NO tool_call, NO respond — crashed silently.
      transport.__pushEvent(ev.turnComplete("exec-1"));
      transport.__pushEvent(ev.agentCompleted("exec-1"));
    });
    await flush();

    const turn = result.current.state.turns.find((t) => t.id === "exec-1");
    expect(turn?.status).toBe("error");
    expect(turn?.errorMessage ?? "").toMatch(/no output/i);
  });

  // -------------------------------------------------------------------------
  // 3. Orphan respond — AgentStarted never arrived
  // -------------------------------------------------------------------------
  it("renders a turn from a Respond event even without a preceding AgentStarted", async () => {
    const transport = installTransport();
    const { result } = renderHook(() => useResearchSession(), {
      wrapper: routerWrapper(INITIAL_PATH),
    });

    await act(async () => {
      await result.current.sendMessage("q");
    });
    await act(async () => {
      transport.__pushEvent(ev.invokeAccepted());
      // Skip agent_started — simulating packet loss / reconnect race.
      transport.__pushEvent(ev.respond("exec-orphan", "late reply"));
    });
    await flush();

    const turn = result.current.state.turns.find((t) => t.id === "exec-orphan");
    expect(turn).toBeTruthy();
    expect(turn?.respond).toBe("late reply");
  });

  // -------------------------------------------------------------------------
  // 4. Sticky ward — never reverts to null once set
  // -------------------------------------------------------------------------
  it("preserves sticky ward when later events omit ward_id", async () => {
    const transport = installTransport();
    const { result } = renderHook(() => useResearchSession(), {
      wrapper: routerWrapper(INITIAL_PATH),
    });

    await act(async () => {
      await result.current.sendMessage("q");
    });
    await act(async () => {
      transport.__pushEvent(ev.invokeAccepted());
      transport.__pushEvent(ev.wardChanged("stock-analysis"));
      // agent_started with no ward_id must NOT clear the sticky ward.
      transport.__pushEvent(ev.agentStarted("exec-1"));
    });
    await flush();

    expect(result.current.state.wardName).toBe("stock-analysis");
    expect(result.current.state.wardId).toBe("stock-analysis");
  });

  // -------------------------------------------------------------------------
  // 5. Malformed events are ignored (hook state stays quiescent)
  //
  // These shapes lack the fields that would trigger a turn-seeding action in
  // event-map.ts (they return `null` from the relevant mapper and the switch
  // default returns `null`). The reducer receives nothing and state.turns
  // stays at its pre-event count. `tool_call` is intentionally excluded:
  // mapToolCall falls back to `"tool"` as the tool name rather than dropping
  // the event, so an "empty" tool_call DOES seed a turn — that's by design
  // (see event-map.ts:toolNameOf fallback).
  // -------------------------------------------------------------------------
  const malformed: [string, ConversationEvent][] = [
    [
      "token with no delta and no content",
      { type: "token", execution_id: "e1" } as unknown as ConversationEvent,
    ],
    [
      "respond with no message and no content",
      { type: "respond", execution_id: "e1" } as unknown as ConversationEvent,
    ],
    [
      "ward_changed with no ward_id or ward.name",
      { type: "ward_changed" } as unknown as ConversationEvent,
    ],
    [
      "thinking with no content",
      { type: "thinking", execution_id: "e1" } as unknown as ConversationEvent,
    ],
    [
      "unknown event kind",
      { type: "foo_bar_baz" } as unknown as ConversationEvent,
    ],
  ];

  it.each(malformed)("ignores malformed event: %s", async (_, event) => {
    const transport = installTransport();
    const { result } = renderHook(() => useResearchSession(), {
      wrapper: routerWrapper(INITIAL_PATH),
    });
    await act(async () => {
      await result.current.sendMessage("q");
    });
    const beforeTurns = result.current.state.turns.length;
    await act(async () => {
      transport.__pushEvent(event);
    });
    await flush();
    expect(result.current.state.turns.length).toBe(beforeTurns);
  });

  it("does not crash on tool_call with no tool name (falls back to 'tool')", async () => {
    // Plan originally grouped this with malformed events, but event-map's
    // toolNameOf() default means the event still produces a valid TOOL_CALL
    // action with tool="tool". Assertion narrows to "no crash + turn seeded
    // with empty timeline entry" so we catch a future regression that makes
    // the event crash rather than degrade gracefully.
    const transport = installTransport();
    const { result } = renderHook(() => useResearchSession(), {
      wrapper: routerWrapper(INITIAL_PATH),
    });
    await act(async () => {
      await result.current.sendMessage("q");
    });
    await act(async () => {
      transport.__pushEvent({
        type: "tool_call",
        execution_id: "e1",
      } as unknown as ConversationEvent);
    });
    await flush();
    const turn = result.current.state.turns.find((t) => t.id === "e1");
    expect(turn).toBeTruthy();
    expect(turn?.timeline.length).toBe(1);
    expect(turn?.timeline[0].kind).toBe("tool_call");
  });

  // -------------------------------------------------------------------------
  // 6. Delegation chain — child turn nests under root
  // -------------------------------------------------------------------------
  it("nests a child turn under its parent via parentExecutionId", async () => {
    const transport = installTransport();
    const { result } = renderHook(() => useResearchSession(), {
      wrapper: routerWrapper(INITIAL_PATH),
    });

    await act(async () => {
      await result.current.sendMessage("plan and build");
    });
    await act(async () => {
      transport.__pushEvent(ev.invokeAccepted());
      transport.__pushEvent(ev.agentStarted("exec-root"));
      transport.__pushEvent(
        ev.delegationStarted("exec-root", "exec-writer", "writer-agent", "draft memo"),
      );
      transport.__pushEvent(ev.childAgentStarted("exec-writer", "exec-root", "writer-agent"));
      transport.__pushEvent(ev.respond("exec-writer", "memo draft"));
      transport.__pushEvent(ev.childAgentCompleted("exec-writer", "exec-root", "writer-agent"));
      transport.__pushEvent(ev.respond("exec-root", "done — see memo"));
      transport.__pushEvent(ev.agentCompleted("exec-root"));
    });
    await flush();

    const root = result.current.state.turns.find((t) => t.id === "exec-root");
    const child = result.current.state.turns.find((t) => t.id === "exec-writer");
    expect(root).toBeTruthy();
    expect(child).toBeTruthy();
    expect(child?.parentExecutionId).toBe("exec-root");
    expect(root?.respond).toContain("memo");
    expect(child?.respond).toBe("memo draft");
  });

  // -------------------------------------------------------------------------
  // 10. Resubscribe-storm regression guard
  // -------------------------------------------------------------------------
  it("does not resubscribe on unrelated re-renders (pillSink stability)", async () => {
    const transport = installTransport();
    const { result, rerender } = renderHook(() => useResearchSession(), {
      wrapper: routerWrapper(INITIAL_PATH),
    });

    await act(async () => {
      await result.current.sendMessage("q");
    });
    // Drive status → running so R14g's session-scope subscription attaches.
    await act(async () => {
      transport.__pushEvent(ev.invokeAccepted());
    });
    await flush();

    const baseline = transport.calls.subscribeConversation.mock.calls.length;
    // R14g: two subscriptions are expected (conv-id + session-id). 1 is also OK
    // when the state.sessionId hasn't been bound yet, but `invoke_accepted`
    // above guarantees we've reached the 2-subscription state.
    expect(baseline).toBeGreaterThanOrEqual(1);

    rerender();
    rerender();
    rerender();
    await flush();

    expect(transport.calls.subscribeConversation.mock.calls.length).toBe(baseline);
  });
});

// ---------------------------------------------------------------------------
// 7 / 8 / 9. deleteSession flow (useSessionsList)
// ---------------------------------------------------------------------------

describe("useSessionsList — delete flow integration", () => {
  it("invokes transport.deleteSession when the user confirms", async () => {
    const transport = installTransport();
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    const { result } = renderHook(() => useSessionsList(), {
      wrapper: routerWrapper(INITIAL_PATH),
    });
    await flush();

    await act(async () => {
      await result.current.deleteSession(MOCK_SESSION_ID);
    });

    expect(transport.calls.deleteSession).toHaveBeenCalledWith(MOCK_SESSION_ID);
    confirmSpy.mockRestore();
  });

  it("declines gracefully when confirm is cancelled", async () => {
    const transport = installTransport();
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
    const { result } = renderHook(() => useSessionsList(), {
      wrapper: routerWrapper(INITIAL_PATH),
    });
    await flush();

    await act(async () => {
      await result.current.deleteSession(MOCK_SESSION_ID);
    });

    expect(transport.calls.deleteSession).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });

  it("surfaces a backend 404 as a console error and does not refresh", async () => {
    const transport = installTransport();
    transport.calls.deleteSession.mockResolvedValueOnce({
      success: false,
      error: "Session not found",
    });
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);

    const { result } = renderHook(() => useSessionsList(), {
      wrapper: routerWrapper(INITIAL_PATH),
    });
    await flush();
    const refreshCallsBefore = transport.calls.listLogSessions.mock.calls.length;

    await act(async () => {
      await result.current.deleteSession("sess-missing");
    });

    expect(consoleSpy).toHaveBeenCalledWith(
      expect.stringContaining("Failed to delete"),
      expect.anything(),
    );
    // No post-delete refresh on error — list request count unchanged.
    expect(transport.calls.listLogSessions.mock.calls.length).toBe(
      refreshCallsBefore,
    );

    consoleSpy.mockRestore();
    confirmSpy.mockRestore();
  });
});
