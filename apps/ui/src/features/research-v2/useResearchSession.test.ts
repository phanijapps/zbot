// =============================================================================
// useResearchSession — R14a tests
//
// The bug this suite guards against: in a brand-new research session the UI
// only subscribed AFTER `state.conversationId` was set, and that was only set
// by an event that required the subscription to be live → events landed in the
// gap and the UI never updated. The fix (this suite): sendMessage subscribes
// imperatively BEFORE the invoke, using a client-minted conv_id.
// =============================================================================

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { createElement, StrictMode, type PropsWithChildren } from "react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import type { Transport } from "@/services/transport";

// ---------------------------------------------------------------------------
// Transport mock — per-test spies for order assertions
// ---------------------------------------------------------------------------

const subscribeConversation = vi.fn<Transport["subscribeConversation"]>();
const executeAgent = vi.fn<Transport["executeAgent"]>();
const stopAgent = vi.fn<Transport["stopAgent"]>();
const getSessionMessages = vi.fn<Transport["getSessionMessages"]>();
const unsubscribeSpy = vi.fn<() => void>();
// Ordered log of all transport calls to assert subscribe-before-invoke.
const callLog: string[] = [];

vi.mock("@/services/transport", () => ({
  getTransport: async () => ({
    subscribeConversation,
    executeAgent,
    stopAgent,
    getSessionMessages,
  }),
}));

// ---------------------------------------------------------------------------
// Import AFTER the mock is registered.
// ---------------------------------------------------------------------------
import { useResearchSession } from "./useResearchSession";

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

const TEST_INITIAL_PATH = "/research-v2";

function routerWrapper(initialPath: string) {
  return function Wrapper({ children }: PropsWithChildren) {
    return createElement(
      MemoryRouter,
      { initialEntries: [initialPath] },
      createElement(
        Routes,
        null,
        createElement(Route, { path: "/research-v2", element: children as React.ReactElement }),
        createElement(Route, {
          path: "/research-v2/:sessionId",
          element: children as React.ReactElement,
        })
      )
    );
  };
}

function strictRouterWrapper(initialPath: string) {
  const Inner = routerWrapper(initialPath);
  return function StrictWrapper({ children }: PropsWithChildren) {
    return createElement(
      StrictMode,
      null,
      createElement(Inner, null, children as React.ReactElement)
    );
  };
}

beforeEach(() => {
  callLog.length = 0;
  subscribeConversation.mockReset();
  executeAgent.mockReset();
  stopAgent.mockReset();
  getSessionMessages.mockReset();
  unsubscribeSpy.mockReset();

  subscribeConversation.mockImplementation((convId: string) => {
    callLog.push(`subscribe:${convId}`);
    return unsubscribeSpy;
  });
  executeAgent.mockImplementation(
    async (_agent, convId /* , msg, sessionId */) => {
      callLog.push(`invoke:${convId}`);
      return { success: true, data: { conversationId: convId } };
    }
  );
  stopAgent.mockResolvedValue({ success: true, data: undefined });
  getSessionMessages.mockResolvedValue({ success: true, data: [] });
});

afterEach(() => {
  vi.clearAllMocks();
});

