// ============================================================================
// IntentInfoButton — render and interaction tests
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import type { Transport } from "@/services/transport";

const getSessionState = vi.fn<Transport["getSessionState"]>();

vi.mock("@/services/transport", () => ({
  getTransport: async () => ({ getSessionState }),
}));

import { IntentInfoButton } from "./IntentInfoButton";

describe("IntentInfoButton", () => {
  beforeEach(() => {
    getSessionState.mockReset();
  });

  it("renders the info button", () => {
    render(<IntentInfoButton sessionId="sess-1" />);
    expect(screen.getByRole("button", { name: /show intent analysis/i })).toBeInTheDocument();
  });

  it("shows popover when button is clicked", async () => {
    getSessionState.mockResolvedValue({
      success: true,
      data: { intentAnalysis: { primary_intent: "coding" } } as never,
    });
    render(<IntentInfoButton sessionId="sess-1" />);
    fireEvent.click(screen.getByRole("button", { name: /show intent analysis/i }));
    await waitFor(() => {
      expect(screen.getByText("Intent analysis")).toBeInTheDocument();
    });
  });

  it("shows loading state while fetching", async () => {
    getSessionState.mockReturnValue(new Promise(() => { /* never resolves */ }));
    render(<IntentInfoButton sessionId="sess-1" />);
    fireEvent.click(screen.getByRole("button", { name: /show intent analysis/i }));
    await waitFor(() => {
      expect(screen.getByText("loading…")).toBeInTheDocument();
    });
  });

  it("shows error when transport fails", async () => {
    getSessionState.mockResolvedValue({ success: false, error: "not found" });
    render(<IntentInfoButton sessionId="sess-1" />);
    fireEvent.click(screen.getByRole("button", { name: /show intent analysis/i }));
    await waitFor(() => {
      expect(screen.getByText("not found")).toBeInTheDocument();
    });
  });

  it("shows 'no intent analysis recorded' when data is null", async () => {
    getSessionState.mockResolvedValue({
      success: true,
      data: { intentAnalysis: null } as never,
    });
    render(<IntentInfoButton sessionId="sess-1" />);
    fireEvent.click(screen.getByRole("button", { name: /show intent analysis/i }));
    await waitFor(() => {
      expect(screen.getByText(/no intent analysis recorded/i)).toBeInTheDocument();
    });
  });

  it("renders primary intent when present", async () => {
    getSessionState.mockResolvedValue({
      success: true,
      data: {
        intentAnalysis: {
          primary_intent: "research task",
          recommended_skills: ["web_search", "summarize"],
          hidden_intents: ["fact_check"],
        },
      } as never,
    });
    render(<IntentInfoButton sessionId="sess-1" />);
    fireEvent.click(screen.getByRole("button", { name: /show intent analysis/i }));
    await waitFor(() => {
      expect(screen.getByText("research task")).toBeInTheDocument();
      expect(screen.getByText("web_search")).toBeInTheDocument();
      expect(screen.getByText("fact_check")).toBeInTheDocument();
    });
  });

  it("renders ward info when present", async () => {
    getSessionState.mockResolvedValue({
      success: true,
      data: {
        intentAnalysis: {
          ward_recommendation: {
            ward_name: "research-ward",
            action: "continue",
            subdirectory: "2024/q1",
            reason: "existing context",
          },
        },
      } as never,
    });
    render(<IntentInfoButton sessionId="sess-1" />);
    fireEvent.click(screen.getByRole("button", { name: /show intent analysis/i }));
    await waitFor(() => {
      expect(screen.getByText("research-ward")).toBeInTheDocument();
    });
  });

  it("renders execution strategy when present", async () => {
    getSessionState.mockResolvedValue({
      success: true,
      data: {
        intentAnalysis: {
          execution_strategy: {
            approach: "parallel",
            explanation: "multiple independent tasks",
          },
        },
      } as never,
    });
    render(<IntentInfoButton sessionId="sess-1" />);
    fireEvent.click(screen.getByRole("button", { name: /show intent analysis/i }));
    await waitFor(() => {
      expect(screen.getByText("parallel")).toBeInTheDocument();
      expect(screen.getByText(/multiple independent tasks/)).toBeInTheDocument();
    });
  });

  it("renders recommended agents when present", async () => {
    getSessionState.mockResolvedValue({
      success: true,
      data: {
        intentAnalysis: {
          recommended_agents: ["planner", "coder"],
        },
      } as never,
    });
    render(<IntentInfoButton sessionId="sess-1" />);
    fireEvent.click(screen.getByRole("button", { name: /show intent analysis/i }));
    await waitFor(() => {
      expect(screen.getByText("planner")).toBeInTheDocument();
      expect(screen.getByText("coder")).toBeInTheDocument();
    });
  });

  it("closes popover when close button is clicked", async () => {
    getSessionState.mockResolvedValue({
      success: true,
      data: { intentAnalysis: { primary_intent: "coding" } } as never,
    });
    render(<IntentInfoButton sessionId="sess-1" />);
    fireEvent.click(screen.getByRole("button", { name: /show intent analysis/i }));
    await waitFor(() => screen.getByText("Intent analysis"));

    fireEvent.click(screen.getByRole("button", { name: /close/i }));
    expect(screen.queryByText("Intent analysis")).toBeNull();
  });

  it("does not refetch when popover is re-opened (uses cached data)", async () => {
    getSessionState.mockResolvedValue({
      success: true,
      data: { intentAnalysis: { primary_intent: "test" } } as never,
    });
    render(<IntentInfoButton sessionId="sess-1" />);

    // Open
    fireEvent.click(screen.getByRole("button", { name: /show intent analysis/i }));
    await waitFor(() => screen.getByText("test"));

    // Close
    fireEvent.click(screen.getByRole("button", { name: /close/i }));

    // Re-open
    fireEvent.click(screen.getByRole("button", { name: /show intent analysis/i }));
    await waitFor(() => screen.getByText("test"));

    // Should only have fetched once
    expect(getSessionState).toHaveBeenCalledTimes(1);
  });

  it("resets state when sessionId changes", async () => {
    getSessionState.mockResolvedValue({
      success: true,
      data: { intentAnalysis: { primary_intent: "first" } } as never,
    });
    const { rerender } = render(<IntentInfoButton sessionId="sess-1" />);
    fireEvent.click(screen.getByRole("button", { name: /show intent analysis/i }));
    await waitFor(() => screen.getByText("first"));

    // Change session — popover should close
    rerender(<IntentInfoButton sessionId="sess-2" />);
    expect(screen.queryByText("Intent analysis")).toBeNull();
  });

  it("handles transport exception gracefully", async () => {
    getSessionState.mockRejectedValue(new Error("boom"));
    render(<IntentInfoButton sessionId="sess-1" />);
    fireEvent.click(screen.getByRole("button", { name: /show intent analysis/i }));
    await waitFor(() => {
      expect(screen.getByText("boom")).toBeInTheDocument();
    });
  });

  it("closes popover on click outside", async () => {
    getSessionState.mockResolvedValue({
      success: true,
      data: { intentAnalysis: { primary_intent: "test" } } as never,
    });
    render(
      <div>
        <IntentInfoButton sessionId="sess-1" />
        <div data-testid="outside">outside</div>
      </div>,
    );
    fireEvent.click(screen.getByRole("button", { name: /show intent analysis/i }));
    await waitFor(() => screen.getByText("Intent analysis"));

    // Simulate click outside (on document)
    fireEvent.mouseDown(screen.getByTestId("outside"));
    expect(screen.queryByText("Intent analysis")).toBeNull();
  });
});
