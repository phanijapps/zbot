import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { MemoryTab } from "../MemoryTab";

vi.mock("@/services/transport", () => ({
  getTransport: async () => ({
    listWards: async () => ({
      success: true,
      data: [{ id: "literature-library", count: 5 }],
    }),
    getWardContent: async (id: string) => ({
      success: true,
      data: {
        ward_id: id,
        summary: { title: id, description: "curated library" },
        facts: [
          {
            id: "f1",
            content: "always check the graph first",
            category: "instruction",
            confidence: 1,
            created_at: new Date().toISOString(),
            age_bucket: "today",
          },
        ],
        wiki: [],
        procedures: [],
        episodes: [],
        counts: { facts: 1, wiki: 0, procedures: 0, episodes: 0 },
      },
    }),
    searchMemoryHybrid: async () => ({
      success: true,
      data: {
        facts: { hits: [], latency_ms: 0 },
        wiki: { hits: [], latency_ms: 0 },
        procedures: { hits: [], latency_ms: 0 },
        episodes: { hits: [], latency_ms: 0 },
      },
    }),
    createMemory: vi.fn(async () => ({ success: true })),
  }),
}));

describe("MemoryTab (command-deck)", () => {
  it("renders wards, selects the first, shows its facts", async () => {
    render(<MemoryTab agentId="agent:root" />);
    const wardButton = await screen.findByRole("button", {
      name: /literature-library/,
    });
    expect(wardButton).toBeInTheDocument();
    fireEvent.click(wardButton);
    await waitFor(() =>
      expect(screen.getByText(/check the graph first/)).toBeInTheDocument(),
    );
  });
});
