// ============================================================================
// MemoryFactCard — delete button promoted to row + confirm flow
// ============================================================================

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@/test/utils";
import { MemoryFactCard } from "./MemoryFactCard";
import type { MemoryFact } from "@/services/transport/types";

function makeFact(overrides: Partial<MemoryFact> = {}): MemoryFact {
  return {
    id: "fact-1",
    agent_id: "agent:root",
    scope: "global",
    category: "preference",
    key: "prefers-jwt",
    content: "User prefers JWT auth over sessions.",
    confidence: 0.92,
    mention_count: 3,
    created_at: "2026-04-20T12:00:00Z",
    updated_at: "2026-04-22T12:00:00Z",
    ...overrides,
  };
}

describe("MemoryFactCard — row delete button", () => {
  let confirmSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
  });

  afterEach(() => {
    confirmSpy.mockRestore();
  });

  it("renders the delete button on the collapsed row (always visible, not buried inside expand)", () => {
    render(<MemoryFactCard fact={makeFact()} onDelete={() => {}} />);
    const btn = screen.getByRole("button", { name: /delete preference memory/i });
    expect(btn).toBeInTheDocument();
    // Confirms the row is still collapsed (delete is on the header row).
    expect(screen.queryByText(/mentions/i)).not.toBeInTheDocument();
  });

  it("calls onDelete after the user confirms", async () => {
    const onDelete = vi.fn();
    render(<MemoryFactCard fact={makeFact()} onDelete={onDelete} />);
    fireEvent.click(screen.getByRole("button", { name: /delete preference memory/i }));
    await waitFor(() => expect(onDelete).toHaveBeenCalledTimes(1));
    expect(confirmSpy).toHaveBeenCalled();
  });

  it("does NOT call onDelete when the user cancels confirm", () => {
    confirmSpy.mockReturnValue(false);
    const onDelete = vi.fn();
    render(<MemoryFactCard fact={makeFact()} onDelete={onDelete} />);
    fireEvent.click(screen.getByRole("button", { name: /delete preference memory/i }));
    expect(confirmSpy).toHaveBeenCalled();
    expect(onDelete).not.toHaveBeenCalled();
  });

  it("does NOT toggle the row expansion when delete is clicked", () => {
    render(<MemoryFactCard fact={makeFact()} onDelete={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: /delete preference memory/i }));
    // Mentions only renders inside expanded body — should still be hidden.
    expect(screen.queryByText(/mentions/i)).not.toBeInTheDocument();
  });

  it("clicking the row body still toggles expansion", () => {
    render(<MemoryFactCard fact={makeFact()} onDelete={() => {}} />);
    // The row body is the parent of the chevron text; click the key text.
    const key = screen.getByText("prefers-jwt");
    fireEvent.click(key);
    expect(screen.getByText(/mentions/i)).toBeInTheDocument();
  });
});
