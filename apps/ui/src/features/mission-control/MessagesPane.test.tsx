// ============================================================================
// MessagesPane — extractMessages helper + render behaviour
// Critical: ward_recommendation comes back from the gateway as either a
// string OR an object (research mode logs it as a structured object). The
// pane must never hand a raw object to React (would throw error #31).
// ============================================================================

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@/test/utils";
import { extractMessages, MessagesPane } from "./MessagesPane";
import type { LogSession, SessionDetail, ExecutionLog } from "@/services/transport/types";

const mockGetLogSession = vi.fn();

vi.mock("@/services/transport", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("@/services/transport");
  return {
    ...actual,
    getTransport: async () => ({
      getLogSession: mockGetLogSession,
    }),
  };
});

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(() => {
  vi.useRealTimers();
});

function makeSession(overrides: Partial<LogSession> = {}): LogSession {
  return {
    session_id: "sess-1",
    conversation_id: "conv-1",
    agent_id: "agent:root",
    agent_name: "root",
    started_at: "2026-04-25T20:00:00Z",
    status: "completed",
    token_count: 0,
    tool_call_count: 0,
    error_count: 0,
    child_session_ids: [],
    ...overrides,
  };
}

function makeLog(category: ExecutionLog["category"], overrides: Partial<ExecutionLog> = {}): ExecutionLog {
  return {
    id: `log-${Math.random().toString(36).slice(2, 8)}`,
    session_id: "sess-1",
    conversation_id: "conv-1",
    agent_id: "agent:root",
    timestamp: "2026-04-25T20:00:01Z",
    level: "info",
    category,
    message: "msg",
    ...overrides,
  };
}

function makeDetail(logs: ExecutionLog[], session?: Partial<LogSession>): SessionDetail {
  return {
    session: makeSession(session),
    logs,
  };
}

// ----------------------------------------------------------------------------
// extractMessages — pure tests
// ----------------------------------------------------------------------------

describe("extractMessages", () => {
  it("returns an empty array when detail is null", () => {
    expect(extractMessages(null)).toEqual([]);
  });

  it("uses the session title as the opening user prompt", () => {
    const detail = makeDetail([], { title: "Cold War proxy wars research" });
    const out = extractMessages(detail);
    expect(out[0]).toMatchObject({ category: "user", body: "Cold War proxy wars research" });
  });

  it("filters logs to intent / response / delegation / error categories only", () => {
    const detail = makeDetail([
      makeLog("intent", { id: "i", message: "intent body", timestamp: "2026-04-25T20:00:02Z" }),
      makeLog("tool_call", { id: "tc", message: "ignored" }),
      makeLog("tool_result", { id: "tr", message: "ignored" }),
      makeLog("response", { id: "r", message: "response body", timestamp: "2026-04-25T20:00:03Z" }),
      makeLog("delegation", { id: "d", message: "delegating to coder", timestamp: "2026-04-25T20:00:04Z" }),
      makeLog("error", { id: "e", message: "boom", timestamp: "2026-04-25T20:00:05Z" }),
      makeLog("session", { id: "s", message: "Session started" }),
    ], { title: "go" });
    const cats = extractMessages(detail).map((m) => m.category);
    expect(cats).toEqual(["user", "intent", "response", "delegation", "error"]);
  });

  it("sorts entries chronologically by timestamp", () => {
    const detail = makeDetail([
      makeLog("response", { id: "r", message: "resp", timestamp: "2026-04-25T20:00:09Z" }),
      makeLog("intent",   { id: "i", message: "int",  timestamp: "2026-04-25T20:00:02Z" }),
    ], { title: "first user", started_at: "2026-04-25T20:00:01Z" });
    const out = extractMessages(detail);
    const order = out.map((m) => m.id);
    // user (started_at 20:00:01) → intent (20:00:02) → response (20:00:09)
    expect(order[0]).toContain("__user");
    expect(order[1]).toBe("i");
    expect(order[2]).toBe("r");
  });

  it("preserves metadata so the renderer can pull primary_intent + ward_recommendation", () => {
    const detail = makeDetail([
      makeLog("intent", {
        id: "i",
        metadata: {
          primary_intent: "research_topic",
          ward_recommendation: { ward_name: "literature-library", action: "use_existing" },
        },
      }),
    ], { title: "go" });
    const out = extractMessages(detail);
    const intent = out.find((m) => m.id === "i")!;
    expect(intent.meta?.primary_intent).toBe("research_topic");
    expect((intent.meta?.ward_recommendation as Record<string, unknown>).ward_name).toBe("literature-library");
  });

  it("aggregates root + child session logs interleaved chronologically", () => {
    const root = makeDetail(
      [
        makeLog("intent", {
          id: "root-int", message: "root intent", agent_id: "agent:root",
          timestamp: "2026-04-25T20:00:02Z",
        }),
        makeLog("delegation", {
          id: "root-del", message: "delegating to researcher", agent_id: "agent:root",
          timestamp: "2026-04-25T20:00:03Z",
        }),
        makeLog("response", {
          id: "root-resp", message: "summary done", agent_id: "agent:root",
          timestamp: "2026-04-25T20:00:09Z",
        }),
      ],
      { title: "go", started_at: "2026-04-25T20:00:01Z" },
    );
    const child: SessionDetail = {
      session: makeSession({ session_id: "child-1", agent_id: "agent:researcher", agent_name: "researcher" }),
      logs: [
        makeLog("intent", {
          id: "child-int", message: "research intent", agent_id: "agent:researcher",
          timestamp: "2026-04-25T20:00:04Z",
        }),
        makeLog("response", {
          id: "child-resp", message: "research done", agent_id: "agent:researcher",
          timestamp: "2026-04-25T20:00:07Z",
        }),
      ],
    };
    const out = extractMessages(root, [child]);
    expect(out.map((m) => m.id)).toEqual([
      `${root.session.session_id}__user`,
      "root-int",
      "root-del",
      "child-int",
      "child-resp",
      "root-resp",
    ]);
    expect(out.find((m) => m.id === "child-resp")!.agent).toBe("agent:researcher");
  });
});

