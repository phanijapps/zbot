// ============================================================================
// useVersion — fetch + module-level memoization tests
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { useVersion, __resetVersionCacheForTest } from "./useVersion";

const HEALTH_OK = { status: "ok", version: "2026.5.3.develop" };

beforeEach(() => {
  __resetVersionCacheForTest();
});

describe("useVersion", () => {
  it("returns null on first render before fetch resolves", () => {
    globalThis.fetch = vi.fn(
      () => new Promise(() => {/* never resolves */}),
    ) as unknown as typeof fetch;

    const { result } = renderHook(() => useVersion());
    expect(result.current).toBeNull();
  });

  it("returns the version after /api/health resolves", async () => {
    globalThis.fetch = vi.fn(async () =>
      new Response(JSON.stringify(HEALTH_OK), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    ) as unknown as typeof fetch;

    const { result } = renderHook(() => useVersion());

    await waitFor(() => expect(result.current).not.toBeNull());
    expect(result.current).toBe("2026.5.3.develop");
  });

  it("returns null when /api/health fails", async () => {
    globalThis.fetch = vi.fn(async () =>
      new Response("nope", { status: 500 }),
    ) as unknown as typeof fetch;

    const { result } = renderHook(() => useVersion());

    await new Promise((r) => setTimeout(r, 10));
    expect(result.current).toBeNull();
  });

  it("returns null when version field is empty/missing", async () => {
    globalThis.fetch = vi.fn(async () =>
      new Response(JSON.stringify({ status: "ok" }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    ) as unknown as typeof fetch;

    const { result } = renderHook(() => useVersion());
    await new Promise((r) => setTimeout(r, 10));
    expect(result.current).toBeNull();
  });

  it("only issues one fetch across multiple consumers", async () => {
    const fetchMock = vi.fn(async () =>
      new Response(JSON.stringify(HEALTH_OK), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    globalThis.fetch = fetchMock as unknown as typeof fetch;

    const a = renderHook(() => useVersion());
    const b = renderHook(() => useVersion());
    const c = renderHook(() => useVersion());

    await waitFor(() => expect(a.result.current).not.toBeNull());
    await waitFor(() => expect(b.result.current).not.toBeNull());
    await waitFor(() => expect(c.result.current).not.toBeNull());

    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
