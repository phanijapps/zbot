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

vi.mock("./graph-hooks", () => ({
  useGraphData: (agentId?: string) => mockUseGraphData(agentId),
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
