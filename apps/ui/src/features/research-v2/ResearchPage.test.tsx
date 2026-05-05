import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { ResearchPage } from "./ResearchPage";
import type { ResearchSessionState } from "./types";
import type { PillState } from "../shared/statusPill";

// Mock the transport so ward-chip open clicks don't fire real HTTP.
// Use vi.hoisted so mocks are constructed before vi.mock factories run.
type OpenWardResult =
  | { success: true; data: { path: string } }
  | { success: false; error: string };

type DeleteSessionResult =
  | { success: true; data?: void }
  | { success: false; error: string };

const { openWardMock, toastErrorMock, listArtifactsMock, deleteSessionMock, listLogSessionsMock } = vi.hoisted(() => ({
  openWardMock: vi.fn<(wardId: string) => Promise<OpenWardResult>>(),
  toastErrorMock: vi.fn(),
  listArtifactsMock: vi.fn(),
  deleteSessionMock: vi.fn<(sessionId: string) => Promise<DeleteSessionResult>>(),
  listLogSessionsMock: vi.fn(),
}));

vi.mock("@/services/transport", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("@/services/transport");
  return {
    ...actual,
    getTransport: async () => ({
      openWard: openWardMock,
      listSessionArtifacts: listArtifactsMock,
      getArtifactContentUrl: () => "about:blank",
      deleteSession: deleteSessionMock,
      listLogSessions: listLogSessionsMock,
    }),
  };
});

vi.mock("sonner", () => ({
  toast: { error: toastErrorMock },
}));

interface MockResearchHook {
  state: ResearchSessionState;
  pillState: PillState;
  sendMessage: ReturnType<typeof vi.fn>;
  stopAgent: ReturnType<typeof vi.fn>;
  startNewResearch: ReturnType<typeof vi.fn>;
  toggleThinking: ReturnType<typeof vi.fn>;
  getFullArtifact: ReturnType<typeof vi.fn>;
}

interface MockListHook {
  sessions: Array<{ id: string; title: string; status: "running" | "complete" | "crashed" | "paused"; wardName: string | null; updatedAt: number }>;
  loading: boolean;
  refresh: ReturnType<typeof vi.fn>;
  deleteSession: ReturnType<typeof vi.fn>;
}

interface UseSessionsListOptionsMock {
  onAfterDelete?: (id: string) => void;
}

const researchRef: { current: MockResearchHook } = { current: makeIdleResearch() };
const listRef: { current: MockListHook } = {
  current: { sessions: [], loading: false, refresh: vi.fn(), deleteSession: vi.fn() },
};
// Captured so tests can simulate "server delete completed, fire onAfterDelete"
// without coupling to the real useSessionsList implementation.
const lastListOptsRef: { current: UseSessionsListOptionsMock } = { current: {} };

function makeIdleResearch(): MockResearchHook {
  const state: ResearchSessionState = {
    sessionId: null,
    conversationId: null,
    title: "",
    status: "idle",
    wardId: null,
    wardName: null,
    rootExecutionId: null,
    turns: [],
    intentAnalyzing: false,
    intentClassification: null,
    planPath: null,
    artifacts: [],
  };
  const pillState: PillState = {
    visible: false,
    narration: "",
    suffix: "",
    category: "neutral",
    starting: false,
    swapCounter: 0,
  };
  return {
    state,
    pillState,
    sendMessage: vi.fn(),
    stopAgent: vi.fn(),
    startNewResearch: vi.fn(),
    toggleThinking: vi.fn(),
    getFullArtifact: vi.fn(),
  };
}

vi.mock("./useResearchSession", () => ({
  useResearchSession: () => researchRef.current,
}));

vi.mock("./useSessionsList", () => ({
  useSessionsList: (opts?: UseSessionsListOptionsMock) => {
    lastListOptsRef.current = opts ?? {};
    return listRef.current;
  },
}));

function renderPage() {
  return render(
    <MemoryRouter initialEntries={["/research-v2"]}>
      <Routes>
        <Route path="/research-v2" element={<ResearchPage />} />
        <Route path="/research-v2/:id" element={<ResearchPage />} />
      </Routes>
    </MemoryRouter>
  );
}

