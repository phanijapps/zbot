// ============================================================================
// SessionListPanel — render, search, filter toggles, selection, sort, pure
// applyFilters helper.
// ============================================================================

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@/test/utils";
import {
  SessionListPanel,
  applyFilters,
  DEFAULT_FILTERS,
} from "./SessionListPanel";
import type { LogSession } from "@/services/transport/types";

function makeSession(overrides: Partial<LogSession> = {}): LogSession {
  return {
    session_id: `s-${Math.random().toString(36).slice(2, 8)}`,
    conversation_id: `c-${Math.random().toString(36).slice(2, 8)}`,
    agent_id: "agent:root",
    agent_name: "root",
    started_at: new Date().toISOString(),
    status: "completed",
    token_count: 0,
    tool_call_count: 0,
    error_count: 0,
    child_session_ids: [],
    ...overrides,
  };
}

describe("applyFilters", () => {
  it("keeps sessions whose status bucket is enabled in the filters", () => {
    const sessions = [
      makeSession({ status: "running" }),
      makeSession({ status: "completed" }),
      makeSession({ status: "error" }),
    ];
    // FAILED chip is on by default — error maps to failed bucket and stays in.
    const result = applyFilters(sessions, DEFAULT_FILTERS);
    expect(result).toHaveLength(3);
  });

  it("hides failed (error/stopped) sessions when the FAILED chip is off", () => {
    const sessions = [
      makeSession({ status: "running" }),
      makeSession({ status: "error" }),
      makeSession({ status: "stopped" }),
    ];
    const filters = {
      ...DEFAULT_FILTERS,
      status: { ...DEFAULT_FILTERS.status, failed: false },
    };
    const result = applyFilters(sessions, filters);
    expect(result).toHaveLength(1);
    expect(result[0].status).toBe("running");
  });

  it("filters by search query against title + agent_name + id (case-insensitive)", () => {
    const sessions = [
      makeSession({ title: "refactor auth ward", agent_name: "code-agent" }),
      makeSession({ title: "summarize Q4 reports", agent_name: "researcher" }),
      makeSession({ title: "deploy", agent_name: "ops" }),
    ];
    const filters = { ...DEFAULT_FILTERS, search: "REFACTOR" };
    const result = applyFilters(sessions, filters);
    expect(result).toHaveLength(1);
    expect(result[0].title).toBe("refactor auth ward");
  });

  it("sorts results newest-first by started_at", () => {
    const older = makeSession({ title: "older", started_at: "2026-04-25T10:00:00Z" });
    const newer = makeSession({ title: "newer", started_at: "2026-04-25T20:00:00Z" });
    const result = applyFilters([older, newer], DEFAULT_FILTERS);
    expect(result[0].title).toBe("newer");
    expect(result[1].title).toBe("older");
  });

  it("empty status filters returns no rows of those statuses", () => {
    const sessions = [
      makeSession({ status: "running" }),
      makeSession({ status: "completed" }),
    ];
    const filters = {
      ...DEFAULT_FILTERS,
      status: { running: false, queued: false, completed: false, failed: false, paused: false },
    };
    expect(applyFilters(sessions, filters)).toHaveLength(0);
  });
});

describe("SessionListPanel", () => {
  function shell(sessions: LogSession[], selectedId: string | null = null) {
    const onSearchChange = vi.fn();
    const onStatusToggle = vi.fn();
    const onSelect = vi.fn();
    const result = render(
      <SessionListPanel
        sessions={sessions}
        selectedId={selectedId}
        filters={DEFAULT_FILTERS}
        onSearchChange={onSearchChange}
        onStatusToggle={onStatusToggle}
        onSelect={onSelect}
      />
    );
    return { ...result, onSearchChange, onStatusToggle, onSelect };
  }

  it("renders the search input + 5 status chips", () => {
    shell([]);
    expect(screen.getByPlaceholderText(/search sessions/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "RUNNING" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "QUEUED" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "DONE" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "FAILED" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "PAUSED" })).toBeInTheDocument();
  });

  it("renders one row per session that passes the filters", () => {
    shell([
      makeSession({ title: "first", status: "running" }),
      makeSession({ title: "second", status: "completed" }),
    ]);
    expect(screen.getByText("first")).toBeInTheDocument();
    expect(screen.getByText("second")).toBeInTheDocument();
  });

  it("calls onSearchChange when typing in the search input", () => {
    const { onSearchChange } = shell([]);
    fireEvent.change(screen.getByPlaceholderText(/search sessions/i), {
      target: { value: "auth" },
    });
    expect(onSearchChange).toHaveBeenCalledWith("auth");
  });

  it("calls onStatusToggle with the chip key when a chip is clicked", () => {
    const { onStatusToggle } = shell([]);
    fireEvent.click(screen.getByRole("button", { name: "PAUSED" }));
    expect(onStatusToggle).toHaveBeenCalledWith("paused");
  });

  it("calls onSelect with the session id when a row is clicked", () => {
    const sessions = [makeSession({ session_id: "abc-1234567890", title: "row" })];
    const { onSelect } = shell(sessions);
    fireEvent.click(screen.getByText("row").closest("button")!);
    expect(onSelect).toHaveBeenCalledWith("abc-1234567890");
  });

  it("marks the active row with aria-current=true", () => {
    const sessions = [
      makeSession({ session_id: "alpha-1", title: "alpha" }),
      makeSession({ session_id: "beta-2", title: "beta" }),
    ];
    shell(sessions, "beta-2");
    const beta = screen.getByText("beta").closest("button")!;
    const alpha = screen.getByText("alpha").closest("button")!;
    expect(beta.getAttribute("aria-current")).toBe("true");
    expect(alpha.getAttribute("aria-current")).toBeNull();
  });

  it("renders an empty-state message when no sessions match", () => {
    shell([]);
    expect(screen.getByText(/no sessions match/i)).toBeInTheDocument();
  });

  it("renders an in/out token pair in the row meta when tokenIndex provides totals", () => {
    const onSearchChange = vi.fn();
    const onStatusToggle = vi.fn();
    const onSelect = vi.fn();
    const tokenIndex = {
      byRootExecId: new Map([
        ["abc-with-tokens", { in: 8500, out: 250, total: 8750 }],
      ]),
      executionsByRootExecId: new Map(),
    };
    render(
      <SessionListPanel
        sessions={[makeSession({ session_id: "abc-with-tokens", title: "row" })]}
        selectedId={null}
        filters={DEFAULT_FILTERS}
        tokenIndex={tokenIndex}
        onSearchChange={onSearchChange}
        onStatusToggle={onStatusToggle}
        onSelect={onSelect}
      />
    );
    expect(screen.getByText(/8\.5k/)).toBeInTheDocument();
    expect(screen.getByText(/250/)).toBeInTheDocument();
  });

  it("hides the token pair when tokenIndex is absent or total is 0", () => {
    shell([makeSession({ session_id: "abc-zero", title: "row" })]);
    expect(screen.queryByText(/↓/)).not.toBeInTheDocument();
    expect(screen.queryByText(/↑/)).not.toBeInTheDocument();
  });
});
