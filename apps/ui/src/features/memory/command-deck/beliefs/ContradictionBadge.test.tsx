// ============================================================================
// ContradictionBadge — tooltip aggregates type + severity, click fires
// when handler is supplied.
// ============================================================================

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@/test/utils";
import { ContradictionBadge } from "./ContradictionBadge";
import type { BeliefContradiction } from "../types.beliefs";

function sample(
  overrides: Partial<BeliefContradiction> = {},
): BeliefContradiction {
  return {
    id: "c1",
    belief_a_id: "b1",
    belief_b_id: "b2",
    contradiction_type: "logical",
    severity: 0.8,
    judge_reasoning: null,
    detected_at: "2026-05-10T00:00:00Z",
    resolved_at: null,
    resolution: null,
    ...overrides,
  };
}

describe("ContradictionBadge", () => {
  it("renders nothing when all contradictions are resolved", () => {
    const { container } = render(
      <ContradictionBadge
        contradictions={[
          sample({ resolved_at: "2026-05-11T00:00:00Z", resolution: "a_won" }),
        ]}
      />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders an aria-labelled badge for unresolved contradictions", () => {
    render(<ContradictionBadge contradictions={[sample()]} />);
    const badge = screen.getByLabelText(/1 unresolved contradiction/i);
    expect(badge).toBeInTheDocument();
    expect(badge.getAttribute("title") ?? "").toContain("logical");
    expect(badge.getAttribute("title") ?? "").toContain("0.80");
  });

  it("includes max severity in the tooltip when multiple contradictions exist", () => {
    render(
      <ContradictionBadge
        contradictions={[
          sample({ id: "c1", severity: 0.3 }),
          sample({ id: "c2", severity: 0.9, contradiction_type: "tension" }),
        ]}
      />,
    );
    const badge = screen.getByLabelText(/2 unresolved contradictions/i);
    expect(badge.getAttribute("title") ?? "").toContain("0.90");
  });

  it("fires onClick when handler is supplied and badge is clicked", () => {
    const onClick = vi.fn();
    render(
      <ContradictionBadge contradictions={[sample()]} onClick={onClick} />,
    );
    fireEvent.click(screen.getByLabelText(/1 unresolved contradiction/i));
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
