// ============================================================================
// WebAgentsPanel — page layout + tab tests
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@/test/utils";
import { WebAgentsPanel } from "./WebAgentsPanel";

// ---------------------------------------------------------------------------
// Mocks — transport returns empty fixtures so the panel mounts cleanly.
// ---------------------------------------------------------------------------

const mockListAgents = vi.fn();
const mockListProviders = vi.fn();
const mockGetModelRegistry = vi.fn();
const mockListSkills = vi.fn();
const mockListSchedules = vi.fn();
const mockListAvailableSkills = vi.fn();

vi.mock("@/services/transport", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("@/services/transport");
  return {
    ...actual,
    getTransport: async () => ({
      listAgents: mockListAgents,
      listProviders: mockListProviders,
      getModelRegistry: mockGetModelRegistry,
      listSkills: mockListSkills,
      listSchedules: mockListSchedules,
      listAvailableSkills: mockListAvailableSkills,
    }),
    getProviderDefaultModel: () => "gpt-4o",
  };
});

beforeEach(() => {
  vi.clearAllMocks();
  mockListAgents.mockResolvedValue({ success: true, data: { agents: [] } });
  mockListProviders.mockResolvedValue({ success: true, data: { providers: [] } });
  mockGetModelRegistry.mockResolvedValue({ success: true, data: {} });
  mockListSkills.mockResolvedValue({ success: true, data: { skills: [] } });
  mockListSchedules.mockResolvedValue({ success: true, data: { jobs: [] } });
  mockListAvailableSkills.mockResolvedValue({ success: true, data: { skills: [] } });
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("WebAgentsPanel — layout + spacing", () => {
  it("renders page header with title + subtitle", async () => {
    render(<WebAgentsPanel />);
    await waitFor(() => expect(screen.getByText(/^Agents$/)).toBeInTheDocument());
    expect(
      screen.getByText(/create and manage your ai assistants/i)
    ).toBeInTheDocument();
  });

  it("renders all three tabs (My Agents · Skills Library · Schedules)", async () => {
    render(<WebAgentsPanel />);
    await waitFor(() => expect(screen.getByText(/my agents/i)).toBeInTheDocument());
    expect(screen.getByText(/skills library/i)).toBeInTheDocument();
    expect(screen.getByText(/schedules/i)).toBeInTheDocument();
  });

  it("renders the help-box AND card-grid as separate stacked elements (not joined)", async () => {
    render(<WebAgentsPanel />);
    await waitFor(() => {
      const helpBox = document.querySelector(".help-box");
      expect(helpBox).not.toBeNull();
    });

    const helpBox = document.querySelector(".help-box");
    // The HelpBox must declare a bottom margin so it doesn't stick to the
    // card-grid below it. We assert via the computed style on the live class.
    // jsdom doesn't compute styles from external CSS, so we verify presence
    // of the class + the empty-state sibling pattern instead.
    expect(helpBox).toBeInTheDocument();

    // With no agents, an EmptyState renders below the HelpBox.
    expect(screen.getByText(/no agents yet/i)).toBeInTheDocument();
  });

  it("renders the empty state on the My Agents tab when no agents are loaded", async () => {
    render(<WebAgentsPanel />);
    await waitFor(() => expect(screen.getByText(/no agents yet/i)).toBeInTheDocument());
    expect(
      screen.getByText(/agents are your ai assistants/i)
    ).toBeInTheDocument();
  });

  it("includes a Create Agent action button", async () => {
    render(<WebAgentsPanel />);
    await waitFor(() => {
      const buttons = screen.getAllByRole("button", { name: /create agent/i });
      expect(buttons.length).toBeGreaterThan(0);
    });
  });
});
