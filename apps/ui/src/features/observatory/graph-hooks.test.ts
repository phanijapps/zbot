// ============================================================================
// graph-hooks — useGraphData / useGraphStats / useDistillationStatus /
// useEntityConnections / useBackfill
// ============================================================================

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mocks — transport (per-agent endpoints) + global fetch (cross-agent endpoints)
// ---------------------------------------------------------------------------

const mockGetGraphEntities = vi.fn();
const mockGetGraphRelationships = vi.fn();
const mockGetEntityNeighbors = vi.fn();

vi.mock("@/services/transport", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("@/services/transport");
  return {
    ...actual,
    getTransport: async () => ({
      getGraphEntities: mockGetGraphEntities,
      getGraphRelationships: mockGetGraphRelationships,
      getEntityNeighbors: mockGetEntityNeighbors,
      // graph-hooks reaches through transport.config to get the base URL.
      config: { httpUrl: "http://localhost:18791" },
    }),
  };
});

const fetchMock = vi.fn();
const realFetch = globalThis.fetch;

beforeEach(() => {
  vi.clearAllMocks();
  // Default fetch handler — returns the URL stamp so tests can assert call paths.
  fetchMock.mockImplementation((url: string) =>
    Promise.resolve({
      ok: true,
      status: 200,
      json: () => Promise.resolve({ url, entities: [], relationships: [] }),
      text: () => Promise.resolve(""),
      statusText: "OK",
    })
  );
  // @ts-expect-error — overriding global fetch for the test
  globalThis.fetch = fetchMock;
});

afterEach(() => {
  // Restore the real (MSW-installed) fetch so MSW teardown still works.
  globalThis.fetch = realFetch;
});

// We import after mocks so the module captures the stubs.
import {
  useGraphData,
  useGraphStats,
  useDistillationStatus,
  useEntityConnections,
  useBackfill,
} from "./graph-hooks";

// ---------------------------------------------------------------------------
// useGraphData
// ---------------------------------------------------------------------------