function lastSubscribedConvId(): string {
  const calls = subscribeConversation.mock.calls;
  expect(calls.length).toBeGreaterThan(0);
  return calls[calls.length - 1][0];
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("useResearchSession — subscription ordering (R14a)", () => {
  it("sendMessage subscribes BEFORE invoke with the SAME convId", async () => {
    const { result } = renderHook(() => useResearchSession(), {
      wrapper: routerWrapper(TEST_INITIAL_PATH),
    });

    await act(async () => {
      await result.current.sendMessage("hello");
    });

    expect(subscribeConversation).toHaveBeenCalledTimes(1);
    expect(executeAgent).toHaveBeenCalledTimes(1);

    const subConvId = subscribeConversation.mock.calls[0][0];
    const invokeConvId = executeAgent.mock.calls[0][1];
    expect(subConvId).toBe(invokeConvId);

    // Subscribe appears before invoke in the ordered log.
    const subIdx = callLog.findIndex((c) => c.startsWith("subscribe:"));
    const invIdx = callLog.findIndex((c) => c.startsWith("invoke:"));
    expect(subIdx).toBeGreaterThanOrEqual(0);
    expect(invIdx).toBeGreaterThan(subIdx);
  });

  it("uses a research-prefixed client-minted convId for a new session", async () => {
    const { result } = renderHook(() => useResearchSession(), {
      wrapper: routerWrapper(TEST_INITIAL_PATH),
    });
    await act(async () => {
      await result.current.sendMessage("first");
    });
    const convId = lastSubscribedConvId();
    expect(convId).toMatch(/^research-/);
  });

  it("second sendMessage on same session does NOT re-subscribe", async () => {
    const { result } = renderHook(() => useResearchSession(), {
      wrapper: routerWrapper(TEST_INITIAL_PATH),
    });
    await act(async () => {
      await result.current.sendMessage("one");
    });
    const firstConvId = lastSubscribedConvId();

    // The hook's "running" status guard blocks back-to-back sends; deliver a
    // terminal error event through the captured onEvent to clear status and
    // unblock the follow-up send. Real lifecycle would clear it via
    // session_complete; error path works the same for this test's purpose.
    const onEvent = subscribeConversation.mock.calls[0][1].onEvent;
    act(() => {
      onEvent({ type: "error", message: "simulated" } as never);
    });

    await act(async () => {
      await result.current.sendMessage("two");
    });

    expect(subscribeConversation).toHaveBeenCalledTimes(1);
    expect(executeAgent).toHaveBeenCalledTimes(2);
    // Both invokes used the same convId.
    expect(executeAgent.mock.calls[0][1]).toBe(firstConvId);
    expect(executeAgent.mock.calls[1][1]).toBe(firstConvId);
  });

  it("startNewResearch unsubscribes and the next send subscribes with a NEW convId", async () => {
    const { result } = renderHook(() => useResearchSession(), {
      wrapper: routerWrapper(TEST_INITIAL_PATH),
    });
    await act(async () => {
      await result.current.sendMessage("one");
    });
    const firstConvId = lastSubscribedConvId();

    act(() => {
      result.current.startNewResearch();
    });
    expect(unsubscribeSpy).toHaveBeenCalledTimes(1);

    await act(async () => {
      await result.current.sendMessage("two");
    });
    expect(subscribeConversation).toHaveBeenCalledTimes(2);
    const secondConvId = lastSubscribedConvId();
    expect(secondConvId).not.toBe(firstConvId);
    expect(secondConvId).toMatch(/^research-/);
  });

  it("unmount calls the stored unsubscribe", async () => {
    const { result, unmount } = renderHook(() => useResearchSession(), {
      wrapper: routerWrapper(TEST_INITIAL_PATH),
    });
    await act(async () => {
      await result.current.sendMessage("one");
    });
    expect(unsubscribeSpy).not.toHaveBeenCalled();

    unmount();

    expect(unsubscribeSpy).toHaveBeenCalledTimes(1);
  });

  it("unmount without any sendMessage does NOT crash", () => {
    const { unmount } = renderHook(() => useResearchSession(), {
      wrapper: routerWrapper(TEST_INITIAL_PATH),
    });
    expect(() => unmount()).not.toThrow();
    expect(unsubscribeSpy).not.toHaveBeenCalled();
  });

  it("StrictMode double-mount: subscribe called only once after one send", async () => {
    const { result } = renderHook(() => useResearchSession(), {
      wrapper: strictRouterWrapper(TEST_INITIAL_PATH),
    });
    await act(async () => {
      await result.current.sendMessage("hi");
    });
    // StrictMode double-invokes effects but sendMessage guards on
    // subscribedConvIdRef — the subscribe must fire exactly once.
    expect(subscribeConversation).toHaveBeenCalledTimes(1);
    expect(executeAgent).toHaveBeenCalledTimes(1);
  });

  it("hydrate + sendMessage: subscribe fires with a fresh convId; invoke carries the sessionId", async () => {
    const EXISTING_SESSION = "sess-existing-123";
    getSessionMessages.mockResolvedValueOnce({
      success: true,
      data: [
        {
          id: "m1",
          role: "user",
          content: "old prompt",
          created_at: "2026-04-10T00:00:00Z",
          execution_id: "exec-old",
          agent_id: "root",
          delegation_type: "root",
        },
      ],
    });

    const { result } = renderHook(() => useResearchSession(), {
      wrapper: routerWrapper(`/research-v2/${EXISTING_SESSION}`),
    });

    // Let hydrate effect flush.
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(getSessionMessages).toHaveBeenCalledWith(
      EXISTING_SESSION,
      expect.objectContaining({ scope: "root" })
    );

    await act(async () => {
      await result.current.sendMessage("follow-up");
    });

    expect(subscribeConversation).toHaveBeenCalledTimes(1);
    const convId = lastSubscribedConvId();
    expect(convId).toMatch(/^research-/);

    // executeAgent gets (agentId, convId, message, sessionId, mode).
    const invokeArgs = executeAgent.mock.calls[0];
    expect(invokeArgs[1]).toBe(convId);
    expect(invokeArgs[3]).toBe(EXISTING_SESSION);
  });

  it("error path: failed invoke dispatches ERROR but keeps the subscription", async () => {
    executeAgent.mockImplementationOnce(async (_agent, convId) => {
      callLog.push(`invoke:${convId}`);
      return { success: false, error: "boom" };
    });
    const { result } = renderHook(() => useResearchSession(), {
      wrapper: routerWrapper(TEST_INITIAL_PATH),
    });
    await act(async () => {
      await result.current.sendMessage("hi");
    });
    expect(result.current.state.status).toBe("error");
    // Subscription is intact so the user can retry/observe.
    expect(unsubscribeSpy).not.toHaveBeenCalled();
  });
});

