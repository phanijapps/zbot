import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { AgentTurnBlock } from "./AgentTurnBlock";
import type { AgentTurn } from "./types";

const baseTurn: AgentTurn = {
  id: "t1",
  agentId: "planner",
  parentExecutionId: null,
  startedAt: 1000,
  completedAt: 2000,
  status: "completed",
  wardId: "w1",
  timeline: [
    { id: "e1", at: 1100, kind: "thinking", text: "analyzing" },
    { id: "e2", at: 1200, kind: "tool_call", text: "write_file", toolName: "write_file", toolArgsPreview: "path=a.py" },
  ],
  tokenCount: 100,
  respond: "# Plan\n\nDone.",
  respondStreaming: "",
  thinkingExpanded: false,
  errorMessage: null,
};

describe("<AgentTurnBlock>", () => {
  it("renders agent id, status icon, and Respond markdown", () => {
    render(<AgentTurnBlock turn={baseTurn} onToggleThinking={() => {}} />);
    expect(screen.getByText(/planner/)).toBeTruthy();
    expect(screen.getByText("Done.")).toBeTruthy();
  });

  it("shows thinking count when collapsed and fires toggle on click", () => {
    const fn = vi.fn();
    render(<AgentTurnBlock turn={baseTurn} onToggleThinking={fn} />);
    const chevron = screen.getByTestId("thinking-chevron-t1");
    expect(chevron.textContent).toContain("Thinking (2");
    fireEvent.click(chevron);
    expect(fn).toHaveBeenCalledWith("t1");
  });

  it("expands the timeline when thinkingExpanded is true", () => {
    const expanded = { ...baseTurn, thinkingExpanded: true };
    const { container } = render(<AgentTurnBlock turn={expanded} onToggleThinking={() => {}} />);
    expect(container.querySelector(".thinking-timeline")).toBeTruthy();
  });

  it("shows streaming buffer when respond is null", () => {
    const streaming = { ...baseTurn, respond: null, respondStreaming: "partial text" };
    render(<AgentTurnBlock turn={streaming} onToggleThinking={() => {}} />);
    expect(screen.getByText(/partial text/)).toBeTruthy();
  });

  it("shows waiting placeholder when respond null and no streaming buffer", () => {
    const waiting = { ...baseTurn, respond: null, respondStreaming: "", status: "running" as const, completedAt: null };
    render(<AgentTurnBlock turn={waiting} onToggleThinking={() => {}} />);
    expect(screen.getByText(/waiting/)).toBeTruthy();
  });

  it("shows running badge when status is running", () => {
    const running = { ...baseTurn, status: "running" as const, completedAt: null };
    render(<AgentTurnBlock turn={running} onToggleThinking={() => {}} />);
    expect(screen.getByTestId("turn-running-badge")).toBeTruthy();
  });

  it("renders the error banner instead of Respond when status is error", () => {
    const errored = {
      ...baseTurn,
      status: "error" as const,
      respond: null,
      errorMessage: "Turn ended with no output (provider error or context limit)",
    };
    render(<AgentTurnBlock turn={errored} onToggleThinking={() => {}} />);
    expect(screen.getByTestId("turn-error-banner")).toBeTruthy();
    expect(screen.getByText(/no output/)).toBeTruthy();
    // Respond markdown should NOT render when the banner is showing.
    expect(screen.queryByText("Done.")).toBeNull();
  });

  it("carries the parentExecutionId via data-parent attribute", () => {
    const child = { ...baseTurn, id: "t2", parentExecutionId: "t1" };
    const { container } = render(<AgentTurnBlock turn={child} onToggleThinking={() => {}} />);
    const block = container.querySelector(".agent-turn-block");
    expect(block?.getAttribute("data-parent")).toBe("t1");
  });

  it("uses empty data-parent attribute when parentExecutionId is null", () => {
    const { container } = render(<AgentTurnBlock turn={baseTurn} onToggleThinking={() => {}} />);
    const block = container.querySelector(".agent-turn-block");
    expect(block?.getAttribute("data-parent")).toBe("");
  });
});
