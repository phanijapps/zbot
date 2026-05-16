// ============================================================================
// BeliefNetworkPanel — mock hooks, render states, budget-exhausted warning
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@/test/utils";

import type {
  BeliefNetworkStatsResponse,
  BeliefActivityEvent,
} from "../types.beliefNetwork";

const mockStatsHook = vi.fn();
const mockActivityHook = vi.fn();

vi.mock("./hooks", () => ({
  useBeliefNetworkStats: () => mockStatsHook(),
  useBeliefNetworkActivity: () => mockActivityHook(),
}));

import { BeliefNetworkPanel } from "./BeliefNetworkPanel";

function statsFixture(
  overrides: Partial<BeliefNetworkStatsResponse> = {},
): BeliefNetworkStatsResponse {
  return {
    enabled: true,
    synthesizer: {
      latest: {
        subjects_examined: 3,
        beliefs_synthesized: 2,
        beliefs_short_circuited: 1,
        beliefs_llm_synthesized: 1,
        llm_calls: 1,
        errors: 0,
        stale_beliefs_resynthesized: 0,
      },
      history: [
        {
          timestamp: "2026-05-15T10:00:00Z",
          subjects_examined: 1,
          beliefs_synthesized: 1,
          beliefs_short_circuited: 0,
          beliefs_llm_synthesized: 1,
          llm_calls: 1,
          errors: 0,
          stale_beliefs_resynthesized: 0,
        },
        {
          timestamp: "2026-05-15T11:00:00Z",
          subjects_examined: 3,
          beliefs_synthesized: 2,
          beliefs_short_circuited: 1,
          beliefs_llm_synthesized: 1,
          llm_calls: 1,
          errors: 0,
          stale_beliefs_resynthesized: 0,
        },
      ],
    },
    contradiction_detector: {
      latest: {
        neighborhoods_examined: 2,
        pairs_examined: 5,
        pairs_skipped_existing: 1,
        llm_calls: 4,
        contradictions_logical: 1,
        contradictions_tension: 1,
        duplicates_logged: 0,
        compatibles_logged: 1,
        errors: 0,
        budget_exhausted: false,
      },
      history: [
        {
          timestamp: "2026-05-15T10:00:00Z",
          neighborhoods_examined: 1,
          pairs_examined: 2,
          pairs_skipped_existing: 0,
          llm_calls: 2,
          contradictions_logical: 0,
          contradictions_tension: 0,
          duplicates_logged: 0,
          compatibles_logged: 0,
          errors: 0,
          budget_exhausted: false,
        },
        {
          timestamp: "2026-05-15T11:00:00Z",
          neighborhoods_examined: 2,
          pairs_examined: 5,
          pairs_skipped_existing: 1,
          llm_calls: 4,
          contradictions_logical: 1,
          contradictions_tension: 1,
          duplicates_logged: 0,
          compatibles_logged: 1,
          errors: 0,
          budget_exhausted: false,
        },
      ],
    },
    propagator: {
      latest: {
        beliefs_invalidated: 0,
        beliefs_retracted: 0,
        beliefs_marked_stale: 0,
        max_propagation_depth: 0,
        errors: 0,
      },
      history: [],
    },
    totals: {
      total_beliefs: 12,
      total_contradictions: 3,
      total_unresolved_contradictions: 1,
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
  mockActivityHook.mockReturnValue({
    events: [] as BeliefActivityEvent[],
    loading: false,
    error: null,
    refetch: vi.fn(),
  });
});

describe("BeliefNetworkPanel", () => {
  it("renders the panel with mock stats data", () => {
    render(<BeliefNetworkPanel />);
    expect(screen.getByTestId("belief-network-panel")).toBeInTheDocument();
    expect(screen.getByTestId("synthesizer-card")).toBeInTheDocument();
    expect(screen.getByTestId("detector-card")).toBeInTheDocument();
    expect(screen.getByTestId("propagator-card")).toBeInTheDocument();
  });

  it("renders the disabled placeholder when stats.enabled is false", () => {
    mockStatsHook.mockReturnValueOnce({
      stats: statsFixture({ enabled: false }),
      loading: false,
      error: null,
      refetch: vi.fn(),
    });
    render(<BeliefNetworkPanel />);
    expect(
      screen.getByTestId("belief-network-panel-disabled"),
    ).toBeInTheDocument();
    expect(
      screen.queryByTestId("belief-network-panel"),
    ).not.toBeInTheDocument();
  });

  it("renders the loading placeholder while the stats hook is pending", () => {
    mockStatsHook.mockReturnValueOnce({
      stats: null,
      loading: true,
      error: null,
      refetch: vi.fn(),
    });
    render(<BeliefNetworkPanel />);
    expect(
      screen.getByTestId("belief-network-panel-loading"),
    ).toBeInTheDocument();
  });

  it("renders sparkline bars matching history length", () => {
    render(<BeliefNetworkPanel />);
    const bars = screen.getAllByTestId("sparkline-bar");
    // 2 synthesizer history rows + 2 contradiction-detector rows = 4 bars
    // (the propagator history is empty so renders the empty placeholder)
    expect(bars.length).toBe(4);
  });

  it("shows the budget-exhausted warning when the flag is set", () => {
    const base = statsFixture();
    base.contradiction_detector.latest.budget_exhausted = true;
    mockStatsHook.mockReturnValueOnce({
      stats: base,
      loading: false,
      error: null,
      refetch: vi.fn(),
    });
    render(<BeliefNetworkPanel />);
    expect(
      screen.getByTestId("budget-exhausted-warning"),
    ).toBeInTheDocument();
  });

  it("renders activity events in the order supplied by the hook", () => {
    const events: BeliefActivityEvent[] = [
      {
        kind: "synthesized",
        timestamp: "2026-05-15T11:00:00Z",
        summary: "Newest event",
      },
      {
        kind: "retracted",
        timestamp: "2026-05-15T10:00:00Z",
        summary: "Older event",
      },
    ];
    mockActivityHook.mockReturnValueOnce({
      events,
      loading: false,
      error: null,
      refetch: vi.fn(),
    });
    render(<BeliefNetworkPanel />);
    const rendered = screen.getAllByTestId("belief-activity-event");
    expect(rendered).toHaveLength(2);
    expect(rendered[0].textContent).toContain("Newest event");
    expect(rendered[1].textContent).toContain("Older event");
  });

  it("renders an empty activity feed when no events", () => {
    render(<BeliefNetworkPanel />);
    expect(screen.getByTestId("belief-activity-empty")).toBeInTheDocument();
  });
});
