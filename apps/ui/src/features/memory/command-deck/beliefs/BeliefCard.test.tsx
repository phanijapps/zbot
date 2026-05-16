// ============================================================================
// BeliefCard — confidence + subject + source count render correctly,
// stale + contradiction badges show only when warranted.
// ============================================================================

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@/test/utils";
import { BeliefCard } from "./BeliefCard";
import type { Belief, BeliefContradiction } from "../types.beliefs";

function sampleBelief(overrides: Partial<Belief> = {}): Belief {
  return {
    id: "b1",
    partition_id: "root",
    subject: "user.location",
    content: "User lives in Seattle.",
    confidence: 0.87,
    valid_from: "2026-05-01T00:00:00Z",
    valid_until: null,
    source_fact_ids: ["fact-1", "fact-2"],
    synthesizer_version: 1,
    reasoning: null,
    stale: false,
    created_at: "2026-05-01T00:00:00Z",
    updated_at: "2026-05-10T00:00:00Z",
    superseded_by: null,
    ...overrides,
  };
}

function sampleContradiction(
  overrides: Partial<BeliefContradiction> = {},
): BeliefContradiction {
  return {
    id: "c1",
    belief_a_id: "b1",
    belief_b_id: "b2",
    contradiction_type: "logical",
    severity: 0.9,
    judge_reasoning: "two different locations",
    detected_at: "2026-05-10T00:00:00Z",
    resolved_at: null,
    resolution: null,
    ...overrides,
  };
}

describe("BeliefCard", () => {
  it("renders the subject and confidence value", () => {
    render(<BeliefCard belief={sampleBelief()} />);
    expect(screen.getByText("user.location")).toBeInTheDocument();
    expect(screen.getByText("0.87")).toBeInTheDocument();
  });

  it("renders source-fact count", () => {
    render(<BeliefCard belief={sampleBelief()} />);
    expect(screen.getByText("2 sources")).toBeInTheDocument();
  });

  it("singularizes 'source' when only one source fact", () => {
    render(<BeliefCard belief={sampleBelief({ source_fact_ids: ["f1"] })} />);
    expect(screen.getByText("1 source")).toBeInTheDocument();
  });

  it("renders a stale label when belief is stale", () => {
    render(<BeliefCard belief={sampleBelief({ stale: true })} />);
    expect(screen.getByText("stale")).toBeInTheDocument();
  });

  it("does not render a stale label by default", () => {
    render(<BeliefCard belief={sampleBelief()} />);
    expect(screen.queryByText("stale")).toBeNull();
  });

  it("renders a contradiction badge when unresolved contradictions are present", () => {
    render(
      <BeliefCard
        belief={sampleBelief()}
        contradictions={[sampleContradiction()]}
      />,
    );
    expect(
      screen.getByLabelText(/unresolved contradiction/i),
    ).toBeInTheDocument();
  });

  it("hides the badge when all contradictions are resolved", () => {
    render(
      <BeliefCard
        belief={sampleBelief()}
        contradictions={[
          sampleContradiction({
            resolved_at: "2026-05-11T00:00:00Z",
            resolution: "a_won",
          }),
        ]}
      />,
    );
    expect(screen.queryByLabelText(/unresolved contradiction/i)).toBeNull();
  });

  it("calls onClick when the card is clicked", () => {
    const onClick = vi.fn();
    render(<BeliefCard belief={sampleBelief()} onClick={onClick} />);
    fireEvent.click(screen.getByText("user.location"));
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
