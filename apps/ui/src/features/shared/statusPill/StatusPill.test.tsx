import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { StatusPill } from "./StatusPill";
import { EMPTY_PILL } from "./types";

describe("<StatusPill>", () => {
  it("renders nothing when not visible", () => {
    const { container } = render(<StatusPill state={EMPTY_PILL} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders narration only (no terminal row) when suffix is empty", () => {
    render(
      <StatusPill
        state={{
          ...EMPTY_PILL,
          visible: true,
          narration: "Thinking…",
          suffix: "",
          category: "neutral",
          swapCounter: 1,
        }}
      />
    );
    expect(screen.getByText("Thinking…")).toBeTruthy();
    expect(screen.queryByTestId("status-pill-terminal")).toBeNull();
  });

  it("renders narration + terminal row when suffix is present", () => {
    render(
      <StatusPill
        state={{
          ...EMPTY_PILL,
          visible: true,
          narration: "Running shell",
          suffix: "ls -la ~",
          category: "read",
          swapCounter: 1,
        }}
      />
    );
    expect(screen.getByText("Running shell")).toBeTruthy();
    expect(screen.getByTestId("status-pill-terminal")).toBeTruthy();
    expect(screen.getByText("ls -la ~")).toBeTruthy();
    expect(screen.getByText("$")).toBeTruthy();
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