describe("<ResearchPage>", () => {
  beforeEach(() => {
    researchRef.current = makeIdleResearch();
    listRef.current = { sessions: [], loading: false, refresh: vi.fn(), deleteSession: vi.fn() };
    lastListOptsRef.current = {};
    openWardMock.mockClear();
    openWardMock.mockResolvedValue({ success: true, data: { path: "/vault/wards/x" } });
    toastErrorMock.mockClear();
    listArtifactsMock.mockClear();
    listArtifactsMock.mockResolvedValue({ success: true, data: [] });
    deleteSessionMock.mockClear();
    deleteSessionMock.mockResolvedValue({ success: true });
    listLogSessionsMock.mockClear();
    listLogSessionsMock.mockResolvedValue({ success: true, data: [] });
    // window.confirm is a browser-level primitive — force-accept so the
    // onAfterDelete code path is exercised without hitting jsdom's "confirm
    // is not implemented" stub.
    window.confirm = vi.fn(() => true);
  });

  it("renders the HeroInput landing when session has no content", () => {
    renderPage();
    // HeroInput surfaces the z-Bot brand + the prompt placeholder.
    expect(screen.getByText("z-Bot")).toBeTruthy();
    expect(screen.getByPlaceholderText(/What would you like to work on/i)).toBeTruthy();
  });

  it("has a drawer-toggle button that is accessible by label", () => {
    renderPage();
    expect(screen.getByLabelText("Open sessions")).toBeTruthy();
  });

  it("toggles the drawer open when ☰ is clicked", () => {
    renderPage();
    fireEvent.click(screen.getByLabelText("Open sessions"));
    expect(screen.getByLabelText("Research sessions")).toBeTruthy();
  });

  it("renders the ward chip ONLY when activeWardName is set", () => {
    renderPage();
    // no ward yet → chip absent
    expect(screen.queryByText("stock-analysis")).toBeNull();

    researchRef.current = {
      ...makeIdleResearch(),
      state: {
        ...makeIdleResearch().state,
        wardId: "stock-analysis",
        wardName: "stock-analysis",
      },
    };
    const { container } = renderPage();
    expect(container.textContent).toContain("stock-analysis");
  });

  it("ward chip renders as a button when wardName + wardId are set", () => {
    researchRef.current = {
      ...makeIdleResearch(),
      state: {
        ...makeIdleResearch().state,
        wardId: "stock-analysis",
        wardName: "stock-analysis",
      },
    };
    renderPage();
    const btn = screen.getByRole("button", { name: /open ward folder/i });
    expect(btn).toBeTruthy();
    expect(btn.tagName).toBe("BUTTON");
  });

  it("ward chip is NOT rendered when wardName is null", () => {
    renderPage();
    expect(screen.queryByRole("button", { name: /open ward folder/i })).toBeNull();
  });

  it("clicking the ward chip calls transport.openWard with the ward id", async () => {
    researchRef.current = {
      ...makeIdleResearch(),
      state: {
        ...makeIdleResearch().state,
        wardId: "stock-analysis",
        wardName: "stock-analysis",
      },
    };
    renderPage();
    fireEvent.click(screen.getByRole("button", { name: /open ward folder/i }));
    await waitFor(() => {
      expect(openWardMock).toHaveBeenCalledWith("stock-analysis");
    });
    expect(toastErrorMock).not.toHaveBeenCalled();
  });

  it("shows a toast when transport.openWard fails", async () => {
    openWardMock.mockResolvedValueOnce({ success: false, error: "boom" });
    researchRef.current = {
      ...makeIdleResearch(),
      state: {
        ...makeIdleResearch().state,
        wardId: "stock-analysis",
        wardName: "stock-analysis",
      },
    };
    renderPage();
    fireEvent.click(screen.getByRole("button", { name: /open ward folder/i }));
    await waitFor(() => {
      expect(toastErrorMock).toHaveBeenCalled();
    });
    const msg = String(toastErrorMock.mock.calls[0][0]);
    expect(msg).toContain("boom");
  });

  it("shows Stop button only while running and fires stopAgent on click", () => {
    // Idle: no Stop button
    renderPage();
    expect(screen.queryByTitle("Stop")).toBeNull();

    const stopSpy = vi.fn();
    researchRef.current = {
      ...makeIdleResearch(),
      state: { ...makeIdleResearch().state, status: "running" },
      stopAgent: stopSpy,
    };
    renderPage();
    const btn = screen.getByTitle("Stop");
    fireEvent.click(btn);
    expect(stopSpy).toHaveBeenCalled();
  });

  it("New research button fires startNewResearch", () => {
    const newSpy = vi.fn();
    researchRef.current = {
      ...makeIdleResearch(),
      startNewResearch: newSpy,
      state: { ...makeIdleResearch().state, sessionId: "sess-1" },
    };
    renderPage();
    // /New research/ also appears as the title placeholder — scope to the button.
    fireEvent.click(screen.getByRole("button", { name: /New research/ }));
    expect(newSpy).toHaveBeenCalled();
  });

  it("renders a user-query bubble when state.turns has an open turn", () => {
    researchRef.current = {
      ...makeIdleResearch(),
      state: {
        ...makeIdleResearch().state,
        sessionId: "sess-1",
        turns: [
          {
            id: "turn-m1",
            index: 0,
            userMessage: {
              id: "m1",
              content: "what's the Q4 outlook?",
              createdAt: "2026-04-19T00:00:00.000Z",
            },
            subagents: [],
            assistantText: null,
            assistantStreaming: "",
            timeline: [],
            status: "running",
            startedAt: "2026-04-19T00:00:00.000Z",
            endedAt: null,
            durationMs: null,
          },
        ],
      },
    };
    renderPage();
    // Bubble and derived title both carry the user message — 2 matches expected.
    expect(screen.getAllByText(/Q4 outlook/).length).toBeGreaterThanOrEqual(1);
  });

  it("renders the status pill when pillState.visible is true", () => {
    researchRef.current = {
      ...makeIdleResearch(),
      state: { ...makeIdleResearch().state, sessionId: "sess-1" },
      pillState: {
        visible: true,
        narration: "Running shell",
        suffix: "ls -la ~",
        category: "read",
        starting: false,
        swapCounter: 1,
      },
    };
    renderPage();
    expect(screen.getByTestId("status-pill")).toBeTruthy();
    expect(screen.getByText("Running shell")).toBeTruthy();
  });

  it("shows intent analysing muted line when intentAnalyzing=true", () => {
    researchRef.current = {
      ...makeIdleResearch(),
      state: {
        ...makeIdleResearch().state,
        sessionId: "sess-1",
        intentAnalyzing: true,
      },
    };
    renderPage();
    expect(screen.getByText(/analyzing intent/i)).toBeTruthy();
  });

  it("renders nested subagent cards under their parent (root → child → grandchild)", () => {
    const now = 1000;
    const child = {
      id: "child-1",
      agentId: "planner-agent",
      parentExecutionId: "root-1",
      startedAt: now + 200,
      completedAt: now + 300,
      status: "completed" as const,
      wardId: "w1",
      request: "Plan this goal.",
      timeline: [],
      tokenCount: 10,
      respond: "child body",
      respondStreaming: "",
      thinkingExpanded: false,
      errorMessage: null,
    };
    const grandchild = {
      ...child,
      id: "grand-1",
      agentId: "builder-agent",
      parentExecutionId: "child-1",
      startedAt: now + 400,
      respond: "grand body",
      request: "Run builder step.",
    };
    researchRef.current = {
      ...makeIdleResearch(),
      state: {
        ...makeIdleResearch().state,
        sessionId: "sess-1",
        rootExecutionId: "root-1",
        turns: [
          {
            id: "turn-u1",
            index: 0,
            userMessage: {
              id: "u1",
              content: "do the thing",
              createdAt: "2026-04-19T00:00:00.000Z",
            },
            subagents: [child, grandchild],
            assistantText: "root body",
            assistantStreaming: "",
            timeline: [],
            status: "completed",
            startedAt: "2026-04-19T00:00:00.000Z",
            endedAt: "2026-04-19T00:01:00.000Z",
            durationMs: 60_000,
          },
        ],
      },
    };
    const { container } = renderPage();

    // Root assistant area renders inside the SessionTurnBlock.
    expect(container.querySelector(".research-msg--assistant")).toBeTruthy();

    // Both subagent cards present, and grand-child is nested under child.
    const planner = container.querySelector('.subagent-card[data-parent="root-1"]');
    expect(planner).toBeTruthy();
    const builder = container.querySelector('.subagent-card[data-parent="child-1"]');
    expect(builder).toBeTruthy();

    // Root's reply is always visible on the SessionTurn's assistant block.
    expect(container.textContent).toContain("root body");
    // Subagent cards are auto-collapsed on completion; expand them and
    // verify their responses then become visible.
    for (const btn of Array.from(container.querySelectorAll("button.subagent-card__toggle"))) {
      fireEvent.click(btn);
    }
    expect(container.textContent).toContain("child body");
    expect(container.textContent).toContain("grand body");
  });

  it("shows intent classification line when set", () => {
    researchRef.current = {
      ...makeIdleResearch(),
      state: {
        ...makeIdleResearch().state,
        sessionId: "sess-1",
        intentClassification: "research",
        wardName: "maritime",
      },
    };
    renderPage();
    expect(screen.getByText(/intent:/)).toBeTruthy();
    expect(screen.getByText("research")).toBeTruthy();
  });

  // ------- R14d: artifact strip + slide-out wiring -------

  it("does not render the artifact strip when state.artifacts is empty", () => {
    renderPage();
    expect(screen.queryByRole("list", { name: /session artifacts/i })).toBeNull();
  });

  it("renders a chip per artifact when state.artifacts has entries", () => {
    researchRef.current = {
      ...makeIdleResearch(),
      state: {
        ...makeIdleResearch().state,
        sessionId: "sess-1",
        artifacts: [
          { id: "a1", fileName: "plan.md", fileType: "md" },
          { id: "a2", fileName: "data.csv", fileType: "csv" },
        ],
      },
    };
    renderPage();
    expect(screen.getByRole("list", { name: /session artifacts/i })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Open artifact plan\.md/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Open artifact data\.csv/ })).toBeTruthy();
  });

  it("clicking a chip opens the slide-out for the matching artifact (cached path)", async () => {
    const cached = {
      id: "a1",
      sessionId: "sess-1",
      filePath: "/tmp/plan.md",
      fileName: "plan.md",
      fileType: "md",
      fileSize: 100,
      createdAt: "2026-04-19T00:00:00Z",
    };
    researchRef.current = {
      ...makeIdleResearch(),
      state: {
        ...makeIdleResearch().state,
        sessionId: "sess-1",
        artifacts: [{ id: "a1", fileName: "plan.md", fileType: "md" }],
      },
      getFullArtifact: vi.fn().mockReturnValue(cached),
    };
    renderPage();
    fireEvent.click(screen.getByRole("button", { name: /Open artifact plan\.md/ }));
    await waitFor(() => {
      expect(researchRef.current.getFullArtifact).toHaveBeenCalledWith("a1");
    });
    // Slide-out header shows the filename.
    await waitFor(() => {
      expect(screen.getAllByText(/plan\.md/).length).toBeGreaterThan(0);
    });
  });

  it("falls back to listSessionArtifacts when the cache miss returns undefined", async () => {
    const remote = {
      id: "a1",
      sessionId: "sess-1",
      filePath: "/tmp/plan.md",
      fileName: "plan.md",
      fileType: "md",
      fileSize: 100,
      createdAt: "2026-04-19T00:00:00Z",
    };
    listArtifactsMock.mockResolvedValueOnce({ success: true, data: [remote] });
    researchRef.current = {
      ...makeIdleResearch(),
      state: {
        ...makeIdleResearch().state,
        sessionId: "sess-1",
        artifacts: [{ id: "a1", fileName: "plan.md", fileType: "md" }],
      },
      getFullArtifact: vi.fn().mockReturnValue(undefined),
    };
    renderPage();
    fireEvent.click(screen.getByRole("button", { name: /Open artifact plan\.md/ }));
    await waitFor(() => {
      expect(listArtifactsMock).toHaveBeenCalledWith("sess-1");
    });
  });

  // ---------------------------------------------------------------------------
  // R19 — onAfterDelete → startNewResearch wiring
  // ---------------------------------------------------------------------------

  it("onAfterDelete fires startNewResearch when the current session is the one deleted", () => {
    const startNewResearch = vi.fn();
    researchRef.current = {
      ...makeIdleResearch(),
      state: { ...makeIdleResearch().state, sessionId: "sess-1" },
      startNewResearch,
    };
    renderPage();
    // ResearchPage handed the hook an onAfterDelete — simulate the hook calling
    // it (what a real transport.deleteSession + refresh() would do).
    lastListOptsRef.current.onAfterDelete?.("sess-1");
    expect(startNewResearch).toHaveBeenCalledTimes(1);
  });

  it("onAfterDelete does NOT fire startNewResearch for a different session id", () => {
    const startNewResearch = vi.fn();
    researchRef.current = {
      ...makeIdleResearch(),
      state: { ...makeIdleResearch().state, sessionId: "sess-1" },
      startNewResearch,
    };
    renderPage();
    lastListOptsRef.current.onAfterDelete?.("sess-999");
    expect(startNewResearch).not.toHaveBeenCalled();
  });

  // ---------------------------------------------------------------------------
  // Regression: chat-mode sessions must not leak into the research landing
  // hero's "Recent" cards. Before the session-kind fix, chat-v2 turns
  // rendered as suggestion chips under the research composer.
  // ---------------------------------------------------------------------------

  it("excludes chat-mode sessions from the research empty-state recent cards", async () => {
    listLogSessionsMock.mockResolvedValue({
      success: true,
      data: [
        {
          session_id: "exec-chat-1",
          conversation_id: "sess-chat-abc",
          agent_id: "root",
          agent_name: "root",
          title: "hi there",
          started_at: "2026-04-24T08:00:00Z",
          status: "completed",
          token_count: 100,
          tool_call_count: 0,
          error_count: 0,
          child_session_ids: [],
          mode: "fast",
        },
        {
          session_id: "exec-research-1",
          conversation_id: "sess-research-xyz",
          agent_id: "root",
          agent_name: "root",
          title: "Analyze Q4 market data",
          started_at: "2026-04-24T09:00:00Z",
          status: "completed",
          token_count: 4200,
          tool_call_count: 12,
          error_count: 0,
          child_session_ids: [],
          mode: null,
        },
      ],
    });
    renderPage();
    // Research card visible.
    await waitFor(() => {
      expect(screen.getByText(/Analyze Q4 market data/)).toBeTruthy();
    });
    // Chat card's title ("hi there") must not appear as a recent card.
    expect(screen.queryByText(/^hi there$/)).toBeNull();
  });
});
