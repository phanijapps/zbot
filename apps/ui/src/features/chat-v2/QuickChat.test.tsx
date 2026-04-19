import { describe, it, expect, vi, beforeAll, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { QuickChat } from "./QuickChat";
import type { QuickChatState } from "./types";
import type { PillState } from "../shared/statusPill";

// A configurable hook mock so tests can drive different UI states.
interface MockHookReturn {
  state: QuickChatState;
  pillState: PillState;
  sendMessage: ReturnType<typeof vi.fn>;
  stopAgent: ReturnType<typeof vi.fn>;
}

const mockHookRef: { current: MockHookReturn } = {
  current: makeIdleHook(),
};

function makeIdleHook(): MockHookReturn {
  return {
    state: {
      sessionId: "sess-chat-xyz",
      conversationId: "chat-xyz",
      messages: [],
      status: "idle",
      activeWardName: "stock-analysis",
    },
    pillState: { visible: false, narration: "", suffix: "", category: "neutral", starting: false, swapCounter: 0 },
    sendMessage: vi.fn(),
    stopAgent: vi.fn(),
  };
}

vi.mock("./useQuickChat", () => ({
  useQuickChat: () => mockHookRef.current,
}));

// jsdom lacks scrollIntoView; polyfill as a no-op so the auto-scroll effect
// doesn't throw during render.
beforeAll(() => {
  if (!Element.prototype.scrollIntoView) {
    Element.prototype.scrollIntoView = () => { /* no-op */ };
  }
});

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

  it("renders the empty state with ward binding", () => {
    renderPage();
    expect(screen.getByText("Quick chat")).toBeTruthy();
    expect(screen.getByText(/bound to stock-analysis/)).toBeTruthy();
  });

  it("shows the ward chip when a ward is active", () => {
    renderPage();
    expect(screen.getAllByText("stock-analysis").length).toBeGreaterThan(0);
  });

  it("does NOT render a ward chip when no ward is active", () => {
    mockHookRef.current = {
      ...makeIdleHook(),
      state: { ...makeIdleHook().state, activeWardName: null },
    };
    const { container } = renderPage();
    expect(container.querySelector(".quick-chat__ward-chip")).toBeNull();
  });

  it("does NOT render a New chat button", () => {
    renderPage();
    expect(screen.queryByText(/New chat/i)).toBeNull();
    expect(screen.queryByTitle(/New chat/i)).toBeNull();
  });

  it("renders conversation messages + inline chips when history is present", () => {
    mockHookRef.current = {
      ...makeIdleHook(),
      state: {
        ...makeIdleHook().state,
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

  it("shows a Stop button while running and fires stopAgent on click", () => {
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
});
