// ============================================================================
// useConnectionState — connection state subscription tests
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { ConnectionState, ConnectionStateCallback } from "@/services/transport/types";

const onConnectionStateChange = vi.fn<
  (cb: ConnectionStateCallback) => () => void
>();

vi.mock("@/services/transport", () => ({
  getTransport: async () => ({ onConnectionStateChange }),
}));

import { useConnectionState } from "./useConnectionState";

describe("useConnectionState", () => {
  beforeEach(() => {
    onConnectionStateChange.mockReset();
    onConnectionStateChange.mockImplementation((cb) => {
      cb({ status: "disconnected" });
      return () => {};
    });
  });

  it("starts with disconnected state", async () => {
    const { result } = renderHook(() => useConnectionState());
    expect(result.current.status).toBe("disconnected");
  });

  it("reflects connected state when transport notifies", async () => {
    let captured: ConnectionStateCallback | null = null;
    onConnectionStateChange.mockImplementation((cb) => {
      captured = cb;
      cb({ status: "disconnected" });
      return () => {};
    });

    const { result } = renderHook(() => useConnectionState());
    // Wait for the async setup to complete
    await new Promise((r) => setTimeout(r, 20));

    act(() => {
      captured?.({ status: "connected" });
    });

    expect(result.current.status).toBe("connected");
  });

  it("reflects reconnecting state with attempt info", async () => {
    let captured: ConnectionStateCallback | null = null;
    onConnectionStateChange.mockImplementation((cb) => {
      captured = cb;
      cb({ status: "disconnected" });
      return () => {};
    });

    const { result } = renderHook(() => useConnectionState());
    await new Promise((r) => setTimeout(r, 20));

    const reconnectingState: ConnectionState = {
      status: "reconnecting",
      attempt: 2,
      maxAttempts: 5,
    };

    act(() => {
      captured?.(reconnectingState);
    });

    expect(result.current.status).toBe("reconnecting");
  });

  it("calls unsubscribe on unmount", async () => {
    const unsubscribe = vi.fn();
    onConnectionStateChange.mockImplementation((cb) => {
      cb({ status: "disconnected" });
      return unsubscribe;
    });

    const { unmount } = renderHook(() => useConnectionState());
    await new Promise((r) => setTimeout(r, 20));

    act(() => unmount());
    expect(unsubscribe).toHaveBeenCalled();
  });
});
