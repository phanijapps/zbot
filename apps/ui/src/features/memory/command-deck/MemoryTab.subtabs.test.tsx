// ============================================================================
// MemoryTab — sub-tab toggle switches between Facts / Beliefs /
// Contradictions and Facts remains the default.
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@/test/utils";
import { MemoryTab } from "./MemoryTab";
import type { WardContent } from "@/services/transport/types";

const mockListWards = vi.fn();
const mockGetWardContent = vi.fn();
const mockHybridSearch = vi.fn();

vi.mock("@/services/transport", async () => {
  const actual = await vi.importActual<Record<string, unknown>>(
    "@/services/transport",
  );
  return {
    ...actual,
    getTransport: async () => ({
      listWards: mockListWards,
      getWardContent: mockGetWardContent,
      deleteMemory: vi.fn(),
      hybridSearch: mockHybridSearch,
      createMemory: vi.fn(),
    }),
  };
});

// Belief API stubs — we only care that the right component mounts.
const mockListBeliefs = vi.fn();
const mockListContradictions = vi.fn();
vi.mock("./beliefs/api", () => ({
  listBeliefs: (...args: unknown[]) => mockListBeliefs(...args),
  listContradictions: (...args: unknown[]) => mockListContradictions(...args),
  getBeliefDetail: vi.fn(),
  resolveContradiction: vi.fn(),
}));

function content(): WardContent {
  return {
    ward_id: "auth-system",
    summary: { description: "Test ward" } as WardContent["summary"],
    counts: { facts: 0, wiki: 0, procedures: 0, episodes: 0 },
    facts: [],
    wiki: [],
    procedures: [],
    episodes: [],
  } as unknown as WardContent;
}

beforeEach(() => {
  vi.clearAllMocks();
  mockListWards.mockResolvedValue({
    success: true,
    data: [{ id: "auth-system", count: 0 }],
  });
  mockGetWardContent.mockResolvedValue({ success: true, data: content() });
  mockHybridSearch.mockResolvedValue({
    success: true,
    data: {
      facts: { hits: [], latency_ms: 1 },
      wiki: { hits: [], latency_ms: 1 },
      procedures: { hits: [], latency_ms: 1 },
      episodes: { hits: [], latency_ms: 1 },
    },
  });
  mockListBeliefs.mockResolvedValue({ success: true, data: [] });
  mockListContradictions.mockResolvedValue({ success: true, data: [] });
});

describe("MemoryTab — sub-tabs", () => {
  it("renders the Facts sub-tab as active by default", async () => {
    render(<MemoryTab agentId="root" />);
    await waitFor(() =>
      expect(screen.getByRole("tab", { name: "Facts" })).toBeInTheDocument(),
    );
    expect(screen.getByRole("tab", { name: "Facts" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("renders Beliefs and Contradictions tabs", async () => {
    render(<MemoryTab agentId="root" />);
    await waitFor(() =>
      expect(screen.getByRole("tab", { name: "Beliefs" })).toBeInTheDocument(),
    );
    expect(
      screen.getByRole("tab", { name: "Contradictions" }),
    ).toBeInTheDocument();
  });

  it("calls the beliefs API when Beliefs sub-tab is selected", async () => {
    render(<MemoryTab agentId="root" />);
    await waitFor(() =>
      expect(screen.getByRole("tab", { name: "Beliefs" })).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByRole("tab", { name: "Beliefs" }));
    await waitFor(() => {
      expect(mockListBeliefs).toHaveBeenCalled();
    });
  });

  it("calls the contradictions API when Contradictions sub-tab is selected", async () => {
    render(<MemoryTab agentId="root" />);
    await waitFor(() =>
      expect(
        screen.getByRole("tab", { name: "Contradictions" }),
      ).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByRole("tab", { name: "Contradictions" }));
    await waitFor(() => {
      expect(mockListContradictions).toHaveBeenCalled();
    });
  });

  it("keeps the Facts view default — selected when first rendered", async () => {
    render(<MemoryTab agentId="root" />);
    // The Content tabs from ContentDeck render under Facts. We assert
    // that the Facts content area exists by looking for the Facts /
    // Wiki / Procedures / Episodes inner tabs.
    await waitFor(() =>
      expect(screen.getByRole("tab", { name: /Wiki/ })).toBeInTheDocument(),
    );
  });
});
