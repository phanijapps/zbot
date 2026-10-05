// ============================================================================
// ObservatoryPage — toolbar, loading/error/empty states, entity selection
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@/test/utils";

// ---------------------------------------------------------------------------
// Mock the graph hooks so we can drive ObservatoryPage state directly.
// We also mock LearningHealthBar to a stub — its own hooks would otherwise
// need fetch mocking.
// ---------------------------------------------------------------------------

const mockUseGraphData = vi.fn();

const mockUseGraphSearch = vi.fn();

vi.mock("./graph-hooks", () => ({
  useGraphData: (agentId?: string) => mockUseGraphData(agentId),
  useGraphSearch: (agentId?: string, onSelect?: (e: unknown) => void) =>
    mockUseGraphSearch(agentId, onSelect),
  useEntityConnections: () => ({ data: null, loading: false, error: null }),
  useGraphStats: () => ({ stats: null, loading: false, error: null }),
  useDistillationStatus: () => ({
    status: null,
    loading: false,
    error: null,
    refetch: () => {},
  }),
  useBackfill: () => ({
    run: () => {},
    isRunning: false,
    isDone: false,
    progress: { current: 0, total: 0 },
    error: null,
  }),
}));

const mockListAgents = vi.fn();

vi.mock("@/services/transport", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("@/services/transport");
  return {
    ...actual,
    getTransport: async () => ({
      listAgents: mockListAgents,
    }),
  };
});

// We import after the mocks so the module captures the stubs.
import { ObservatoryPage } from "./ObservatoryPage";

beforeEach(() => {
  vi.clearAllMocks();
  mockListAgents.mockResolvedValue({
    success: true,
    data: [
      { id: "agent-1", name: "researcher" },
      { id: "agent-2", name: "coder" },
    ],
  });
  mockUseGraphData.mockReturnValue({
    entities: [],
    relationships: [],
    loading: false,
    error: null,
    refetch: vi.fn(),
    totals: { entities: 0, relationships: 0 },
    complete: true,
    stale: false,
    capped: false,
    loopbackOnly: false,
    unresolvedEndpoints: 0,
  });
  mockUseGraphSearch.mockReturnValue({
    query: "",
    results: [],
    searched: false,
    failed: false,
    setQuery: vi.fn(),
    open: vi.fn(),
  });
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("ObservatoryPage — toolbar", () => {
  it("renders the All filter chip and loads agents into pills", async () => {
    render(<ObservatoryPage />);
    expect(screen.getByText("All")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByText("researcher")).toBeInTheDocument();
      expect(screen.getByText("coder")).toBeInTheDocument();
    });
  });

  it("All chip is active by default", () => {
    render(<ObservatoryPage />);
    expect(screen.getByText("All").className).toContain("filter-chip--active");
  });

  it("clicking an agent pill makes that agent active and unsets All", async () => {
    render(<ObservatoryPage />);
    await waitFor(() => expect(screen.getByText("researcher")).toBeInTheDocument());
    fireEvent.click(screen.getByText("researcher"));
    expect(screen.getByText("researcher").className).toContain("filter-chip--active");
    expect(screen.getByText("All").className).not.toContain("filter-chip--active");
  });

  it("renders the search input", () => {
    render(<ObservatoryPage />);
    expect(screen.getByPlaceholderText(/highlight entities/i)).toBeInTheDocument();
  });

  it("clicking Refresh calls the refetch from useGraphData", () => {
    const refetch = vi.fn();
    mockUseGraphData.mockReturnValueOnce({
      entities: [],
      relationships: [],
      loading: false,
      error: null,
      refetch,
      totals: { entities: 0, relationships: 0 },
      complete: false,
      stale: false,
      capped: false,
      loopbackOnly: false,
      unresolvedEndpoints: 0,
    });
    render(<ObservatoryPage />);
    fireEvent.click(screen.getByRole("button", { name: /refresh/i }));
    expect(refetch).toHaveBeenCalled();
  });
});

