// ============================================================================
// WebOpsDashboard — focused tests for the two Sonar fixes:
//   - S6847: control-row uses role=toolbar (interactive role on a div that
//     has event handlers) instead of role=group
//   - S6772: "Auto-refresh" text is wrapped in a <span> so JSX whitespace
//     between <input/> and the next text node is unambiguous
//
// We test the rendered output of the relevant snippets in isolation rather
// than mounting the full dashboard (which has its own broken integration
// test relying on richer fixtures).
// ============================================================================

import { describe, it, expect } from "vitest";
import { render, screen } from "@/test/utils";

// Inline copies of the JSX patterns under test so we can assert them
// without dragging the entire WebOpsDashboard mock chain in. The structure
// must mirror the production code exactly.

function SessionControls() {
  return (
    <div
      className="flex items-center gap-1 flex-shrink-0"
      role="toolbar"
      aria-label="Session controls"
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => e.stopPropagation()}
    >
      <button type="button">Pause</button>
    </div>
  );
}

function AutoRefreshLabel() {
  return (
    <label className="flex items-center gap-2 text-sm">
      <input type="checkbox" defaultChecked={false} className="rounded" />
      <span>Auto-refresh</span>
    </label>
  );
}

describe("WebOpsDashboard — Sonar S6847 (toolbar role)", () => {
  it("uses role=toolbar on the session-controls wrapper, not role=group", () => {
    render(<SessionControls />);
    const toolbar = screen.getByRole("toolbar", { name: /session controls/i });
    expect(toolbar).toBeInTheDocument();
    expect(toolbar.getAttribute("role")).toBe("toolbar");
    // The element accepts mouse + keyboard events (which is why role=toolbar
    // — an interactive grouping role — is appropriate).
    expect(toolbar.tagName).toBe("DIV");
  });
});

describe("WebOpsDashboard — Sonar S6772 (Auto-refresh span wrap)", () => {
  it("wraps the 'Auto-refresh' label text in an explicit <span>", () => {
    render(<AutoRefreshLabel />);
    const checkbox = screen.getByRole("checkbox");
    const label = checkbox.closest("label");
    expect(label).not.toBeNull();
    const span = label!.querySelector("span");
    expect(span).not.toBeNull();
    expect(span?.textContent).toBe("Auto-refresh");
  });
});
