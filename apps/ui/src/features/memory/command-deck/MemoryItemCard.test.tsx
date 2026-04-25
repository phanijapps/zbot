// ============================================================================
// MemoryItemCard (Command Deck) — delete button presence + confirm flow
// ============================================================================

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@/test/utils";
import { MemoryItemCard, type MemoryItemCardProps } from "./MemoryItemCard";

function baseProps(overrides: Partial<MemoryItemCardProps> = {}): MemoryItemCardProps {
  return {
    id: "fact-1",
    content: "User prefers JWT.",
    category: "preference",
    confidence: 0.92,
    created_at: "2026-04-20T12:00:00Z",
    age_bucket: "today",
    ...overrides,
  };
}

describe("MemoryItemCard — delete button", () => {
  let confirmSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
  });

  afterEach(() => {
    confirmSpy.mockRestore();
  });

  it("does NOT render a delete button when onDelete is not provided", () => {
    render(<MemoryItemCard {...baseProps()} />);
    expect(
      screen.queryByRole("button", { name: /delete preference memory/i })
    ).not.toBeInTheDocument();
  });

  it("renders a delete button when onDelete is provided", () => {
    render(<MemoryItemCard {...baseProps({ onDelete: () => {} })} />);
    const btn = screen.getByRole("button", { name: /delete preference memory/i });
    expect(btn).toBeInTheDocument();
  });

  it("calls onDelete with the fact id after the user confirms", async () => {
    const onDelete = vi.fn();
    render(<MemoryItemCard {...baseProps({ onDelete })} />);
    fireEvent.click(screen.getByRole("button", { name: /delete preference memory/i }));
    await waitFor(() => expect(onDelete).toHaveBeenCalledWith("fact-1"));
    expect(confirmSpy).toHaveBeenCalled();
  });

  it("does NOT call onDelete when the user cancels confirm", () => {
    confirmSpy.mockReturnValue(false);
    const onDelete = vi.fn();
    render(<MemoryItemCard {...baseProps({ onDelete })} />);
    fireEvent.click(screen.getByRole("button", { name: /delete preference memory/i }));
    expect(onDelete).not.toHaveBeenCalled();
  });

  it("clicking delete does NOT trigger the row's onClick handler", () => {
    const onClick = vi.fn();
    const onDelete = vi.fn();
    render(<MemoryItemCard {...baseProps({ onClick, onDelete })} />);
    fireEvent.click(screen.getByRole("button", { name: /delete preference memory/i }));
    expect(onClick).not.toHaveBeenCalled();
  });

  it("disables the delete button while a delete is in flight", async () => {
    let resolve: (() => void) | null = null;
    const onDelete = vi.fn(
      () => new Promise<void>((res) => { resolve = res; })
    );
    render(<MemoryItemCard {...baseProps({ onDelete })} />);
    const btn = screen.getByRole("button", { name: /delete preference memory/i });
    fireEvent.click(btn);
    await waitFor(() => expect(btn).toBeDisabled());
    resolve?.();
    await waitFor(() => expect(btn).not.toBeDisabled());
  });
});
