// ============================================================================
// AgentToolGroup — recursive rendering, tone helpers, status badges
// ============================================================================

import { describe, it, expect } from "vitest";
import userEvent from "@testing-library/user-event";
import { render, screen } from "@/test/utils";
import { AgentToolGroup, avatarLabel, avatarTone } from "./AgentToolGroup";
import type { TraceNode } from "../logs/trace-types";

function tool(label: string, status: TraceNode["status"] = "completed"): TraceNode {
  return {
    id: `tool-${label}`,
    type: "tool_call",
    agentId: "agent:code",
    label,
    durationMs: 12,
    status,
    timestamp: new Date().toISOString(),
    children: [],
  };
}

function delegation(
  agentId: string,
  status: TraceNode["status"],
  children: TraceNode[],
): TraceNode {
  return {
    id: `del-${agentId}`,
    type: "delegation",
    agentId,
    label: agentId,
    durationMs: 1000,
    status,
    timestamp: new Date().toISOString(),
    children,
  };
}

function root(children: TraceNode[]): TraceNode {
  return {
    id: "root-1",
    type: "root",
    agentId: "agent:root",
    label: "root",
    durationMs: 5000,
    status: "running",
    timestamp: new Date().toISOString(),
    children,
  };
}

describe("AgentToolGroup", () => {
  it("renders the agent name + status + tool count in the header", () => {
    render(<AgentToolGroup node={root([tool("read_file"), tool("grep")])} />);
    expect(screen.getByText("root")).toBeInTheDocument();
    expect(screen.getByText("running")).toBeInTheDocument();
    expect(screen.getByText("2 tools")).toBeInTheDocument();
  });

  it("renders a row per tool call", () => {
    render(<AgentToolGroup node={root([tool("read_file"), tool("grep"), tool("write_file")])} />);
    expect(screen.getByText("read_file")).toBeInTheDocument();
    expect(screen.getByText("grep")).toBeInTheDocument();
    expect(screen.getByText("write_file")).toBeInTheDocument();
  });

  it("renders nested subagents recursively, each as its own card", () => {
    const tree = root([
      tool("delegate"),
      delegation("code-agent", "running", [tool("read_file"), tool("write_file")]),
      delegation("researcher", "completed", [tool("web_search")]),
    ]);
    render(<AgentToolGroup node={tree} />);
    expect(screen.getByText("root")).toBeInTheDocument();
    expect(screen.getByText("code-agent")).toBeInTheDocument();
    expect(screen.getByText("researcher")).toBeInTheDocument();
    // root's delegate-note shows the count
    expect(screen.getByText(/delegated 2 subagents/i)).toBeInTheDocument();
  });

  it("renders a grandchild subagent inside its parent's card", () => {
    // TraceNode.status doesn't include "queued"; undefined represents the
    // "not yet started" case (mapped to the queued bucket by AgentToolGroup).
    const grandchild = delegation("unit-test-writer", undefined, []);
    const tree = root([
      delegation("code-agent", "running", [tool("read_file"), grandchild]),
    ]);
    render(<AgentToolGroup node={tree} />);
    expect(screen.getByText("code-agent")).toBeInTheDocument();
    expect(screen.getByText("unit-test-writer")).toBeInTheDocument();
  });

  it("shows the in-flight cursor on a running tool", () => {
    const tree = root([tool("write_file", "running")]);
    render(<AgentToolGroup node={tree} />);
    expect(screen.getByLabelText(/in flight/i)).toBeInTheDocument();
  });

  it("shows a check + duration for a completed tool", () => {
    const tree = root([tool("read_file", "completed")]);
    render(<AgentToolGroup node={tree} />);
    expect(screen.getByText(/✓ 12ms/)).toBeInTheDocument();
  });

  it("renders a single 'tool' label correctly", () => {
    render(<AgentToolGroup node={root([tool("read_file")])} />);
    expect(screen.getByText("1 tool")).toBeInTheDocument();
    expect(screen.queryByText("1 tools")).not.toBeInTheDocument();
  });

  it("renders per-subagent token count in the agent header meta when > 0", () => {
    const subWithTokens: TraceNode = {
      id: "sub-1",
      type: "delegation",
      agentId: "researcher",
      label: "researcher",
      durationMs: 3200,
      tokenCount: 12_500,
      status: "completed",
      timestamp: new Date().toISOString(),
      children: [tool("web_search")],
    };
    render(<AgentToolGroup node={root([subWithTokens])} />);
    expect(screen.getByText(/12\.5k tok/)).toBeInTheDocument();
  });

  it("hides the token meta when tokenCount is undefined or 0", () => {
    const subNoTokens: TraceNode = {
      id: "sub-1",
      type: "delegation",
      agentId: "code-agent",
      label: "code-agent",
      status: "running",
      timestamp: new Date().toISOString(),
      children: [],
    };
    const { container } = render(<AgentToolGroup node={root([subNoTokens])} />);
    // No "tok" text anywhere in the rendered tree.
    expect(container.textContent).not.toMatch(/\d+\s*tok/);
  });

  it("renders the delegate count in plural for 2+ subagents", () => {
    const tree = root([
      delegation("code-agent", "running", []),
      delegation("researcher", "completed", []),
    ]);
    render(<AgentToolGroup node={tree} />);
    expect(screen.getByText(/delegated 2 subagents/i)).toBeInTheDocument();
  });

  it("collapses every subagent except the latest by default", () => {
    const tree = root([
      delegation("first-agent", "completed", [tool("read_file")]),
      delegation("middle-agent", "completed", [tool("write_file")]),
      delegation("latest-agent", "running", [tool("grep")]),
    ]);
    render(<AgentToolGroup node={tree} />);
    // Headers always render
    expect(screen.getByText("first-agent")).toBeInTheDocument();
    expect(screen.getByText("middle-agent")).toBeInTheDocument();
    expect(screen.getByText("latest-agent")).toBeInTheDocument();
    // Bodies: latest is expanded → its tool is visible; earlier two are not.
    expect(screen.getByText("grep")).toBeInTheDocument();
    expect(screen.queryByText("read_file")).not.toBeInTheDocument();
    expect(screen.queryByText("write_file")).not.toBeInTheDocument();
  });

  it("expands a collapsed subagent when its header is clicked", async () => {
    const user = userEvent.setup();
    const tree = root([
      delegation("first-agent", "completed", [tool("hidden_tool")]),
      delegation("latest-agent", "running", [tool("visible_tool")]),
    ]);
    render(<AgentToolGroup node={tree} />);
    expect(screen.queryByText("hidden_tool")).not.toBeInTheDocument();
    const header = screen.getByRole("button", { name: /Expand first-agent/i });
    await user.click(header);
    expect(screen.getByText("hidden_tool")).toBeInTheDocument();
  });

  it("never collapses the root group", () => {
    const tree = root([tool("read_file"), tool("grep")]);
    render(<AgentToolGroup node={tree} />);
    // The root header is not a button — it's a plain header.
    expect(screen.queryByRole("button", { name: /Expand|Collapse/i })).not.toBeInTheDocument();
    expect(screen.getByText("read_file")).toBeInTheDocument();
    expect(screen.getByText("grep")).toBeInTheDocument();
  });

  it("indents children: depth class on the recursed AgentToolGroup", () => {
    const tree = root([delegation("code-agent", "running", [delegation("ut", undefined, [])])]);
    const { container } = render(<AgentToolGroup node={tree} />);
    expect(container.querySelector(".agent-tool-group--depth-0")).not.toBeNull();
    expect(container.querySelector(".agent-tool-group--depth-1")).not.toBeNull();
    expect(container.querySelector(".agent-tool-group--depth-2")).not.toBeNull();
  });
});

