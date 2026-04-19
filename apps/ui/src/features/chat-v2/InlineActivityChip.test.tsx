import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { InlineActivityChip } from "./InlineActivityChip";

describe("<InlineActivityChip>", () => {
  it("renders recall chip with label", () => {
    render(<InlineActivityChip chip={{ id: "c1", kind: "recall", label: "recalled 2" }} />);
    expect(screen.getByText("recalled 2")).toBeTruthy();
  });

  it("exposes kind via data-kind attribute", () => {
    const { container } = render(
      <InlineActivityChip chip={{ id: "c1", kind: "delegate", label: "→ writer-agent" }} />
    );
    const chip = container.querySelector(".quick-chat__chip");
    expect(chip?.getAttribute("data-kind")).toBe("delegate");
  });

  it("applies detail as title attribute when present", () => {
    const { container } = render(
      <InlineActivityChip chip={{ id: "c1", kind: "skill", label: "loaded web-read", detail: "fetched 3 urls" }} />
    );
    const chip = container.querySelector(".quick-chat__chip");
    expect(chip?.getAttribute("title")).toBe("fetched 3 urls");
  });
});
