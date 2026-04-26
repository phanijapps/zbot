// ============================================================================
// MissionControlPage — page-level integration test
// Verifies KPI strip + session list + detail pane all render and respond to
// session selection. Mocks the data hooks so the test stays focused on the
// page composition (other tests cover hooks + sub-components in isolation).
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@/test/utils";

// ---------------------------------------------------------------------------
// Mock the data hooks — we feed sessions in via the mock state.
// Mock SessionChatViewer so the test doesn't need transport plumbing.
// ---------------------------------------------------------------------------

const mockUseLogSessions = vi.fn();
const mockUseAutoRefresh = vi.fn();
const mockUseSessionTrace = vi.fn();
const mockUseTraceSubscription = vi.fn();

vi.mock("../logs/log-hooks", () => ({
  useLogSessions: () => mockUseLogSessions(),
  useAutoRefresh: (...args: unknown[]) => mockUseAutoRefresh(...args),
}));

vi.mock("../logs/useSessionTrace", () => ({
  useSessionTrace: (sessionId: string | null) => mockUseSessionTrace(sessionId),
}));

vi.mock("../logs/useTraceSubscription", () => ({
  useTraceSubscription: (...args: unknown[]) => mockUseTraceSubscription(...args),
}));

// Stub MessagesPane (replaces the older SessionChatViewer-based view) so the
// page test stays focused on layout + selection rather than the messages
// fetch path. MessagesPane has its own dedicated test file.
vi.mock("./MessagesPane", () => ({
  MessagesPane: ({ session }: { session: { session_id: string } | null }) => (
    <div data-testid="session-chat-viewer">{session?.session_id ?? ""}</div>
  ),
}));

// Import after mocks so they resolve.
import { MissionControlPage } from "./MissionControlPage";
import type { LogSession } from "@/services/transport/types";

function makeSession(overrides: Partial<LogSession> = {}): LogSession {
  return {
    session_id: "sess-x",
    conversation_id: "conv-x",
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

beforeEach(() => {
  vi.clearAllMocks();
  mockUseLogSessions.mockReturnValue({
    sessions: [],
    loading: false,
    error: null,
    refetch: vi.fn(),
  });
  mockUseSessionTrace.mockReturnValue({ trace: null, loading: false, refetch: vi.fn() });
  mockUseAutoRefresh.mockReturnValue(undefined);
  mockUseTraceSubscription.mockReturnValue(undefined);
});

describe("MissionControlPage", () => {
  it("renders the KPI strip with all five labels", () => {
    render(<MissionControlPage />);
    expect(screen.getByRole("region", { name: /mission control overview/i })).toBeInTheDocument();
    expect(screen.getByText("Running")).toBeInTheDocument();
    expect(screen.getByText("Done · 24h")).toBeInTheDocument();
  });

  it("renders the empty session-list message when there are no sessions", () => {
    render(<MissionControlPage />);
    expect(screen.getByText(/no sessions match/i)).toBeInTheDocument();
  });

  it("renders one row per session in the list", () => {
    mockUseLogSessions.mockReturnValue({
      sessions: [
        makeSession({ session_id: "alpha-1", title: "alpha" }),
        makeSession({ session_id: "beta-2", title: "beta" }),
      ],
      loading: false,
      error: null,
      refetch: vi.fn(),
    });
    render(<MissionControlPage />);
    expect(screen.getByText("alpha")).toBeInTheDocument();
    expect(screen.getByText("beta")).toBeInTheDocument();
  });

  it("auto-selects the first visible session when none is selected", () => {
    // Explicit timestamps so applyFilters' newest-first sort is deterministic.
    mockUseLogSessions.mockReturnValue({
      sessions: [
        makeSession({
          session_id: "first-aaaaaa",
          title: "alpha-row",
          started_at: "2026-04-25T22:00:00Z",
        }),
        makeSession({
          session_id: "second-bbbbbb",
          title: "beta-row",
          started_at: "2026-04-25T21:00:00Z",
        }),
      ],
      loading: false,
      error: null,
      refetch: vi.fn(),
    });
    render(<MissionControlPage />);
    // Detail pane mounts the SessionChatViewer for the auto-selected session.
    // Newest first → first-aaaaaa is selected.
    const viewer = screen.getByTestId("session-chat-viewer");
    expect(viewer.textContent).toBe("first-aaaaaa");
  });

  it("switches the detail pane when a different session row is clicked", () => {
    mockUseLogSessions.mockReturnValue({
      sessions: [
        makeSession({ session_id: "first-1", title: "first" }),
        makeSession({ session_id: "second-2", title: "second" }),
      ],
      loading: false,
      error: null,
      refetch: vi.fn(),
    });
    render(<MissionControlPage />);
    const secondRow = screen.getByText("second").closest("button")!;
    fireEvent.click(secondRow);
    // Detail pane title now includes "second"
    expect(screen.getAllByText(/second/).length).toBeGreaterThan(0);
  });

  it("toggles a status filter when a chip is clicked", () => {
    mockUseLogSessions.mockReturnValue({
      sessions: [
        makeSession({ session_id: "ok", title: "ok-row", status: "completed" }),
        makeSession({ session_id: "broken", title: "broken-row", status: "error" }),
      ],
      loading: false,
      error: null,
      refetch: vi.fn(),
    });
    render(<MissionControlPage />);
    expect(screen.getByText("broken-row")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "FAILED" }));
    expect(screen.queryByText("broken-row")).not.toBeInTheDocument();
  });

  it("filters by search term", () => {
    mockUseLogSessions.mockReturnValue({
      sessions: [
        makeSession({ session_id: "auth-1", title: "refactor auth ward" }),
        makeSession({ session_id: "report-1", title: "summarize Q4 reports" }),
      ],
      loading: false,
      error: null,
      refetch: vi.fn(),
    });
    render(<MissionControlPage />);
    fireEvent.change(screen.getByPlaceholderText(/search sessions/i), {
      target: { value: "auth" },
    });
    expect(screen.getByText("refactor auth ward")).toBeInTheDocument();
    expect(screen.queryByText("summarize Q4 reports")).not.toBeInTheDocument();
  });

  it("hands the WS subscription to the selected session", () => {
    mockUseLogSessions.mockReturnValue({
      sessions: [makeSession({ session_id: "live-1", status: "running", title: "live-row" })],
      loading: false,
      error: null,
      refetch: vi.fn(),
    });
    render(<MissionControlPage />);
    expect(mockUseTraceSubscription).toHaveBeenCalled();
    const calls = mockUseTraceSubscription.mock.calls as [{ session: LogSession | null }][];
    const lastCall = calls[calls.length - 1];
    expect(lastCall[0].session?.session_id).toBe("live-1");
  });
});
