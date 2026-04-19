import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { StatusPill } from "./StatusPill";
import { EMPTY_PILL } from "./types";

describe("<StatusPill>", () => {
  it("renders nothing when not visible", () => {
    const { container } = render(<StatusPill state={EMPTY_PILL} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders narration + suffix when visible", () => {
    render(
      <StatusPill
        state={{
          ...EMPTY_PILL,
          visible: true,
          narration: "Recalling fundamentals",
          suffix: "· memory",
          category: "read",
          swapCounter: 1,
        }}
      />
    );
    expect(screen.getByText("Recalling fundamentals")).toBeTruthy();
    expect(screen.getByText("· memory")).toBeTruthy();
  });

  it("applies category data attribute", () => {
    render(
      <StatusPill
        state={{
          ...EMPTY_PILL,
          visible: true,
          narration: "Responding",
          category: "respond",
          swapCounter: 1,
        }}
      />
    );
    const pill = screen.getByTestId("status-pill");
    expect(pill.getAttribute("data-category")).toBe("respond");
  });
});
