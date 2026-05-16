// ============================================================================
// ContradictionResolver — button clicks fire the resolveContradiction API
// with the right payload; disabled state when already resolved.
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@/test/utils";
import { ContradictionResolver } from "./ContradictionResolver";
import type { Belief, BeliefContradiction } from "../types.beliefs";

const mockGetBeliefDetail = vi.fn();
const mockResolveContradiction = vi.fn();

vi.mock("./api", () => ({
  listBeliefs: vi.fn(),
  listContradictions: vi.fn(),
  getBeliefDetail: (...args: unknown[]) => mockGetBeliefDetail(...args),
  resolveContradiction: (...args: unknown[]) =>
    mockResolveContradiction(...args),
}));

function belief(id: string, subject: string): Belief {
  return {
    id,
    partition_id: "root",
    subject,
    content: `content for ${subject}`,
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
  };
}

function contradiction(over: Partial<BeliefContradiction> = {}): BeliefContradiction {
  return {
    id: "c1",
    belief_a_id: "ba",
    belief_b_id: "bb",
    contradiction_type: "logical",
    severity: 0.85,
    judge_reasoning: "Two different employers at the same time",
    detected_at: "2026-05-10T00:00:00Z",
    resolved_at: null,
    resolution: null,
    ...over,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  mockGetBeliefDetail.mockImplementation((_agent: string, id: string) =>
    Promise.resolve({
      success: true,
      data: {
        belief: belief(id, id === "ba" ? "subj.a" : "subj.b"),
        source_facts: [],
        contradictions: [],
      },
    }),
  );
  mockResolveContradiction.mockResolvedValue({ success: true });
});

describe("ContradictionResolver", () => {
  it("renders both belief panels and the judge reasoning", async () => {
    render(
      <ContradictionResolver
        agentId="root"
        contradiction={contradiction()}
        onClose={() => {}}
        onResolved={() => {}}
      />,
    );
    await waitFor(() => expect(screen.getByText("subj.a")).toBeInTheDocument());
    expect(screen.getByText("subj.b")).toBeInTheDocument();
    expect(
      screen.getByText("Two different employers at the same time"),
    ).toBeInTheDocument();
  });

  it("calls resolveContradiction with 'a_won' when A wins is clicked", async () => {
    const onResolved = vi.fn();
    render(
      <ContradictionResolver
        agentId="root"
        contradiction={contradiction()}
        onClose={() => {}}
        onResolved={onResolved}
      />,
    );
    await waitFor(() => expect(screen.getByText("subj.a")).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: /A wins/i }));
    await waitFor(() => {
      expect(mockResolveContradiction).toHaveBeenCalledWith("c1", "a_won");
      expect(onResolved).toHaveBeenCalledTimes(1);
    });
  });

  it("calls resolveContradiction with 'b_won' when B wins is clicked", async () => {
    render(
      <ContradictionResolver
        agentId="root"
        contradiction={contradiction()}
        onClose={() => {}}
        onResolved={() => {}}
      />,
    );
    await waitFor(() => expect(screen.getByText("subj.a")).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: /B wins/i }));
    await waitFor(() =>
      expect(mockResolveContradiction).toHaveBeenCalledWith("c1", "b_won"),
    );
  });

  it("calls resolveContradiction with 'compatible' when Mark compatible is clicked", async () => {
    render(
      <ContradictionResolver
        agentId="root"
        contradiction={contradiction()}
        onClose={() => {}}
        onResolved={() => {}}
      />,
    );
    await waitFor(() => expect(screen.getByText("subj.a")).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: /Mark compatible/i }));
    await waitFor(() =>
      expect(mockResolveContradiction).toHaveBeenCalledWith("c1", "compatible"),
    );
  });

  it("disables resolution buttons when already resolved", async () => {
    render(
      <ContradictionResolver
        agentId="root"
        contradiction={contradiction({
          resolved_at: "2026-05-11T00:00:00Z",
          resolution: "a_won",
        })}
        onClose={() => {}}
        onResolved={() => {}}
      />,
    );
    await waitFor(() => expect(screen.getByText("subj.a")).toBeInTheDocument());
    expect(screen.getByRole("button", { name: /A wins/i })).toBeDisabled();
    expect(screen.getByRole("button", { name: /B wins/i })).toBeDisabled();
    expect(
      screen.getByRole("button", { name: /Mark compatible/i }),
    ).toBeDisabled();
  });
});
