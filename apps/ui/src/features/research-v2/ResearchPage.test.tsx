import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { ResearchPage } from "./ResearchPage";
import type { ResearchSessionState } from "./types";
import type { PillState } from "../shared/statusPill";

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
      state: { ...makeIdleResearch().state, wardName: "stock-analysis" },
    };
    const { container } = renderPage();
    expect(container.textContent).toContain("stock-analysis");
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
