// ============================================================================
// KpiStrip — render test (numbers + delta + sub-labels)
// ============================================================================

import { describe, it, expect } from "vitest";
import { render, screen } from "@/test/utils";
import { KpiStrip } from "./KpiStrip";
import type { MissionKpis } from "./types";

function makeKpis(overrides: Partial<MissionKpis> = {}): MissionKpis {
  return {
    running: 0,
    queued: 0,
    done24h: 0,
    failed24h: 0,
    paused: 0,
    runningTokens: 0,
    successRate: null,
    delta24h: null,
    ...overrides,
  };
}

describe("KpiStrip", () => {
  it("renders all five labels in canonical order", () => {
    render(<KpiStrip kpis={makeKpis()} />);
    expect(screen.getByText("Running")).toBeInTheDocument();
    expect(screen.getByText("Queued")).toBeInTheDocument();
    expect(screen.getByText("Done · 24h")).toBeInTheDocument();
    expect(screen.getByText("Failed · 24h")).toBeInTheDocument();
    expect(screen.getByText("Paused")).toBeInTheDocument();
  });

  it("renders the running count with a streaming-tokens sub-label", () => {
    render(<KpiStrip kpis={makeKpis({ running: 3, runningTokens: 1_400_000 })} />);
    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByText(/1\.4M tok streaming/)).toBeInTheDocument();
  });

  it("renders 'all clear' on Failed when count is zero", () => {
    render(<KpiStrip kpis={makeKpis()} />);
    expect(screen.getByText(/all clear/i)).toBeInTheDocument();
  });

  it("renders 'needs review' on Failed when count > 0", () => {
    render(<KpiStrip kpis={makeKpis({ failed24h: 2 })} />);
    expect(screen.getByText(/needs review/i)).toBeInTheDocument();
  });

  it("renders the success rate sub-label on Done when computed", () => {
    render(<KpiStrip kpis={makeKpis({ done24h: 9, successRate: 87 })} />);
    expect(screen.getByText(/87% success/)).toBeInTheDocument();
  });

  it("renders ▲ + delta% in green for a positive delta", () => {
    render(<KpiStrip kpis={makeKpis({ delta24h: 12 })} />);
    expect(screen.getByText(/▲ \+12%/)).toBeInTheDocument();
  });

  it("renders ▼ delta% for a negative delta", () => {
    render(<KpiStrip kpis={makeKpis({ delta24h: -8 })} />);
    expect(screen.getByText(/▼ -8%/)).toBeInTheDocument();
  });

  it("renders an em-dash placeholder when delta is null", () => {
    render(<KpiStrip kpis={makeKpis()} />);
    // Delta cell shows "—"
    expect(screen.getByText(/vs 24h ago/i).previousSibling?.textContent).toBe("—");
  });

  it("has a region role with an accessible name", () => {
    render(<KpiStrip kpis={makeKpis()} />);
    expect(screen.getByRole("region", { name: /mission control overview/i })).toBeInTheDocument();
  });
});
