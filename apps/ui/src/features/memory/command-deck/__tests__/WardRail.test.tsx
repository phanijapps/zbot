import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { WardRail } from "../WardRail";

const wards = [
  { id: "literature-library", count: 142 },
  { id: "equity-valuation", count: 89 },
  { id: "__global__", count: 34 },
];

describe("WardRail", () => {
  it("lists wards, marks active, and fires onSelect", () => {
    const onSelect = vi.fn();
    render(<WardRail wards={wards} activeId="literature-library" onSelect={onSelect} />);
    const active = screen.getByText("literature-library");
    expect(active.closest("button")).toHaveAttribute("aria-current", "true");
    fireEvent.click(screen.getByText("equity-valuation"));
    expect(onSelect).toHaveBeenCalledWith("equity-valuation");
  });

  it("separates global wards under a GLOBAL heading", () => {
    render(<WardRail wards={wards} activeId="" onSelect={() => {}} />);
    expect(screen.getByText(/^GLOBAL$/)).toBeInTheDocument();
    expect(screen.getByText("__global__")).toBeInTheDocument();
  });
});
