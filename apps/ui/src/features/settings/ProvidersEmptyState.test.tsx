// ============================================================================
// ProvidersEmptyState — featured presets, expand-to-connect, custom button
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@/test/utils";
import { ProvidersEmptyState } from "./ProvidersEmptyState";

const mockTestProvider = vi.fn();
const mockCreateProvider = vi.fn();
const mockSetDefaultProvider = vi.fn();

vi.mock("@/services/transport", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("@/services/transport");
  return {
    ...actual,
    getTransport: async () => ({
      testProvider: mockTestProvider,
      createProvider: mockCreateProvider,
      setDefaultProvider: mockSetDefaultProvider,
    }),
  };
});

beforeEach(() => {
  vi.clearAllMocks();
  mockTestProvider.mockResolvedValue({ success: true, data: { success: true } });
  mockCreateProvider.mockResolvedValue({ success: true, data: { id: "new-id" } });
  mockSetDefaultProvider.mockResolvedValue({ success: true });
});

describe("ProvidersEmptyState", () => {
  it("renders the Get Started hero with the three featured presets", () => {
    render(
      <ProvidersEmptyState
        existingProviders={[]}
        onProviderCreated={() => {}}
        onOpenCustom={() => {}}
      />
    );
    expect(screen.getByText(/get started/i)).toBeInTheDocument();
    expect(screen.getByText(/openai/i)).toBeInTheDocument();
    expect(screen.getByText(/anthropic/i)).toBeInTheDocument();
    expect(screen.getByText(/ollama cloud/i)).toBeInTheDocument();
  });

  it("does not show the inline API key form by default", () => {
    render(
      <ProvidersEmptyState
        existingProviders={[]}
        onProviderCreated={() => {}}
        onOpenCustom={() => {}}
      />
    );
    expect(screen.queryByLabelText(/api key/i)).not.toBeInTheDocument();
  });

  it("expands the inline form when a non-Ollama preset is clicked", () => {
    render(
      <ProvidersEmptyState
        existingProviders={[]}
        onProviderCreated={() => {}}
        onOpenCustom={() => {}}
      />
    );
    fireEvent.click(screen.getByText(/^openai$/i).closest('[role="button"]') as HTMLElement);
    expect(screen.getByLabelText(/api key/i)).toBeInTheDocument();
  });

  it("offers a Show more toggle when there are non-featured providers", () => {
    render(
      <ProvidersEmptyState
        existingProviders={[]}
        onProviderCreated={() => {}}
        onOpenCustom={() => {}}
      />
    );
    expect(screen.getByText(/show \d+ more providers/i)).toBeInTheDocument();
  });

  it("opens the custom-provider entry when 'Custom provider' is clicked", () => {
    const onOpenCustom = vi.fn();
    render(
      <ProvidersEmptyState
        existingProviders={[]}
        onProviderCreated={() => {}}
        onOpenCustom={onOpenCustom}
      />
    );
    fireEvent.click(screen.getByRole("button", { name: /custom provider/i }));
    expect(onOpenCustom).toHaveBeenCalledTimes(1);
  });

  it("calls testProvider then createProvider when an API key is submitted", async () => {
    const onProviderCreated = vi.fn();
    render(
      <ProvidersEmptyState
        existingProviders={[]}
        onProviderCreated={onProviderCreated}
        onOpenCustom={() => {}}
      />
    );
    fireEvent.click(screen.getByText(/^openai$/i).closest('[role="button"]') as HTMLElement);
    const input = screen.getByLabelText(/api key/i);
    fireEvent.change(input, { target: { value: "sk-test-123" } });
    fireEvent.click(screen.getByRole("button", { name: /test & connect/i }));
    await waitFor(() => expect(mockTestProvider).toHaveBeenCalled());
    await waitFor(() => expect(mockCreateProvider).toHaveBeenCalled());
    await waitFor(() => expect(onProviderCreated).toHaveBeenCalled());
  });
});
