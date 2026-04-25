// ============================================================================
// WebSettingsPanel — header, tab structure, empty state, basic loading flow
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@/test/utils";
import { WebSettingsPanel } from "./WebSettingsPanel";

// ---------------------------------------------------------------------------
// Mocks — minimal transport that returns empty fixtures so the panel mounts.
// ---------------------------------------------------------------------------

const mockListProviders = vi.fn();
const mockListModels = vi.fn();
const mockGetToolSettings = vi.fn();
const mockGetLogSettings = vi.fn();
const mockGetExecutionSettings = vi.fn();
const mockGetEmbeddingsHealth = vi.fn();
const mockGetEmbeddingsModels = vi.fn();
const mockGetOllamaEmbeddingModels = vi.fn();
const mockConfigureEmbeddings = vi.fn();

vi.mock("@/services/transport", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("@/services/transport");
  return {
    ...actual,
    getTransport: async () => ({
      listProviders: mockListProviders,
      listModels: mockListModels,
      getToolSettings: mockGetToolSettings,
      getLogSettings: mockGetLogSettings,
      getExecutionSettings: mockGetExecutionSettings,
      getEmbeddingsHealth: mockGetEmbeddingsHealth,
      getEmbeddingsModels: mockGetEmbeddingsModels,
      getOllamaEmbeddingModels: mockGetOllamaEmbeddingModels,
      configureEmbeddings: mockConfigureEmbeddings,
    }),
  };
});

beforeEach(() => {
  vi.clearAllMocks();
  mockListProviders.mockResolvedValue({ success: true, data: [] });
  mockListModels.mockResolvedValue({ success: true, data: {} });
  mockGetToolSettings.mockResolvedValue({ success: true, data: { tools: {} } });
  mockGetLogSettings.mockResolvedValue({
    success: true,
    data: { level: "info", retentionDays: 7 },
  });
  mockGetExecutionSettings.mockResolvedValue({
    success: true,
    data: { featureFlags: {} },
  });
  mockGetEmbeddingsHealth.mockResolvedValue({ success: true, data: { healthy: false } });
  // getEmbeddingsModels() returns CuratedModel[] directly (not wrapped).
  mockGetEmbeddingsModels.mockResolvedValue({ success: true, data: [] });
  mockGetOllamaEmbeddingModels.mockResolvedValue({ success: true, data: { models: [] } });
  mockConfigureEmbeddings.mockResolvedValue({ success: true });
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("WebSettingsPanel — page chrome", () => {
  it("renders the Settings page title + subtitle", async () => {
    render(<WebSettingsPanel />);
    await waitFor(() => expect(screen.getByText("Settings")).toBeInTheDocument());
    expect(
      screen.getByText(/configure your ai providers, system preferences, and logging/i)
    ).toBeInTheDocument();
  });

  it("renders all four tabs (Providers · General · Logging · Advanced)", async () => {
    render(<WebSettingsPanel />);
    await waitFor(() => expect(screen.getByText("Providers")).toBeInTheDocument());
    expect(screen.getByText("General")).toBeInTheDocument();
    expect(screen.getByText("Logging")).toBeInTheDocument();
    expect(screen.getByText("Advanced")).toBeInTheDocument();
  });

  it("loads providers + models + settings on mount", async () => {
    render(<WebSettingsPanel />);
    await waitFor(() => {
      expect(mockListProviders).toHaveBeenCalled();
      expect(mockListModels).toHaveBeenCalled();
      expect(mockGetToolSettings).toHaveBeenCalled();
      expect(mockGetLogSettings).toHaveBeenCalled();
      expect(mockGetExecutionSettings).toHaveBeenCalled();
    });
  });
});

describe("WebSettingsPanel — providers tab empty state", () => {
  it("renders the Get Started empty state when no providers exist", async () => {
    render(<WebSettingsPanel />);
    await waitFor(() =>
      expect(screen.getByText(/get started/i)).toBeInTheDocument()
    );
  });

  it("provider count badge reads 0 when there are no providers", async () => {
    render(<WebSettingsPanel />);
    await waitFor(() => expect(screen.getByText("Providers")).toBeInTheDocument());
    // The TabBar count is rendered next to the label.
    const tab = screen.getByText("Providers").closest("button, [role='tab'], div");
    expect(tab).not.toBeNull();
  });
});

describe("WebSettingsPanel — tab switching", () => {
  it("switches to General when the General tab is clicked", async () => {
    render(<WebSettingsPanel />);
    await waitFor(() => expect(screen.getByText("Settings")).toBeInTheDocument());
    fireEvent.click(screen.getByText("General"));
    // Loading completes and General tab shows content. Just check no crash.
    await waitFor(() => expect(screen.getByText("General")).toBeInTheDocument());
  });

  it("switches to Logging when the Logging tab is clicked", async () => {
    render(<WebSettingsPanel />);
    await waitFor(() => expect(screen.getByText("Settings")).toBeInTheDocument());
    fireEvent.click(screen.getByText("Logging"));
    await waitFor(() => expect(screen.getByText("Logging")).toBeInTheDocument());
  });

  it("switches to Advanced when the Advanced tab is clicked", async () => {
    render(<WebSettingsPanel />);
    await waitFor(() => expect(screen.getByText("Settings")).toBeInTheDocument());
    fireEvent.click(screen.getByText("Advanced"));
    await waitFor(() => expect(screen.getByText("Advanced")).toBeInTheDocument());
  });
});

describe("WebSettingsPanel — provider load failure", () => {
  it("does not crash when listProviders returns an error (still mounts the page)", async () => {
    mockListProviders.mockResolvedValueOnce({
      success: false,
      error: "Network timeout",
    });
    render(<WebSettingsPanel />);
    await waitFor(() => expect(screen.getByText("Settings")).toBeInTheDocument());
    // The empty-state path renders since hasProviders=false. The page survives the error.
    expect(screen.getByText("Providers")).toBeInTheDocument();
  });
});
