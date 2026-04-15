import { describe, it, expect, vi } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { useWardContent, useHybridSearch, useWards } from "../hooks";

const searchMemoryHybrid = vi.fn(async () => ({
  success: true,
  data: {
    facts: { hits: [{ id: "f1", content: "hello", match_source: "fts" }], total: 1 },
    wiki: { hits: [], total: 0 },
    procedures: { hits: [], total: 0 },
    episodes: { hits: [], total: 0 },
  },
}));

vi.mock("@/services/transport", () => ({
  getTransport: async () => ({
    getWardContent: vi.fn(async (wardId: string) => ({
      success: true,
      data: {
        ward_id: wardId,
        summary: { title: wardId },
        facts: [
          {
            id: "f1",
            content: "x",
            category: "pattern",
            confidence: 0.9,
            created_at: "2026-04-15T10:00:00Z",
            age_bucket: "today",
          },
        ],
        wiki: [],
        procedures: [],
        episodes: [],
        counts: { facts: 1, wiki: 0, procedures: 0, episodes: 0 },
      },
    })),
    searchMemoryHybrid,
    listWards: vi.fn(async () => ({
      success: true,
      data: [{ id: "wardA", count: 3 }],
    })),
  }),
}));

describe("useWardContent", () => {
  it("loads and returns ward content", async () => {
    const { result } = renderHook(() => useWardContent("wardA"));
    await waitFor(() =>
      expect(result.current.data?.counts.facts).toBe(1),
    );
  });

  it("does not fetch when wardId is null", async () => {
    const { result } = renderHook(() => useWardContent(null));
    // Should stay null with no error.
    expect(result.current.data).toBeNull();
    expect(result.current.loading).toBe(false);
  });
});

describe("useHybridSearch", () => {
  it("debounces and produces data", async () => {
    vi.useFakeTimers();
    searchMemoryHybrid.mockClear();
    const { result } = renderHook(() =>
      useHybridSearch("hello", { limit: 10 }),
    );
    // Before debounce fires, no call should have been issued.
    expect(searchMemoryHybrid).not.toHaveBeenCalled();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(260);
    });
    vi.useRealTimers();
    await waitFor(() =>
      expect(result.current.data?.facts.hits).toHaveLength(1),
    );
    expect(searchMemoryHybrid).toHaveBeenCalledTimes(1);
  });

  it("returns null for empty query", async () => {
    const { result } = renderHook(() =>
      useHybridSearch("   ", { limit: 10 }),
    );
    expect(result.current.data).toBeNull();
  });
});

describe("useWards", () => {
  it("lists wards from transport", async () => {
    const { result } = renderHook(() => useWards());
    await waitFor(() => expect(result.current).toHaveLength(1));
    expect(result.current[0]).toEqual({ id: "wardA", count: 3 });
  });
});
