// ============================================================================
// AgentEditPanel — Advanced section + voice-recording removal tests
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@/test/utils";
import { AgentEditPanel } from "./AgentEditPanel";
import type {
  AgentResponse,
  ProviderResponse,
  ModelRegistryResponse,
} from "@/services/transport";

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

const mockListMcps = vi.fn();
const mockUpdateAgent = vi.fn();

vi.mock("@/services/transport", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("@/services/transport");
  return {
    ...actual,
    getTransport: async () => ({
      listMcps: mockListMcps,
      updateAgent: mockUpdateAgent,
    }),
  };
});

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

function makeAgent(overrides: Partial<AgentResponse> = {}): AgentResponse {
  return {
    id: "agent-1",
    name: "researcher",
    displayName: "Researcher",
    description: "Investigates topics",
    providerId: "openai",
    model: "gpt-4o",
    temperature: 0.7,
    maxTokens: 4096,
    thinkingEnabled: false,
    voiceRecordingEnabled: false,
    instructions: "You are helpful.",
    mcps: [],
    skills: [],
    ...overrides,
  };
}

const PROVIDERS: ProviderResponse[] = [
  {
    id: "openai",
    name: "openai",
    displayName: "OpenAI",
    type: "openai",
    enabled: true,
    apiBaseUrl: "https://api.openai.com/v1",
  } as unknown as ProviderResponse,
];

const MODEL_REGISTRY: ModelRegistryResponse = {};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("AgentEditPanel — Advanced section", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockListMcps.mockResolvedValue({ success: true, data: { servers: [] } });
    mockUpdateAgent.mockResolvedValue({ success: true });
  });

  async function renderPanel(agent = makeAgent()) {
    const onClose = vi.fn();
    const onSave = vi.fn();
    const result = render(
      <AgentEditPanel
        agent={agent}
        providers={PROVIDERS}
        modelRegistry={MODEL_REGISTRY}
        onClose={onClose}
        onSave={onSave}
      />
    );
    // Wait for the loading spinner to clear and the form to mount.
    await waitFor(() => expect(screen.getByText(/basic information/i)).toBeInTheDocument());
    return { ...result, onClose, onSave };
  }

  it("does NOT render a Voice Recording toggle in Advanced", async () => {
    await renderPanel();
    fireEvent.click(screen.getByRole("button", { name: /advanced options/i }));
    expect(screen.queryByText(/voice recording/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/enable voice input/i)).not.toBeInTheDocument();
  });

  it("renders the Thinking Enabled toggle in Advanced", async () => {
    await renderPanel();
    fireEvent.click(screen.getByRole("button", { name: /advanced options/i }));
    expect(screen.getByText(/thinking enabled/i)).toBeInTheDocument();
    expect(screen.getByText(/allow the model to show reasoning steps/i)).toBeInTheDocument();
  });

  it("uses .toggle-switch--off when thinking is disabled", async () => {
    await renderPanel(makeAgent({ thinkingEnabled: false }));
    fireEvent.click(screen.getByRole("button", { name: /advanced options/i }));
    const thinkingRow = screen.getByText(/thinking enabled/i).closest(".skill-toggle");
    expect(thinkingRow).not.toBeNull();
    const toggle = thinkingRow!.querySelector(".toggle-switch");
    expect(toggle?.classList.contains("toggle-switch--off")).toBe(true);
    expect(toggle?.classList.contains("toggle-switch--on")).toBe(false);
  });

  it("uses .toggle-switch--on when thinking is enabled", async () => {
    await renderPanel(makeAgent({ thinkingEnabled: true }));
    fireEvent.click(screen.getByRole("button", { name: /advanced options/i }));
    const thinkingRow = screen.getByText(/thinking enabled/i).closest(".skill-toggle");
    const toggle = thinkingRow!.querySelector(".toggle-switch");
    expect(toggle?.classList.contains("toggle-switch--on")).toBe(true);
    expect(toggle?.classList.contains("toggle-switch--off")).toBe(false);
  });

  it("flips the thinking toggle when the row is clicked", async () => {
    await renderPanel(makeAgent({ thinkingEnabled: false }));
    fireEvent.click(screen.getByRole("button", { name: /advanced options/i }));
    const thinkingRow = screen.getByText(/thinking enabled/i).closest(".skill-toggle")!;
    fireEvent.click(thinkingRow);
    const toggle = thinkingRow.querySelector(".toggle-switch");
    expect(toggle?.classList.contains("toggle-switch--on")).toBe(true);
  });

  it("does not send voiceRecordingEnabled in the update payload on Save", async () => {
    const { onSave } = await renderPanel();
    fireEvent.click(screen.getByRole("button", { name: /save changes/i }));
    await waitFor(() => expect(mockUpdateAgent).toHaveBeenCalledTimes(1));
    const [, payload] = mockUpdateAgent.mock.calls[0];
    expect(payload).not.toHaveProperty("voiceRecordingEnabled");
    expect(payload).toHaveProperty("thinkingEnabled");
    await waitFor(() => expect(onSave).toHaveBeenCalled());
  });

  it("collapses Advanced section by default", async () => {
    await renderPanel();
    expect(screen.queryByText(/thinking enabled/i)).not.toBeInTheDocument();
  });
});