describe("ObservatoryPage — main pane states", () => {
  it("renders loading spinner copy when loading=true", () => {
    mockUseGraphData.mockReturnValueOnce({
      entities: [],
      relationships: [],
      loading: true,
      error: null,
      refetch: vi.fn(),
      totals: { entities: 0, relationships: 0 },
      complete: false,
      stale: false,
      capped: false,
      loopbackOnly: false,
      unresolvedEndpoints: 0,
    });
    render(<ObservatoryPage />);
    expect(screen.getByText(/loading knowledge graph/i)).toBeInTheDocument();
  });

  it("renders error + Retry button when error is set", () => {
    const refetch = vi.fn();
    mockUseGraphData.mockReturnValueOnce({
      entities: [],
      relationships: [],
      loading: false,
      error: "Network unreachable",
      refetch,
      totals: { entities: 0, relationships: 0 },
      complete: false,
      stale: false,
      capped: false,
      loopbackOnly: false,
      unresolvedEndpoints: 0,
    });
    render(<ObservatoryPage />);
    expect(screen.getByText(/network unreachable/i)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /retry/i }));
    expect(refetch).toHaveBeenCalled();
  });

  it("renders the empty state when no entities exist", () => {
    render(<ObservatoryPage />);
    expect(screen.getByText(/no knowledge graph data/i)).toBeInTheDocument();
    expect(
      screen.getByText(/entities and relationships appear here/i)
    ).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Truthful exploration status (spec AC3)
// ---------------------------------------------------------------------------
describe("ObservatoryPage — truthful status", () => {
  it("shows loaded versus available counts and the complete flag", () => {
    mockUseGraphData.mockReturnValue({
      entities: new Array(1200).fill({ id: "e", agent_id: "a", name: "e", entity_type: "Concept" }),
      relationships: [],
      loading: false,
      error: null,
      refetch: vi.fn(),
      totals: { entities: 16693, relationships: 4728 },
      complete: true,
      stale: false,
      capped: false,
      loopbackOnly: false,
      unresolvedEndpoints: 0,
    });
    render(<ObservatoryPage />);
    expect(screen.getByText(/1,200 \/ 16,693 entities/)).toBeInTheDocument();
    expect(screen.getByText(/0 \/ 4,728 relationships/)).toBeInTheDocument();
    expect(screen.getByText("complete")).toBeInTheDocument();
  });

  it("offers refresh when the dataset changed during load and counts unresolved edges", () => {
    const refetch = vi.fn();
    mockUseGraphData.mockReturnValue({
      entities: [],
      relationships: [],
      loading: false,
      error: null,
      refetch,
      totals: { entities: 5, relationships: 0 },
      complete: false,
      stale: true,
      capped: false,
      loopbackOnly: false,
      unresolvedEndpoints: 3,
    });
    render(<ObservatoryPage />);
    expect(screen.getByText(/dataset changed during load/)).toBeInTheDocument();
    expect(screen.getByText(/3 edges reference unloaded entities/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Refresh now" }));
    expect(refetch).toHaveBeenCalledOnce();
  });

  it("discloses the loopback-only denial state truthfully", () => {
    mockUseGraphData.mockReturnValue({
      entities: [],
      relationships: [],
      loading: false,
      error: null,
      refetch: vi.fn(),
      totals: { entities: 0, relationships: 0 },
      complete: false,
      stale: false,
      capped: false,
      loopbackOnly: true,
      unresolvedEndpoints: 0,
    });
    render(<ObservatoryPage />);
    expect(screen.getByText(/available on this device only/i)).toBeInTheDocument();
  });

  it("explains the admission cap instead of claiming a complete view", () => {
    mockUseGraphData.mockReturnValue({
      entities: new Array(3).fill({ id: "e", agent_id: "a", name: "e", entity_type: "Concept" }),
      relationships: [],
      loading: false,
      error: null,
      refetch: vi.fn(),
      totals: { entities: 60000, relationships: 0 },
      complete: false,
      stale: false,
      capped: true,
      loopbackOnly: false,
      unresolvedEndpoints: 0,
    });
    render(<ObservatoryPage />);
    expect(screen.getByText(/use search to inspect the rest/i)).toBeInTheDocument();
    expect(screen.queryByText("complete")).not.toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// AC3 — graph strings render as inert text, never markup.
// ---------------------------------------------------------------------------
describe("ObservatoryPage — inert display data", () => {
  it("renders a markup-bearing entity name literally in the detail panel", async () => {
    const hostile = {
      id: "evil",
      agent_id: "a",
      name: '<img src=x onerror="alert(1)">',
      entity_type: "concept",
      properties: { note: "<script>alert(2)</script>" },
      mention_count: 1,
      first_seen_at: "2026-10-04T00:00:00Z",
      last_seen_at: "2026-10-04T00:00:00Z",
    };
    const { EntityDetail } = await import("./EntityDetail");
    render(<EntityDetail entity={hostile} onClose={vi.fn()} />);
    expect(screen.getByText(/alert\(1\)/)).toBeInTheDocument();
    expect(screen.getByText(/<script>alert\(2\)<\/script>/)).toBeInTheDocument();
    expect(document.querySelector("img[src='x']")).toBeNull();
    expect(document.querySelector("script")).toBeNull();
  });
});

describe("ObservatoryPage — search failure copy", () => {
  it("renders search-unavailable distinctly from no-matches", () => {
    mockUseGraphData.mockReturnValue({
      entities: [], relationships: [], loading: false, error: null, refetch: vi.fn(),
      totals: { entities: 0, relationships: 0 }, complete: true, stale: false,
      capped: false, loopbackOnly: false, unresolvedEndpoints: 0,
    });
    mockUseGraphSearch.mockReturnValue({
      query: "deep", results: [], searched: true, failed: true,
      setQuery: vi.fn(), open: vi.fn(),
    });
    const { rerender } = render(<ObservatoryPage />);
    expect(screen.getByText(/search is unavailable right now/i)).toBeInTheDocument();
    expect(screen.queryByText(/no server matches/i)).not.toBeInTheDocument();
    mockUseGraphSearch.mockReturnValue({
      query: "deep", results: [], searched: true, failed: false,
      setQuery: vi.fn(), open: vi.fn(),
    });
    rerender(<ObservatoryPage />);
    expect(screen.getByText(/no server matches for/i)).toBeInTheDocument();
  });
});
