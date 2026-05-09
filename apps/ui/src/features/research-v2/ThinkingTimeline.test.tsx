// ============================================================================
// ThinkingTimeline — render tests
// ============================================================================

import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ThinkingTimeline } from "./ThinkingTimeline";
import type { TimelineEntry } from "./types";

function makeEntry(overrides: Partial<TimelineEntry> = {}): TimelineEntry {
  return {
    id: "entry-1",
    kind: "note",
    at: new Date("2024-01-15T12:00:00Z").getTime(),
    text: "Thinking...",
    ...overrides,
  };
}

describe("ThinkingTimeline", () => {
  it("renders empty state when entries array is empty", () => {
    render(<ThinkingTimeline entries={[]} />);
    expect(screen.getByText("no intermediate events")).toBeInTheDocument();
  });

  it("renders a list when entries are provided", () => {
    const entries = [
      makeEntry({ id: "e1", text: "Step one" }),
      makeEntry({ id: "e2", text: "Step two" }),
    ];
    render(<ThinkingTimeline entries={entries} />);
    expect(screen.getByText("Step one")).toBeInTheDocument();
    expect(screen.getByText("Step two")).toBeInTheDocument();
  });

  it("renders tool_call entries with tool name in code element", () => {
    const entry = makeEntry({
      id: "e1",
      kind: "tool_call",
      toolName: "bash_execute",
      toolArgsPreview: "ls -la",
    });
    render(<ThinkingTimeline entries={[entry]} />);
    expect(screen.getByText("bash_execute")).toBeInTheDocument();
    expect(screen.getByText("ls -la")).toBeInTheDocument();
  });

  it("renders tool_call without toolArgsPreview fine", () => {
    const entry = makeEntry({
      id: "e1",
      kind: "tool_call",
      toolName: "read_file",
    });
    render(<ThinkingTimeline entries={[entry]} />);
    expect(screen.getByText("read_file")).toBeInTheDocument();
  });

  it("renders tool_result entries with result preview", () => {
    const entry = makeEntry({
      id: "e1",
      kind: "tool_result",
      toolResultPreview: "file contents here",
    });
    render(<ThinkingTimeline entries={[entry]} />);
    expect(screen.getByText("file contents here")).toBeInTheDocument();
  });

  it("falls back to entry.text for tool_result when no toolResultPreview", () => {
    const entry = makeEntry({
      id: "e1",
      kind: "tool_result",
      text: "fallback text",
    });
    render(<ThinkingTimeline entries={[entry]} />);
    expect(screen.getByText("fallback text")).toBeInTheDocument();
  });

  it("applies correct CSS class for each entry kind", () => {
    const entries = [
      makeEntry({ id: "e1", kind: "note", text: "note entry" }),
      makeEntry({ id: "e2", kind: "tool_call", toolName: "mytool" }),
    ];
    const { container } = render(<ThinkingTimeline entries={entries} />);
    const items = container.querySelectorAll("li");
    expect(items[0].className).toContain("thinking-timeline__item--note");
    expect(items[1].className).toContain("thinking-timeline__item--tool_call");
  });

  it("renders the arrow label for tool_result entries", () => {
    const entry = makeEntry({
      id: "e1",
      kind: "tool_result",
      toolResultPreview: "result",
    });
    render(<ThinkingTimeline entries={[entry]} />);
    expect(screen.getByText("↳")).toBeInTheDocument();
  });
});
