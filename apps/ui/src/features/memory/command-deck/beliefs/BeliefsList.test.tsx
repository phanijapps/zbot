// ============================================================================
// BeliefsList — loading / empty / disabled / error states + filtering.
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@/test/utils";
import { BeliefsList } from "./BeliefsList";
import type { Belief, BeliefContradiction } from "../types.beliefs";

const mockListBeliefs = vi.fn();
const mockListContradictions = vi.fn();
const mockGetBeliefDetail = vi.fn();

vi.mock("./api", () => ({
  listBeliefs: (...args: unknown[]) => mockListBeliefs(...args),
  listContradictions: (...args: unknown[]) => mockListContradictions(...args),
  getBeliefDetail: (...args: unknown[]) => mockGetBeliefDetail(...args),
  resolveContradiction: vi.fn(),
}));

function belief(over: Partial<Belief> = {}): Belief {
  return {
    id: "b1",
    partition_id: "root",
    subject: "user.location",
    content: "User lives in Seattle.",
    confidence: 0.9,
    valid_from: null,
    valid_until: null,
    source_fact_ids: ["f1"],
    synthesizer_version: 1,
    reasoning: null,
    stale: false,
    created_at: "2026-05-01T00:00:00Z",
    updated_at: "2026-05-10T00:00:00Z",
    superseded_by: null,
    ...over,
  };
}

function contradiction(over: Partial<BeliefContradiction> = {}): BeliefContradiction {
  return {
    id: "c1",
    belief_a_id: "b1",
    belief_b_id: "b2",
    contradiction_type: "logical",
    severity: 0.7,
    judge_reasoning: null,
    detected_at: "2026-05-10T00:00:00Z",
    resolved_at: null,
    resolution: null,
    ...over,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("BeliefsList", () => {
  it("shows 'Select a ward' prompt when no partition is supplied", () => {
    render(<BeliefsList agentId="root" partitionId={null} />);
    expect(screen.getByText(/Select a ward/i)).toBeInTheDocument();
  });

  it("renders beliefs returned by the API, sorted by confidence desc", async () => {
    mockListBeliefs.mockResolvedValue({
      success: true,
      data: [
        belief({ id: "b1", subject: "user.location", confidence: 0.5 }),
        belief({ id: "b2", subject: "user.employment", confidence: 0.95 }),
      ],
    });
    mockListContradictions.mockResolvedValue({ success: true, data: [] });

    render(<BeliefsList agentId="root" partitionId="root" />);

    await waitFor(() => {
      expect(screen.getByText("user.employment")).toBeInTheDocument();
    });
    const subjects = screen.getAllByText(/^user\./);
    expect(subjects[0]).toHaveTextContent("user.employment");
  });

  it("renders the disabled message when API returns 503", async () => {
    mockListBeliefs.mockResolvedValue({ success: false, disabled: true });
    mockListContradictions.mockResolvedValue({
      success: false,
      disabled: true,
    });
    render(<BeliefsList agentId="root" partitionId="root" />);
    await waitFor(() => {
      expect(
        screen.getByText(/Belief Network is disabled/i),
      ).toBeInTheDocument();
    });
  });

  it("renders an error message when the API fails", async () => {
    mockListBeliefs.mockResolvedValue({ success: false, error: "boom" });
    mockListContradictions.mockResolvedValue({ success: true, data: [] });
    render(<BeliefsList agentId="root" partitionId="root" />);
    await waitFor(() => {
      expect(screen.getByText("boom")).toBeInTheDocument();
    });
  });

  it("filters out beliefs below min-confidence when slider moves", async () => {
    mockListBeliefs.mockResolvedValue({
      success: true,
      data: [
        belief({ id: "b1", subject: "low.conf", confidence: 0.3 }),
        belief({ id: "b2", subject: "high.conf", confidence: 0.9 }),
      ],
    });
    mockListContradictions.mockResolvedValue({ success: true, data: [] });
    render(<BeliefsList agentId="root" partitionId="root" />);
    await waitFor(() =>
      expect(screen.getByText("low.conf")).toBeInTheDocument(),
    );

    const slider = screen.getByLabelText(/Min confidence/i);
    fireEvent.change(slider, { target: { value: "0.5" } });

    await waitFor(() => {
      expect(screen.queryByText("low.conf")).toBeNull();
      expect(screen.getByText("high.conf")).toBeInTheDocument();
    });
  });

  it("filters to only-contradicted beliefs when toggle is on", async () => {
    mockListBeliefs.mockResolvedValue({
      success: true,
      data: [belief({ id: "b1" }), belief({ id: "b2", subject: "other.subject" })],
    });
    mockListContradictions.mockResolvedValue({
      success: true,
      data: [contradiction({ belief_a_id: "b1", belief_b_id: "bx" })],
    });
    render(<BeliefsList agentId="root" partitionId="root" />);
    await waitFor(() =>
      expect(screen.getByText("user.location")).toBeInTheDocument(),
    );

    fireEvent.click(screen.getByLabelText(/Only contradicted/i));
    await waitFor(() => {
      expect(screen.queryByText("other.subject")).toBeNull();
      expect(screen.getByText("user.location")).toBeInTheDocument();
    });
  });
});
