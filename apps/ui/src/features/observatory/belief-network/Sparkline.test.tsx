// ============================================================================
// Sparkline — bar count + empty handling
// ============================================================================

import { describe, it, expect } from "vitest";
import { render, screen } from "@/test/utils";

import { Sparkline } from "./Sparkline";

describe("Sparkline", () => {
  it("renders one bar per value", () => {
    render(<Sparkline values={[1, 2, 3, 4, 5]} />);
    const bars = screen.getAllByTestId("sparkline-bar");
    expect(bars).toHaveLength(5);
  });

  it("renders zero bars when the values list is empty", () => {
    render(<Sparkline values={[]} />);
    expect(screen.queryAllByTestId("sparkline-bar")).toHaveLength(0);
    expect(screen.getByTestId("sparkline-empty")).toBeInTheDocument();
  });

  it("scales bars relative to the max value", () => {
    render(<Sparkline values={[0, 10]} height={20} />);
    const bars = screen.getAllByTestId("sparkline-bar");
    const heightAttr = (el: Element) => Number(el.getAttribute("height"));
    expect(heightAttr(bars[1])).toBeGreaterThan(heightAttr(bars[0]));
  });

  it("uses the supplied ariaLabel for accessibility", () => {
    render(<Sparkline values={[1, 2]} ariaLabel="cycles per minute" />);
    expect(screen.getByRole("img", { name: "cycles per minute" })).toBeInTheDocument();
  });
});
