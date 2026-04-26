// ============================================================================
// useTraceSubscription — verifies the WS-based subscription replaces the old
// 3-second polling. The hook should:
//   • not subscribe when session is null
//   • not subscribe when session is not running (terminal trace, no updates)
//   • subscribe to the session's conversation_id when running
//   • call onEvent only for trace-relevant event types
//   • unsubscribe on unmount or when the session/status changes
// ============================================================================

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mock transport — capture subscribeConversation calls so we can fire events
// at the registered handler from tests.
// ---------------------------------------------------------------------------

interface CapturedSubscription {
  conversationId: string;
  onEvent: (e: { type: string; [k: string]: unknown }) => void;
  unsubscribe: ReturnType<typeof vi.fn>;
}

const subscriptions: CapturedSubscription[] = [];

const mockSubscribe = vi.fn(
  (conversationId: string, options: { onEvent: (e: { type: string }) => void }) => {
    const unsubscribe = vi.fn();
    subscriptions.push({
      conversationId,
      onEvent: options.onEvent,
      unsubscribe,
    });
    return unsubscribe;
  },
);

vi.mock("@/services/transport", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("@/services/transport");
  return {
    ...actual,
    getTransport: async () => ({
      subscribeConversation: mockSubscribe,
    }),
  };
});

// Import after mock so the module captures the stub.
import { useTraceSubscription } from "./useTraceSubscription";
import type { LogSession } from "@/services/transport/types";

function makeSession(overrides: Partial<LogSession> = {}): LogSession {
  return {
    session_id: "sess-1",
    conversation_id: "conv-1",
    agent_id: "agent:root",
    agent_name: "root",
    started_at: new Date().toISOString(),
    status: "running",
    token_count: 0,
    tool_call_count: 0,
    error_count: 0,
    child_session_ids: [],
    ...overrides,
  };
}

beforeEach(() => {
  subscriptions.length = 0;
  mockSubscribe.mockClear();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("useTraceSubscription", () => {
  it("does NOT subscribe when session is null", () => {
    renderHook(() => useTraceSubscription({ session: null, onEvent: vi.fn() }));
    expect(mockSubscribe).not.toHaveBeenCalled();
  });

  it("does NOT subscribe when the session is completed (terminal)", () => {
    renderHook(() =>
      useTraceSubscription({
        session: makeSession({ status: "completed" }),
        onEvent: vi.fn(),
      }),
    );
    expect(mockSubscribe).not.toHaveBeenCalled();
  });

  it("subscribes to the session's conversation_id when running", async () => {
    renderHook(() =>
      useTraceSubscription({
        session: makeSession({ conversation_id: "conv-7" }),
        onEvent: vi.fn(),
      }),
    );
    await waitFor(() => expect(mockSubscribe).toHaveBeenCalledTimes(1));
    expect(subscriptions[0].conversationId).toBe("conv-7");
  });

  it("invokes onEvent for tool_call events", async () => {
    const onEvent = vi.fn();
    renderHook(() => useTraceSubscription({ session: makeSession(), onEvent }));
    await waitFor(() => expect(subscriptions).toHaveLength(1));
    act(() => {
      subscriptions[0].onEvent({ type: "tool_call", timestamp: Date.now() });
    });
    expect(onEvent).toHaveBeenCalledTimes(1);
  });

  it("invokes onEvent for tool_result, delegation, agent_started, agent_completed, error, session_status_changed", async () => {
    const onEvent = vi.fn();
    renderHook(() => useTraceSubscription({ session: makeSession(), onEvent }));
    await waitFor(() => expect(subscriptions).toHaveLength(1));
    const handler = subscriptions[0].onEvent;
    const types = ["tool_result", "delegation", "agent_started", "agent_completed", "error", "session_status_changed"];
    act(() => { for (const t of types) handler({ type: t, timestamp: Date.now() }); });
    expect(onEvent).toHaveBeenCalledTimes(types.length);
  });

  it("ignores irrelevant event types (e.g., token, ping)", async () => {
    const onEvent = vi.fn();
    renderHook(() => useTraceSubscription({ session: makeSession(), onEvent }));
    await waitFor(() => expect(subscriptions).toHaveLength(1));
    act(() => {
      subscriptions[0].onEvent({ type: "token", timestamp: Date.now() });
      subscriptions[0].onEvent({ type: "ping", timestamp: Date.now() });
    });
    expect(onEvent).not.toHaveBeenCalled();
  });

  it("unsubscribes when the hook unmounts", async () => {
    const { unmount } = renderHook(() =>
      useTraceSubscription({ session: makeSession(), onEvent: vi.fn() }),
    );
    await waitFor(() => expect(subscriptions).toHaveLength(1));
    unmount();
    expect(subscriptions[0].unsubscribe).toHaveBeenCalledTimes(1);
  });

  it("re-subscribes when the conversation_id changes", async () => {
    const onEvent = vi.fn();
    const { rerender } = renderHook(
      ({ session }: { session: LogSession }) => useTraceSubscription({ session, onEvent }),
      { initialProps: { session: makeSession({ conversation_id: "conv-A" }) } },
    );
    await waitFor(() => expect(mockSubscribe).toHaveBeenCalledTimes(1));
    expect(subscriptions[0].conversationId).toBe("conv-A");

    rerender({ session: makeSession({ conversation_id: "conv-B" }) });
    await waitFor(() => expect(mockSubscribe).toHaveBeenCalledTimes(2));
    expect(subscriptions[1].conversationId).toBe("conv-B");
    // Old subscription was disposed.
    expect(subscriptions[0].unsubscribe).toHaveBeenCalled();
  });
});
