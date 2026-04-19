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

const { openWardMock, toastErrorMock } = vi.hoisted(() => ({
  openWardMock: vi.fn<(wardId: string) => Promise<OpenWardResult>>(),
  toastErrorMock: vi.fn(),
}));

vi.mock("@/services/transport", () => ({
  getTransport: async () => ({ openWard: openWardMock }),
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
    fireEvent.click(screen.getByText(/New research/));
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
    expect(screen.getByText(/Q4 outlook/)).toBeTruthy();
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

  it("renders nested subagent turns under their parent (depth 0 → 1 → 2)", () => {
    const now = 1000;
    const root = {
      id: "root-1",
      agentId: "planner",
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
      errorMessage: null,
    };
    const child = {
      ...root,
      id: "child-1",
      agentId: "solution",
      parentExecutionId: "root-1",
      startedAt: now + 200,
      respond: "child body",
    };
    const grandchild = {
      ...root,
      id: "grand-1",
      agentId: "builder",
      parentExecutionId: "child-1",
      startedAt: now + 400,
      respond: "grand body",
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

    // Root is rendered as a top-level block (data-parent empty).
    const rootBlock = container.querySelector('.agent-turn-block[data-parent=""]');
    expect(rootBlock).toBeTruthy();

    // The root block contains the nested children container.
    const nestedContainer = rootBlock?.querySelector(".agent-turn-block__children");
    expect(nestedContainer).toBeTruthy();

    // Inside the nested container, the child block is present…
    const childBlock = nestedContainer?.querySelector('[data-parent="root-1"]');
    expect(childBlock).toBeTruthy();

    // …and the child block contains ITS OWN nested container for the grandchild.
    const grandBlock = childBlock?.querySelector('[data-parent="child-1"]');
    expect(grandBlock).toBeTruthy();

    // All three agent labels present.
    const agentLabels = container.querySelectorAll(".agent-turn-block__agent");
    expect(agentLabels).toHaveLength(3);
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
});