describe("useGraphData", () => {
  it("fetches via transport when agentId is provided (per-agent path)", async () => {
    mockGetGraphEntities.mockResolvedValue({
      success: true,
      data: { entities: [{ id: "e1" }] },
    });
    mockGetGraphRelationships.mockResolvedValue({
      success: true,
      data: { relationships: [{ id: "r1" }] },
    });
    const { result } = renderHook(() => useGraphData("agent-1"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.entities).toEqual([{ id: "e1" }]);
    expect(result.current.relationships).toEqual([{ id: "r1" }]);
    expect(mockGetGraphEntities).toHaveBeenCalledWith("agent-1", { limit: 200 });
    expect(mockGetGraphRelationships).toHaveBeenCalledWith("agent-1", { limit: 500 });
  });

  it("hits /api/graph/all/* endpoints when agentId is omitted (cross-agent path)", async () => {
    fetchMock.mockImplementation((url: string) => {
      if (url.includes("/api/graph/all/entities")) {
        return Promise.resolve({
          ok: true,
          status: 200,
          json: () => Promise.resolve({ entities: [{ id: "ent-x" }] }),
          text: () => Promise.resolve(""),
          statusText: "OK",
        });
      }
      return Promise.resolve({
        ok: true,
        status: 200,
        json: () => Promise.resolve({ relationships: [{ id: "rel-x" }], total: 1 }),
        text: () => Promise.resolve(""),
        statusText: "OK",
      });
    });
    const { result } = renderHook(() => useGraphData());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.entities).toEqual([{ id: "ent-x" }]);
    expect(result.current.relationships).toEqual([{ id: "rel-x" }]);
  });

  it("captures error message when entity fetch fails (per-agent)", async () => {
    mockGetGraphEntities.mockResolvedValue({
      success: false,
      error: "permission denied",
    });
    mockGetGraphRelationships.mockResolvedValue({
      success: true,
      data: { relationships: [] },
    });
    const { result } = renderHook(() => useGraphData("agent-1"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toMatch(/permission denied/);
  });

  it("refetch increments tick and re-runs the load", async () => {
    mockGetGraphEntities.mockResolvedValue({
      success: true,
      data: { entities: [] },
    });
    mockGetGraphRelationships.mockResolvedValue({
      success: true,
      data: { relationships: [] },
    });
    const { result } = renderHook(() => useGraphData("agent-1"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(mockGetGraphEntities).toHaveBeenCalledTimes(1);
    act(() => result.current.refetch());
    await waitFor(() => expect(mockGetGraphEntities).toHaveBeenCalledTimes(2));
  });
});

// ---------------------------------------------------------------------------
// useGraphStats
// ---------------------------------------------------------------------------

describe("useGraphStats", () => {
  it("hits GET /api/graph/stats and exposes the parsed stats", async () => {
    fetchMock.mockImplementation((url: string) => {
      if (url.endsWith("/api/graph/stats")) {
        return Promise.resolve({
          ok: true,
          status: 200,
          json: () => Promise.resolve({ facts: 10, entities: 5, relationships: 3, episodes: 2 }),
          text: () => Promise.resolve(""),
          statusText: "OK",
        });
      }
      return Promise.resolve({ ok: false, status: 500, statusText: "err", text: () => Promise.resolve("") });
    });
    const { result } = renderHook(() => useGraphStats());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.stats).toEqual({ facts: 10, entities: 5, relationships: 3, episodes: 2 });
    expect(result.current.error).toBeNull();
  });

  it("captures error message when /api/graph/stats returns non-ok", async () => {
    fetchMock.mockImplementation(() =>
      Promise.resolve({
        ok: false,
        status: 500,
        statusText: "Internal Server Error",
        text: () => Promise.resolve("server exploded"),
        json: () => Promise.resolve({}),
      })
    );
    const { result } = renderHook(() => useGraphStats());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toMatch(/server exploded/);
  });
});

// ---------------------------------------------------------------------------
// useDistillationStatus
// ---------------------------------------------------------------------------

describe("useDistillationStatus", () => {
  it("hits GET /api/distillation/status and exposes status + refetch", async () => {
    fetchMock.mockImplementation(() =>
      Promise.resolve({
        ok: true,
        status: 200,
        json: () => Promise.resolve({
          success_count: 3,
          failed_count: 0,
          skipped_count: 1,
          permanently_failed_count: 0,
        }),
        text: () => Promise.resolve(""),
        statusText: "OK",
      })
    );
    const { result } = renderHook(() => useDistillationStatus());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.status?.success_count).toBe(3);
    expect(typeof result.current.refetch).toBe("function");
  });
});

// ---------------------------------------------------------------------------
// useEntityConnections
// ---------------------------------------------------------------------------

describe("useEntityConnections", () => {
  it("returns null data and skips fetch when agentId or entityId is empty", async () => {
    const { result } = renderHook(() => useEntityConnections("", ""));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.data).toBeNull();
    expect(mockGetEntityNeighbors).not.toHaveBeenCalled();
  });

  it("calls transport.getEntityNeighbors with the IDs and exposes the data", async () => {
    mockGetEntityNeighbors.mockResolvedValue({
      success: true,
      data: { neighbors: [{ entity: { id: "n1" }, relationship: {}, direction: "outgoing" }] },
    });
    const { result } = renderHook(() => useEntityConnections("agent-1", "ent-1"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(mockGetEntityNeighbors).toHaveBeenCalledWith("agent-1", "ent-1", { limit: 50 });
    expect(result.current.data?.neighbors.length).toBe(1);
  });

  it("captures error when getEntityNeighbors returns failure", async () => {
    mockGetEntityNeighbors.mockResolvedValue({ success: false, error: "not found" });
    const { result } = renderHook(() => useEntityConnections("agent-1", "ent-1"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toMatch(/not found/);
  });
});

// ---------------------------------------------------------------------------
// useBackfill
// ---------------------------------------------------------------------------

describe("useBackfill", () => {
  it("returns isDone=true immediately when there are no undistilled sessions", async () => {
    fetchMock.mockImplementation(() =>
      Promise.resolve({
        ok: true,
        status: 200,
        json: () => Promise.resolve([]),
        text: () => Promise.resolve(""),
        statusText: "OK",
      })
    );
    const onComplete = vi.fn();
    const { result } = renderHook(() => useBackfill(onComplete));
    await act(async () => { await result.current.run(); });
    expect(result.current.isDone).toBe(true);
    expect(result.current.isRunning).toBe(false);
    expect(onComplete).toHaveBeenCalled();
  });

  it("triggers distillation for each session and reports progress", async () => {
    const sessions = [
      { session_id: "s1", agent_id: "agent-1" },
      { session_id: "s2", agent_id: "agent-1" },
    ];
    let undistilledCalled = false;
    fetchMock.mockImplementation((url: string) => {
      if (url.includes("/api/distillation/undistilled")) {
        undistilledCalled = true;
        return Promise.resolve({
          ok: true,
          status: 200,
          json: () => Promise.resolve(sessions),
          text: () => Promise.resolve(""),
          statusText: "OK",
        });
      }
      // /api/distillation/trigger/* — POST
      return Promise.resolve({
        ok: true,
        status: 200,
        json: () => Promise.resolve({}),
        text: () => Promise.resolve(""),
        statusText: "OK",
      });
    });
    const onComplete = vi.fn();
    const { result } = renderHook(() => useBackfill(onComplete));
    await act(async () => { await result.current.run(); });
    expect(undistilledCalled).toBe(true);
    expect(result.current.progress).toEqual({ current: 2, total: 2 });
    expect(result.current.isDone).toBe(true);
    expect(onComplete).toHaveBeenCalled();
  });

  it("captures error when /api/distillation/undistilled fetch fails", async () => {
    fetchMock.mockImplementation(() =>
      Promise.resolve({
        ok: false,
        status: 500,
        statusText: "boom",
        text: () => Promise.resolve("backend off"),
        json: () => Promise.resolve({}),
      })
    );
    const { result } = renderHook(() => useBackfill());
    await act(async () => { await result.current.run(); });
    expect(result.current.error).toMatch(/backend off/);
    expect(result.current.isRunning).toBe(false);
  });
});
