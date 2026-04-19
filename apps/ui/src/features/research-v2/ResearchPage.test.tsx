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

const { openWardMock, toastErrorMock, listArtifactsMock } = vi.hoisted(() => ({
  openWardMock: vi.fn<(wardId: string) => Promise<OpenWardResult>>(),
  toastErrorMock: vi.fn(),
  listArtifactsMock: vi.fn(),
}));

vi.mock("@/services/transport", () => ({
  getTransport: async () => ({
    openWard: openWardMock,
    listSessionArtifacts: listArtifactsMock,
    getArtifactContentUrl: () => "about:blank",
  }),
}));

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
}

const researchRef: { current: MockResearchHook } = { current: makeIdleResearch() };
const listRef: { current: MockListHook } = { current: { sessions: [], loading: false, refresh: vi.fn() } };

function makeIdleResearch(): MockResearchHook {
  const state: ResearchSessionState = {
    sessionId: null,
    conversationId: null,
    title: "",
    status: "idle",
    wardId: null,
    wardName: null,
    messages: [],
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
  useSessionsList: () => listRef.current,
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
    listRef.current = { sessions: [], loading: false, refresh: vi.fn() };
    openWardMock.mockClear();
    openWardMock.mockResolvedValue({ success: true, data: { path: "/vault/wards/x" } });
    toastErrorMock.mockClear();
    listArtifactsMock.mockClear();
    listArtifactsMock.mockResolvedValue({ success: true, data: [] });
  });

  it("renders the empty state when session has no content", () => {
    renderPage();
    expect(screen.getByText("Research")).toBeTruthy();
    expect(screen.getByText(/full agent chain/)).toBeTruthy();
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
    researchRef.current = { ...makeIdleResearch(), startNewResearch: newSpy };
    renderPage();
    // /New research/ also appears as the title placeholder — scope to the button.
    fireEvent.click(screen.getByRole("button", { name: /New research/ }));
    expect(newSpy).toHaveBeenCalled();
  });

  it("renders a user-query bubble when state.messages has entries", () => {
    researchRef.current = {
      ...makeIdleResearch(),
      state: {
        ...makeIdleResearch().state,
        sessionId: "sess-1",
        messages: [{ id: "m1", role: "user", content: "what's the Q4 outlook?", timestamp: 1 }],
      },
    };
    renderPage();
    // Bubble and derived title both carry the user message — 2 matches expected.
    expect(screen.getAllByText(/Q4 outlook/).length).toBeGreaterThanOrEqual(1);
  });

  it("renders the status pill when pillState.visible is true", () => {
    researchRef.current = {
      ...makeIdleResearch(),
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

  it("renders nested subagent cards under their parent (depth 0 → 1 → 2)", () => {
    const now = 1000;
    const root = {
      id: "root-1",
      agentId: "root",
      parentExecutionId: null,
      startedAt: now,
      completedAt: now + 100,
      status: "completed" as const,
      wardId: "w1",
      timeline: [],
      tokenCount: 10,
      respond: "root body",
      respondStreaming: "",
      thinkingExpanded: false,
      errorMessage: null, request: null,
    };
    const child = {
      ...root,
      id: "child-1",
      agentId: "planner-agent",
      parentExecutionId: "root-1",
      startedAt: now + 200,
      respond: "child body",
      request: "Plan this goal.",
    };
    const grandchild = {
      ...root,
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
        turns: [root, child, grandchild],
      },
    };
    const { container } = renderPage();

    // Root renders as an assistant message (not a bordered turn block).
    expect(container.querySelector(".research-msg--assistant")).toBeTruthy();

    // Both subagent cards present, and grand-child is nested under child.
    const planner = container.querySelector('.subagent-card[data-parent="root-1"]');
    expect(planner).toBeTruthy();
    const builder = container.querySelector('.subagent-card[data-parent="child-1"]');
    expect(builder).toBeTruthy();

    // All three respond bodies are visible.
    expect(container.textContent).toContain("root body");
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
    expect(screen.getByRole("listitem", { name: /Open artifact plan\.md/ })).toBeTruthy();
    expect(screen.getByRole("listitem", { name: /Open artifact data\.csv/ })).toBeTruthy();
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
    fireEvent.click(screen.getByRole("listitem", { name: /Open artifact plan\.md/ }));
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
    fireEvent.click(screen.getByRole("listitem", { name: /Open artifact plan\.md/ }));
    await waitFor(() => {
      expect(listArtifactsMock).toHaveBeenCalledWith("sess-1");
    });
  });
});
