import { describe, it, expect, vi, beforeAll, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { QuickChat } from "./QuickChat";
import type { QuickChatState } from "./types";
import type { PillState } from "../shared/statusPill";

// jsdom doesn't implement scrollIntoView; polyfill as a no-op so auto-scroll
// effects don't throw during render.
beforeAll(() => {
  if (!Element.prototype.scrollIntoView) {
    Element.prototype.scrollIntoView = () => { /* no-op */ };
  }
});

// A configurable hook mock so tests can drive different UI states.
interface MockHookReturn {
  state: QuickChatState;
  pillState: PillState;
  sendMessage: ReturnType<typeof vi.fn>;
  startNewChat: ReturnType<typeof vi.fn>;
  stopAgent: ReturnType<typeof vi.fn>;
  loadOlder: ReturnType<typeof vi.fn>;
}

const mockHookRef: { current: MockHookReturn } = {
  current: makeIdleHook(),
};

function makeIdleHook(): MockHookReturn {
  return {
    state: {
      sessionId: null,
      conversationId: "c1",
      messages: [],
      status: "idle",
      activeWardName: "stock-analysis",
      olderCursor: null,
      hasMoreOlder: false,
    },
    pillState: { visible: false, narration: "", suffix: "", category: "neutral", starting: false, swapCounter: 0 },
    sendMessage: vi.fn(),
    startNewChat: vi.fn(),
    stopAgent: vi.fn(),
    loadOlder: vi.fn(),
  };
}

vi.mock("./useQuickChat", () => ({
  useQuickChat: () => mockHookRef.current,
}));

function renderPage() {
  return render(
    <MemoryRouter initialEntries={["/chat-v2"]}>
      <Routes>
        <Route path="/chat-v2" element={<QuickChat />} />
      </Routes>
    </MemoryRouter>
  );
}

describe("<QuickChat>", () => {
  beforeEach(() => {
    mockHookRef.current = makeIdleHook();
  });

  it("renders empty state with ward binding", () => {
    renderPage();
    expect(screen.getByText("Quick chat")).toBeTruthy();
    expect(screen.getByText(/bound to stock-analysis/)).toBeTruthy();
  });

  it("shows New chat button", () => {
    renderPage();
    expect(screen.getByText(/New chat/)).toBeTruthy();
  });

  it("shows ward chip with active ward name", () => {
    renderPage();
    expect(screen.getAllByText("stock-analysis").length).toBeGreaterThan(0);
  });

  it("shows 'no ward' chip when no ward is active", () => {
    mockHookRef.current = {
      ...makeIdleHook(),
      state: { ...makeIdleHook().state, activeWardName: null },
    };
    renderPage();
    expect(screen.getByText("no ward")).toBeTruthy();
  });

  it("renders conversation messages + inline chips when not empty", () => {
    mockHookRef.current = {
      ...makeIdleHook(),
      state: {
        ...makeIdleHook().state,
        sessionId: "sess-1",
        messages: [
          { id: "u1", role: "user", content: "what was z.ai rate limit?", timestamp: 1 },
          {
            id: "a1",
            role: "assistant",
            content: "Per-key semaphore size 3.",
            timestamp: 2,
            chips: [{ id: "c1", kind: "recall", label: "recalled 1" }],
          },
        ],
      },
    };
    renderPage();
    expect(screen.getByText(/z.ai rate limit/)).toBeTruthy();
    expect(screen.getByText(/Per-key semaphore size/)).toBeTruthy();
    expect(screen.getByText("recalled 1")).toBeTruthy();
  });

  it("shows Stop button while running and triggers stopAgent on click", () => {
    const stopSpy = vi.fn();
    mockHookRef.current = {
      ...makeIdleHook(),
      state: { ...makeIdleHook().state, status: "running" },
      stopAgent: stopSpy,
    };
    renderPage();
    const stopBtn = screen.getByTitle("Stop");
    fireEvent.click(stopBtn);
    expect(stopSpy).toHaveBeenCalled();
  });

  it("fires startNewChat when New chat is clicked", () => {
    const newChatSpy = vi.fn();
    mockHookRef.current = { ...makeIdleHook(), startNewChat: newChatSpy };
    renderPage();
    fireEvent.click(screen.getByTitle("New chat"));
    expect(newChatSpy).toHaveBeenCalled();
  });

  it("renders a visible status pill when pillState.visible is true", () => {
    mockHookRef.current = {
      ...makeIdleHook(),
      pillState: {
        visible: true,
        narration: "Recalling fundamentals",
        suffix: "· recall",
        category: "read",
        starting: false,
        swapCounter: 1,
      },
    };
    renderPage();
    expect(screen.getByTestId("status-pill")).toBeTruthy();
    expect(screen.getByText("Recalling fundamentals")).toBeTruthy();
  });

  it("renders 'Show earlier turns' when hasMoreOlder is true and fires loadOlder", () => {
    const loadOlderSpy = vi.fn();
    mockHookRef.current = {
      ...makeIdleHook(),
      state: {
        ...makeIdleHook().state,
        sessionId: "sess-1",
        messages: [{ id: "u1", role: "user", content: "hi", timestamp: 1 }],
        hasMoreOlder: true,
      },
      loadOlder: loadOlderSpy,
    };
    renderPage();
    const btn = screen.getByText(/Show earlier turns/);
    fireEvent.click(btn);
    expect(loadOlderSpy).toHaveBeenCalled();
  });
});
