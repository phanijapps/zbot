// ============================================================================
// AccentPicker — UI behaviour tests
// ============================================================================

import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@/test/utils";
import { AccentPicker } from "./AccentPicker";

describe("AccentPicker", () => {
  beforeEach(() => {
    window.localStorage.clear();
    document.documentElement.style.removeProperty("--fx-accent");
  });

  it("renders the trigger with the current accent label", () => {
    render(<AccentPicker />);
    const trigger = screen.getByRole("button", { name: /theme accent: cyan/i });
    expect(trigger).toBeInTheDocument();
    expect(trigger).toHaveAttribute("aria-expanded", "false");
  });

  it("opens the popover with all four swatches when clicked", () => {
    render(<AccentPicker />);
    fireEvent.click(screen.getByRole("button", { name: /theme accent/i }));
    const options = screen.getAllByRole("menuitemradio");
    expect(options).toHaveLength(4);
    expect(options.map((o) => o.getAttribute("title"))).toEqual([
      "Cyan",
      "Violet",
      "Amber",
      "Magenta",
    ]);
  });

  it("marks the active swatch with aria-checked + closes on selection", () => {
    render(<AccentPicker />);
    fireEvent.click(screen.getByRole("button", { name: /theme accent/i }));
    const cyan = screen.getByRole("menuitemradio", { name: /cyan/i });
    expect(cyan).toHaveAttribute("aria-checked", "true");

    const violet = screen.getByRole("menuitemradio", { name: /violet/i });
    fireEvent.click(violet);

    // Popover closes
    expect(screen.queryByRole("menuitemradio", { name: /cyan/i })).not.toBeInTheDocument();

    // Live var + persistence
    expect(document.documentElement.style.getPropertyValue("--fx-accent").trim()).toBe("#a78bff");
    expect(window.localStorage.getItem("agentzero-accent")).toBe("violet");
  });

  it("closes when Escape is pressed", () => {
    render(<AccentPicker />);
    fireEvent.click(screen.getByRole("button", { name: /theme accent/i }));
    expect(screen.getAllByRole("menuitemradio")).toHaveLength(4);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("menuitemradio")).not.toBeInTheDocument();
  });
});
