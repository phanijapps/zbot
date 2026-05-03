// ============================================================================
// usePaths — fetch + module-level memoization tests
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { usePaths, __resetPathsCacheForTest, type Paths } from "./usePaths";

const SAMPLE: Paths = {
  vaultDir: "/home/pi/zbot",
  configDir: "/home/pi/zbot/config",
  logsDir: "/home/pi/zbot/logs",
  pluginsDir: "/home/pi/zbot/plugins",
  agentsDir: "/home/pi/zbot/agents",
  vaultDirDisplay: "~/zbot",
  configDirDisplay: "~/zbot/config",
  logsDirDisplay: "~/zbot/logs",
  pluginsDirDisplay: "~/zbot/plugins",
};

beforeEach(() => {
  __resetPathsCacheForTest();
});

describe("usePaths", () => {
  it("returns null on first render before fetch resolves", () => {
    globalThis.fetch = vi.fn(
      () => new Promise(() => {/* never resolves */}),
    ) as unknown as typeof fetch;

    const { result } = renderHook(() => usePaths());
    expect(result.current).toBeNull();
  });

  it("returns paths after fetch resolves", async () => {
    globalThis.fetch = vi.fn(async () =>
      new Response(JSON.stringify(SAMPLE), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    ) as unknown as typeof fetch;

    const { result } = renderHook(() => usePaths());

    await waitFor(() => expect(result.current).not.toBeNull());
    expect(result.current).toEqual(SAMPLE);
  });

  it("returns null when fetch fails", async () => {
    globalThis.fetch = vi.fn(async () =>
      new Response("nope", { status: 500 }),
    ) as unknown as typeof fetch;

    const { result } = renderHook(() => usePaths());

    // Wait for the promise to settle by re-rendering.
    await new Promise((r) => setTimeout(r, 10));
    expect(result.current).toBeNull();
  });

  it("only issues one fetch across multiple consumers (module-level cache)", async () => {
    const fetchMock = vi.fn(async () =>
      new Response(JSON.stringify(SAMPLE), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    globalThis.fetch = fetchMock as unknown as typeof fetch;

    const a = renderHook(() => usePaths());
    const b = renderHook(() => usePaths());
    const c = renderHook(() => usePaths());

    await waitFor(() => expect(a.result.current).not.toBeNull());
    await waitFor(() => expect(b.result.current).not.toBeNull());
    await waitFor(() => expect(c.result.current).not.toBeNull());

    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
