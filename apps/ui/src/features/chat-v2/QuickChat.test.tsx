import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { QuickChat } from "./QuickChat";

// Stub the useQuickChat hook so render doesn't require a live transport.
vi.mock("./useQuickChat", () => ({
  useQuickChat: () => ({
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
  }),
}));

describe("<QuickChat>", () => {
  it("renders empty state with ward binding", () => {
    render(
      <MemoryRouter initialEntries={["/chat-v2"]}>
        <Routes>
          <Route path="/chat-v2" element={<QuickChat />} />
        </Routes>
      </MemoryRouter>
    );
    expect(screen.getByText("Quick chat")).toBeTruthy();
    expect(screen.getByText(/bound to stock-analysis/)).toBeTruthy();
  });

  it("shows New chat button", () => {
    render(
      <MemoryRouter initialEntries={["/chat-v2"]}>
        <Routes>
          <Route path="/chat-v2" element={<QuickChat />} />
        </Routes>
      </MemoryRouter>
    );
    expect(screen.getByText(/New chat/)).toBeTruthy();
  });

  it("shows ward chip with active ward name", () => {
    render(
      <MemoryRouter initialEntries={["/chat-v2"]}>
        <Routes>
          <Route path="/chat-v2" element={<QuickChat />} />
        </Routes>
      </MemoryRouter>
    );
    expect(screen.getAllByText("stock-analysis").length).toBeGreaterThan(0);
  });
});
