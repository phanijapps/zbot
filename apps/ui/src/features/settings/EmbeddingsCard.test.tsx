// ============================================================================
// EmbeddingsCard tests
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@/test/utils";

import type {
  CuratedModel,
  EmbeddingsHealth,
  EmbeddingConfig,
  OllamaModelsResponse,
} from "@/services/transport";

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

const mockGetHealth = vi.fn();
const mockGetModels = vi.fn();
const mockGetOllamaModels = vi.fn();
const mockConfigure = vi.fn();

vi.mock("@/services/transport", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("@/services/transport");
  return {
    ...actual,
    getTransport: async () => ({
      getEmbeddingsHealth: mockGetHealth,
      getEmbeddingsModels: mockGetModels,
      getOllamaEmbeddingModels: mockGetOllamaModels,
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
  { tag: "nomic-embed-text", dim: 768, size_mb: 274, mteb: 62 },
  { tag: "bge-small-v1", dim: 384, size_mb: 130, mteb: 62 },
  { tag: "mxbai-embed-large", dim: 1024, size_mb: 670, mteb: 65 },
];

const UNREACHABLE_OLLAMA: OllamaModelsResponse = {
  all: [],
  likely_embedding: [],
  reachable: false,
};

beforeEach(() => {
  mockGetHealth.mockReset();
  mockGetModels.mockReset();
  mockGetOllamaModels.mockReset();
  mockConfigure.mockReset();
  mockGetHealth.mockResolvedValue({ success: true, data: INTERNAL_HEALTH });
  mockGetModels.mockResolvedValue({ success: true, data: MODELS });
  mockGetOllamaModels.mockResolvedValue({ success: true, data: UNREACHABLE_OLLAMA });
  mockConfigure.mockResolvedValue({ success: true, data: INTERNAL_HEALTH });
});

describe("EmbeddingsCard", () => {
  it("renders internal default state after loading health", async () => {
    render(<EmbeddingsCard />);
    await waitFor(() =>
      expect(screen.getByRole("checkbox", { name: /use internal embedding/i })).toBeInTheDocument(),
    );
    const toggle = screen.getByRole("checkbox", { name: /use internal embedding/i }) as HTMLInputElement;
    expect(toggle.checked).toBe(true);
    expect(screen.getByTestId("embeddings-status-footer").textContent).toMatch(/internal/);
    expect(screen.getByTestId("embeddings-status-footer").textContent).toMatch(/384d/);
    expect(screen.getByTestId("embeddings-status-footer").textContent).toMatch(/100 indexed/);
  });

  it("reveals Ollama subform with URL, model, dimensions fields when internal is off", async () => {
    render(<EmbeddingsCard />);
    await waitFor(() => screen.getByRole("checkbox", { name: /use internal embedding/i }));
    const toggle = screen.getByRole("checkbox", { name: /use internal embedding/i }) as HTMLInputElement;
    fireEvent.click(toggle);
    expect(screen.getByLabelText(/ollama base url/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/^model$/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/^dimensions$/i)).toBeInTheDocument();
  });

  it("auto-fills dimensions when the typed model matches a curated entry", async () => {
    render(<EmbeddingsCard />);
    await waitFor(() => screen.getByRole("checkbox", { name: /use internal embedding/i }));
    const toggle = screen.getByRole("checkbox", { name: /use internal embedding/i }) as HTMLInputElement;
    fireEvent.click(toggle);

    const modelInput = screen.getByLabelText(/^model$/i) as HTMLInputElement;
    const dimInput = screen.getByLabelText(/^dimensions$/i) as HTMLInputElement;

    fireEvent.change(modelInput, { target: { value: "nomic-embed-text" } });
    expect(dimInput.value).toBe("768");

    fireEvent.change(modelInput, { target: { value: "mxbai-embed-large" } });
    expect(dimInput.value).toBe("1024");
  });

  it("leaves dimensions editable for custom (non-curated) models", async () => {
    render(<EmbeddingsCard />);
    await waitFor(() => screen.getByRole("checkbox", { name: /use internal embedding/i }));
    const toggle = screen.getByRole("checkbox", { name: /use internal embedding/i }) as HTMLInputElement;
    fireEvent.click(toggle);

    const modelInput = screen.getByLabelText(/^model$/i) as HTMLInputElement;
    const dimInput = screen.getByLabelText(/^dimensions$/i) as HTMLInputElement;

    fireEvent.change(modelInput, { target: { value: "my-custom-embedder" } });
    // Didn't match curated — dim not auto-filled.
    fireEvent.change(dimInput, { target: { value: "512" } });
    expect(dimInput.value).toBe("512");
    expect(screen.getByText(/custom model — dimensions not auto-filled/i)).toBeInTheDocument();
  });

  it("disables Save & Switch when form matches current health state", async () => {
    render(<EmbeddingsCard />);
    await waitFor(() => screen.getByRole("checkbox", { name: /use internal embedding/i }));
    const save = screen.getByRole("button", { name: /save & switch/i });
    expect(save).toBeDisabled();
  });

  it("submits the new wire-shape { internal, ollama: {url, model, dimensions} } on save", async () => {
    render(<EmbeddingsCard />);
    await waitFor(() => screen.getByRole("checkbox", { name: /use internal embedding/i }));
    const toggle = screen.getByRole("checkbox", { name: /use internal embedding/i }) as HTMLInputElement;
    fireEvent.click(toggle);

    const modelInput = screen.getByLabelText(/^model$/i) as HTMLInputElement;
    fireEvent.change(modelInput, { target: { value: "nomic-embed-text" } });

    const save = screen.getByRole("button", { name: /save & switch/i });
    expect(save).not.toBeDisabled();
    fireEvent.click(save);

    const modal = await screen.findByTestId("progress-modal-stub");
    const parsed = JSON.parse(modal.textContent!) as EmbeddingConfig;
    expect(parsed.internal).toBe(false);
    expect(parsed.ollama?.url).toBe("http://localhost:11434");
    expect(parsed.ollama?.model).toBe("nomic-embed-text");
    expect(parsed.ollama?.dimensions).toBe(768);
  });
});
