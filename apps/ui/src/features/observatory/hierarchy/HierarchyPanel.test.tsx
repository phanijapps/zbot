// ============================================================================
// HierarchyPanel — render states (loading / error / disabled / populated),
// chip rendering, and singular/plural noun agreement.
// ============================================================================
//
// Mocks the `useHierarchyStats` hook (same pattern as BeliefNetworkPanel
// uses for `useBeliefNetworkStats`) so the component renders synchronously
// without an HTTP round-trip.
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@/test/utils";

import type { HierarchyStatsResponse } from "./types";

const mockStatsHook = vi.fn();

vi.mock("./hooks", () => ({
  useHierarchyStats: () => mockStatsHook(),
}));

import { HierarchyPanel } from "./HierarchyPanel";

function statsFixture(
  overrides: Partial<HierarchyStatsResponse> = {},
): HierarchyStatsResponse {
  return {
    enabled: true,
    agent_id: "root",
    summary: {
      layer_counts: [
        [0, 693],
        [1, 30],
      ],
      inter_cluster_relations: 11,
      top_aggregates: [
        {
          id: "agg-1",
          name: "Maritime Operations",
          layer: 1,
          member_count: 25,
          description: "Cluster spanning shipping, vessels, and port logistics.",
        },
        {
          id: "agg-2",
          name: "Code Review Patterns",
          layer: 1,
          member_count: 1,
          description: "",
        },
      ],
    },
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  mockStatsHook.mockReturnValue({
    stats: statsFixture(),
    loading: false,
    error: null,
    refetch: vi.fn(),
  });
});

describe("HierarchyPanel", () => {
  it("renders the loading placeholder while pending", () => {
    mockStatsHook.mockReturnValueOnce({
      stats: null,
      loading: true,
      error: null,
      refetch: vi.fn(),
    });
    render(<HierarchyPanel />);
    expect(screen.getByText(/Loading hierarchy stats/i)).toBeInTheDocument();
  });

  it("renders the error message with a retry button when the hook errors", () => {
    const refetch = vi.fn();
    mockStatsHook.mockReturnValueOnce({
      stats: null,
      loading: false,
      error: "boom",
      refetch,
    });
    render(<HierarchyPanel />);
    expect(
      screen.getByText(/Failed to load hierarchy stats: boom/i),
    ).toBeInTheDocument();
    const retry = screen.getByRole("button", { name: /Retry/i });
    fireEvent.click(retry);
    expect(refetch).toHaveBeenCalledTimes(1);
  });

  it("renders the disabled message when hierarchy.enabled is false", () => {
    mockStatsHook.mockReturnValueOnce({
      stats: statsFixture({ enabled: false }),
      loading: false,
      error: null,
      refetch: vi.fn(),
    });
    render(<HierarchyPanel />);
    expect(
      screen.getByText(/Hierarchical memory is disabled/i),
    ).toBeInTheDocument();
  });

  it("renders a layer chip per layer with the L<n> · <count> label", () => {
    render(<HierarchyPanel />);
    expect(screen.getByText("L0 · 693")).toBeInTheDocument();
    expect(screen.getByText("L1 · 30")).toBeInTheDocument();
  });

  it("renders the inter-cluster edge count with pluralization", () => {
    render(<HierarchyPanel />);
    expect(screen.getByText("11 edges")).toBeInTheDocument();
  });

  it("uses singular 'edge' when there is exactly 1 inter-cluster relation", () => {
    const fixture = statsFixture();
    fixture.summary.inter_cluster_relations = 1;
    mockStatsHook.mockReturnValueOnce({
      stats: fixture,
      loading: false,
      error: null,
      refetch: vi.fn(),
    });
    render(<HierarchyPanel />);
    expect(screen.getByText("1 edge")).toBeInTheDocument();
  });

  it("renders top aggregates with name, description, layer + member chips", () => {
    render(<HierarchyPanel />);
    const list = screen.getByTestId("hierarchy-aggregate-list");
    expect(list).toBeInTheDocument();
    expect(screen.getByText("Maritime Operations")).toBeInTheDocument();
    expect(
      screen.getByText(/Cluster spanning shipping, vessels, and port logistics/),
    ).toBeInTheDocument();
    expect(screen.getByText("25 members")).toBeInTheDocument();
    expect(screen.getByText("1 member")).toBeInTheDocument();
  });

  it("renders the empty-aggregates message when top_aggregates is empty", () => {
    const fixture = statsFixture();
    fixture.summary.top_aggregates = [];
    mockStatsHook.mockReturnValueOnce({
      stats: fixture,
      loading: false,
      error: null,
      refetch: vi.fn(),
    });
    render(<HierarchyPanel />);
    expect(screen.getByText(/No aggregates yet/i)).toBeInTheDocument();
  });

  it("renders the empty-entities message when there are zero layers", () => {
    const fixture = statsFixture();
    fixture.summary.layer_counts = [];
    fixture.summary.top_aggregates = [];
    fixture.summary.inter_cluster_relations = 0;
    mockStatsHook.mockReturnValueOnce({
      stats: fixture,
      loading: false,
      error: null,
      refetch: vi.fn(),
    });
    render(<HierarchyPanel />);
    expect(
      screen.getByText(/No entities yet — nothing to cluster/i),
    ).toBeInTheDocument();
  });
});
