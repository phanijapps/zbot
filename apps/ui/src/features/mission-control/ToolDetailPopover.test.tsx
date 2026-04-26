// ============================================================================
// ToolDetailPopover — render, close handlers, JSON pretty-print.
// ============================================================================

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@/test/utils";
import { ToolDetailPopover, formatJson } from "./ToolDetailPopover";
import type { TraceNode } from "../logs/trace-types";

function makeTool(overrides: Partial<TraceNode> = {}): TraceNode {
  return {
    id: "tool-1",
    type: "tool_call",
    agentId: "code-agent",
    label: "read_file",
    summary: "src/auth.rs",
    durationMs: 12,
    status: "completed",
    timestamp: new Date().toISOString(),
    args: '{"path":"src/auth.rs"}',
    result: '{"content":"pub fn auth() {}"}',
    children: [],
    ...overrides,
  };
}

describe("ToolDetailPopover", () => {
  it("renders nothing when tool is null", () => {
    const { container } = render(<ToolDetailPopover tool={null} onClose={() => {}} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders tool name + summary in the header", () => {
    render(<ToolDetailPopover tool={makeTool()} onClose={() => {}} />);
    expect(screen.getByText("read_file")).toBeInTheDocument();
    expect(screen.getByText("src/auth.rs")).toBeInTheDocument();
  });

  it("renders agent + duration + status meta stats", () => {
    render(<ToolDetailPopover tool={makeTool()} onClose={() => {}} />);
    expect(screen.getByText("code-agent")).toBeInTheDocument();
    expect(screen.getByText("12ms")).toBeInTheDocument();
    expect(screen.getByText("completed")).toBeInTheDocument();
  });

  it("renders Input + Output sections with the tool's args and result", () => {
    render(<ToolDetailPopover tool={makeTool()} onClose={() => {}} />);
    expect(screen.getByText("Input")).toBeInTheDocument();
    expect(screen.getByText("Output")).toBeInTheDocument();
    // JSON gets pretty-printed → "path" appears on its own line.
    expect(screen.getByText(/"path": "src\/auth\.rs"/)).toBeInTheDocument();
    expect(screen.getByText(/"content": "pub fn auth\(\) \{\}"/)).toBeInTheDocument();
  });

  it("shows an Error section when tool.error is set", () => {
    render(
      <ToolDetailPopover
        tool={makeTool({ error: "permission denied", status: "error" })}
        onClose={() => {}}
      />,
    );
    expect(screen.getByText("Error")).toBeInTheDocument();
    expect(screen.getByText("permission denied")).toBeInTheDocument();
  });

  it("shows 'empty' placeholder when args/result are missing", () => {
    render(
      <ToolDetailPopover
        tool={makeTool({ args: undefined, result: undefined })}
        onClose={() => {}}
      />,
    );
    const placeholders = screen.getAllByText(/^empty$/i);
    // Both Input and Output are empty.
    expect(placeholders).toHaveLength(2);
  });

  it("calls onClose when the X button is clicked", () => {
    const onClose = vi.fn();
    render(<ToolDetailPopover tool={makeTool()} onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: /^close$/i }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("calls onClose when the backdrop is clicked", () => {
    const onClose = vi.fn();
    render(<ToolDetailPopover tool={makeTool()} onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: /close tool details/i }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("calls onClose when Escape is pressed", () => {
    const onClose = vi.fn();
    render(<ToolDetailPopover tool={makeTool()} onClose={onClose} />);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("dialog has aria-modal + descriptive aria-label", () => {
    render(<ToolDetailPopover tool={makeTool()} onClose={() => {}} />);
    const dialog = screen.getByRole("dialog");
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(dialog.getAttribute("aria-label")).toMatch(/read_file tool details/i);
  });
});

describe("formatJson", () => {
  it("returns empty string for null/undefined/empty input", () => {
    expect(formatJson(null)).toBe("");
    expect(formatJson(undefined)).toBe("");
    expect(formatJson("")).toBe("");
    expect(formatJson("   ")).toBe("");
  });

  it("returns the input unchanged when it doesn't start with { or [", () => {
    expect(formatJson("plain text")).toBe("plain text");
    expect(formatJson("123")).toBe("123");
  });

  it("pretty-prints valid JSON objects with 2-space indent", () => {
    expect(formatJson('{"a":1,"b":2}')).toBe('{\n  "a": 1,\n  "b": 2\n}');
  });

  it("pretty-prints valid JSON arrays", () => {
    expect(formatJson('[1,2,3]')).toBe('[\n  1,\n  2,\n  3\n]');
  });

  it("returns input unchanged when parsing fails (defensive)", () => {
    expect(formatJson("{not json}")).toBe("{not json}");
  });
});
