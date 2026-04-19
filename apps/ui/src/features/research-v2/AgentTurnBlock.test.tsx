import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { AgentTurnBlock } from "./AgentTurnBlock";
import type { AgentTurn } from "./types";

function makeRoot(overrides: Partial<AgentTurn> = {}): AgentTurn {
  return {
    id: "exec-root",
    agentId: "root",
    parentExecutionId: null,
    startedAt: 1000,
    completedAt: 2000,
    status: "completed",
    wardId: "w1",
    request: null,
    timeline: [],
    tokenCount: 0,
    respond: "Final answer.",
    respondStreaming: "",
    thinkingExpanded: false,
    errorMessage: null,
    ...overrides,
  };
}

function makeChild(overrides: Partial<AgentTurn> = {}): AgentTurn {
  return {
    ...makeRoot({ id: "exec-child-1", agentId: "planner-agent", parentExecutionId: "exec-root" }),
    ...overrides,
  };
}

describe("<AgentTurnBlock> (root)", () => {
  it("renders the final respond markdown and a copy button", () => {
    render(<AgentTurnBlock turn={makeRoot()} />);
    expect(screen.getByText("Final answer.")).toBeTruthy();
    expect(screen.getByRole("button", { name: /copy response/i })).toBeTruthy();
  });

  it("does NOT render a thinking chevron or tool timeline on root", () => {
    render(<AgentTurnBlock turn={makeRoot()} />);
    expect(screen.queryByTestId(/thinking-chevron/)).toBeNull();
    expect(screen.queryByTestId(/timeline/)).toBeNull();
  });

  it("renders nested subagent cards when childTurns are provided", () => {
    const root = makeRoot();
    const child = makeChild({
      id: "exec-c1",
      request: "Make a plan.",
      respond: "Plan done.",
    });
    render(
      <AgentTurnBlock
        turn={root}
        childTurns={[child]}
        allTurns={[root, child]}
      />,
    );
    expect(screen.getByText("planner-agent")).toBeTruthy();
    expect(screen.getByText("Request")).toBeTruthy();
    expect(screen.getByText("Response")).toBeTruthy();
    expect(screen.getByText("Make a plan.")).toBeTruthy();
    expect(screen.getByText("Plan done.")).toBeTruthy();
  });

  it("subagent running → Response body shows 'waiting…' instead of content", () => {
    const root = makeRoot();
    const child = makeChild({
      id: "exec-c1",
      request: "Pending task.",
      status: "running",
      respond: null,
      respondStreaming: "",
      completedAt: null,
    });
    render(
      <AgentTurnBlock
        turn={root}
        childTurns={[child]}
        allTurns={[root, child]}
      />,
    );
    expect(screen.getByText("Pending task.")).toBeTruthy();
    expect(screen.getByText(/waiting/i)).toBeTruthy();
  });

  it("does not render a copy-response button when respond/streaming are empty", () => {
    const root = makeRoot({ respond: null, respondStreaming: "" });
    render(<AgentTurnBlock turn={root} />);
    expect(screen.queryByRole("button", { name: /copy response/i })).toBeNull();
  });

  it("accepts onToggleThinking as an optional no-op for API stability", () => {
    const spy = vi.fn();
    render(<AgentTurnBlock turn={makeRoot()} onToggleThinking={spy} />);
    expect(spy).not.toHaveBeenCalled();
  });
});
