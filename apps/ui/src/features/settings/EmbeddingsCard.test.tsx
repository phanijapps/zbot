// ============================================================================
// EmbeddingsCard tests
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@/test/utils";

import type {
  CuratedModel,
  EmbeddingsHealth,
  EmbeddingConfig,
} from "@/services/transport";

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

const mockGetHealth = vi.fn();
const mockGetModels = vi.fn();
const mockConfigure = vi.fn();

vi.mock("@/services/transport", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("@/services/transport");
  return {
    ...actual,
    getTransport: async () => ({
      getEmbeddingsHealth: mockGetHealth,
      getEmbeddingsModels: mockGetModels,
      configureEmbeddings: mockConfigure,
    }),
  };
});

// Minimal stub for the progress modal so we don't have to simulate SSE here.
vi.mock("./EmbeddingProgressModal", () => ({
  EmbeddingProgressModal: ({ config }: { config: EmbeddingConfig }) => (
    <div data-testid="progress-modal-stub">{JSON.stringify(config)}</div>
  ),
}));

import { EmbeddingsCard } from "./EmbeddingsCard";

const INTERNAL_HEALTH: EmbeddingsHealth = {
  backend: "internal",
  dim: 384,
  status: "ready",
  indexed_count: 100,
};

const MODELS: CuratedModel[] = [
  { tag: "nomic-embed-text", dim: 768, size_mb: 274, mteb: 62.4 },
  { tag: "bge-small-v1", dim: 384, size_mb: 130, mteb: 62.1 },
  { tag: "mxbai-embed-large", dim: 1024, size_mb: 670, mteb: 64.7 },
];

beforeEach(() => {
  mockGetHealth.mockReset();
  mockGetModels.mockReset();
  mockConfigure.mockReset();
  mockGetHealth.mockResolvedValue({ success: true, data: INTERNAL_HEALTH });
  mockGetModels.mockResolvedValue({ success: true, data: MODELS });
  mockConfigure.mockResolvedValue({ success: true, data: INTERNAL_HEALTH });
});

describe("EmbeddingsCard", () => {
  it("renders internal default state after loading health", async () => {
    render(<EmbeddingsCard />);
    await waitFor(() => expect(screen.getByRole("checkbox", { name: /use internal embedding/i })).toBeInTheDocument());
    const toggleInput = screen.getByRole("checkbox", { name: /use internal embedding/i }) as HTMLInputElement;
    expect(toggleInput.checked).toBe(true);
    expect(screen.getByTestId("embeddings-status-footer").textContent).toMatch(/internal/);
    expect(screen.getByTestId("embeddings-status-footer").textContent).toMatch(/384d/);
    expect(screen.getByTestId("embeddings-status-footer").textContent).toMatch(/100 indexed/);
  });

  it("reveals Ollama subform when internal toggle is disabled", async () => {
    render(<EmbeddingsCard />);
    await waitFor(() => screen.getByRole("checkbox", { name: /use internal embedding/i }));
    const toggleInput = screen.getByRole("checkbox", { name: /use internal embedding/i }) as HTMLInputElement;
    fireEvent.click(toggleInput);
    expect(screen.getByLabelText(/ollama base url/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/^model$/i)).toBeInTheDocument();
  });

  it("shows 'no reindex' suffix for models with matching dim", async () => {
    render(<EmbeddingsCard />);
    await waitFor(() => screen.getByRole("checkbox", { name: /use internal embedding/i }));
    const toggleInput = screen.getByRole("checkbox", { name: /use internal embedding/i }) as HTMLInputElement;
    fireEvent.click(toggleInput);
    const select = screen.getByLabelText(/^model$/i) as HTMLSelectElement;
    const options = Array.from(select.options).map((o) => o.textContent);
    expect(options.some((t) => t?.includes("bge-small-v1") && t?.includes("no reindex"))).toBe(true);
    expect(options.some((t) => t?.includes("nomic-embed-text") && t?.includes("no reindex"))).toBe(false);
  });

  it("shows warning with estimated time when dim differs", async () => {
    render(<EmbeddingsCard />);
    await waitFor(() => screen.getByRole("checkbox", { name: /use internal embedding/i }));
    const toggleInput = screen.getByRole("checkbox", { name: /use internal embedding/i }) as HTMLInputElement;
    fireEvent.click(toggleInput);
    const select = screen.getByLabelText(/^model$/i) as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "nomic-embed-text" } });
    const warning = await screen.findByTestId("emb-warning");
    expect(warning.textContent).toMatch(/nomic-embed-text/);
    expect(warning.textContent).toMatch(/~274MB/);
    expect(warning.textContent).toMatch(/reindex 100 embeddings/);
    // 100 * 0.4 = 40s for ollama target
    expect(warning.textContent).toMatch(/~40s/);
  });

  it("disables Save & Switch when form matches current health state", async () => {
    render(<EmbeddingsCard />);
    await waitFor(() => screen.getByRole("checkbox", { name: /use internal embedding/i }));
    const save = screen.getByRole("button", { name: /save & switch/i });
    expect(save).toBeDisabled();
  });

  it("opens progress modal with form contents on submit", async () => {
    render(<EmbeddingsCard />);
    await waitFor(() => screen.getByRole("checkbox", { name: /use internal embedding/i }));
    const toggleInput = screen.getByRole("checkbox", { name: /use internal embedding/i }) as HTMLInputElement;
    fireEvent.click(toggleInput);
    const select = screen.getByLabelText(/^model$/i) as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "nomic-embed-text" } });

    const save = screen.getByRole("button", { name: /save & switch/i });
    expect(save).not.toBeDisabled();
    fireEvent.click(save);

    const modal = await screen.findByTestId("progress-modal-stub");
    const parsed = JSON.parse(modal.textContent!) as EmbeddingConfig;
    expect(parsed.backend).toBe("ollama");
    expect(parsed.dimensions).toBe(768);
    expect(parsed.ollama?.model).toBe("nomic-embed-text");
    expect(parsed.ollama?.base_url).toBe("http://localhost:11434");
  });
});