describe("avatarLabel", () => {
  it("strips agent: prefix and uppercases", () => {
    expect(avatarLabel("agent:root")).toBe("RO");
    expect(avatarLabel("code-agent")).toBe("CA");
    expect(avatarLabel("researcher")).toBe("RE");
  });

  it("handles single-letter ids", () => {
    expect(avatarLabel("x")).toBe("X·");
  });

  it("handles empty / undefined-ish ids", () => {
    expect(avatarLabel("")).toBe("··");
  });

  it("picks first two alpha-numeric characters across separators", () => {
    expect(avatarLabel("unit-test-writer")).toBe("UT");
    expect(avatarLabel("plan_step_2")).toBe("PS");
  });
});

describe("avatarTone", () => {
  it("returns root tone for depth 0 regardless of id", () => {
    expect(avatarTone("agent:root", 0)).toBe("root");
    expect(avatarTone("code-agent", 0)).toBe("root");
  });

  it("identifies research agents at depth >= 1", () => {
    expect(avatarTone("researcher", 1)).toBe("research");
    expect(avatarTone("research-helper", 2)).toBe("research");
  });

  it("identifies planner-like agents at depth >= 1", () => {
    expect(avatarTone("planner", 1)).toBe("planner");
    expect(avatarTone("unit-test-writer", 1)).toBe("planner");
    expect(avatarTone("tutor", 1)).toBe("planner");
  });

  it("falls back to code tone for unknown agents", () => {
    expect(avatarTone("code-agent", 1)).toBe("code");
    expect(avatarTone("foo", 1)).toBe("code");
  });
});