// ----------------------------------------------------------------------------
// MessagesPane — render behaviour
// ----------------------------------------------------------------------------

describe("MessagesPane", () => {
  it("shows the empty selector message when no session is selected", () => {
    render(<MessagesPane session={null} />);
    expect(screen.getByText(/select a session/i)).toBeInTheDocument();
  });

  it("renders user → intent → response timeline for a research session", async () => {
    mockGetLogSession.mockResolvedValue({
      success: true,
      data: makeDetail(
        [
          makeLog("intent", {
            id: "i",
            message: "Learn about the Cold War.",
            timestamp: "2026-04-25T20:00:02Z",
            metadata: { primary_intent: "research_topic", ward_recommendation: "history-ward" },
          }),
          makeLog("response", {
            id: "r",
            message: "Here is a summary of Cold War proxy conflicts.",
            timestamp: "2026-04-25T20:00:09Z",
          }),
        ],
        { title: "Cold War proxy wars research", started_at: "2026-04-25T20:00:01Z" },
      ),
    });
    render(<MessagesPane session={makeSession({ title: "Cold War proxy wars research" })} />);
    await waitFor(() => expect(screen.getByText(/Cold War proxy wars research/)).toBeInTheDocument());
    expect(screen.getByText(/Learn about the Cold War/)).toBeInTheDocument();
    expect(screen.getByText(/summary of Cold War proxy conflicts/)).toBeInTheDocument();
    expect(screen.getByText(/research_topic/)).toBeInTheDocument();
    expect(screen.getByText(/history-ward/)).toBeInTheDocument();
  });

  it("DOES NOT crash when ward_recommendation is an object (regression: React error #31)", async () => {
    mockGetLogSession.mockResolvedValue({
      success: true,
      data: makeDetail(
        [
          makeLog("intent", {
            id: "i",
            message: "Some intent.",
            metadata: {
              primary_intent: "research_topic",
              // The actual gateway shape that originally crashed the pane:
              ward_recommendation: {
                action: "create_subdirectory",
                reason: "Cold-war research is a sub-topic of history",
                structure: "history/cold-war/proxy-wars",
                subdirectory: "proxy-wars",
                ward_name: "history-ward",
              },
            },
          }),
        ],
        { title: "go" },
      ),
    });
    render(<MessagesPane session={makeSession()} />);
    // Should render the intent without throwing — and pull `ward_name`
    // out of the object so the user still sees a useful ward label.
    await waitFor(() => expect(screen.getByText(/research_topic/)).toBeInTheDocument());
    expect(screen.getByText(/history-ward/)).toBeInTheDocument();
    // The other object keys must not leak into the DOM as raw text.
    expect(screen.queryByText(/create_subdirectory/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Cold-war research is a sub-topic/)).not.toBeInTheDocument();
  });

  it("shows the Cached badge when the session is not running", async () => {
    mockGetLogSession.mockResolvedValue({
      success: true,
      data: makeDetail([], { title: "done" }),
    });
    render(<MessagesPane session={makeSession({ status: "completed" })} />);
    await waitFor(() => expect(screen.getByText(/cached/i)).toBeInTheDocument());
  });

  it("shows the Live badge when the session is running", async () => {
    mockGetLogSession.mockResolvedValue({
      success: true,
      data: makeDetail([], { title: "live" }),
    });
    render(<MessagesPane session={makeSession({ status: "running" })} />);
    await waitFor(() => expect(screen.getByText(/^live$/i)).toBeInTheDocument());
  });

  it("renders the empty-state message when the session has no message-like logs", async () => {
    mockGetLogSession.mockResolvedValue({
      success: true,
      data: { session: makeSession({ title: undefined }), logs: [] },
    });
    render(<MessagesPane session={makeSession({ title: undefined })} />);
    await waitFor(() =>
      expect(screen.getByText(/no intent or response logged/i)).toBeInTheDocument(),
    );
  });
});
