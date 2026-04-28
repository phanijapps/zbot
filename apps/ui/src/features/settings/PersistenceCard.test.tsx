// ============================================================================
// PersistenceCard tests
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@/test/utils";

import type { ExecutionSettings } from "@/services/transport";

const mockGetExec = vi.fn();
const mockUpdateExec = vi.fn();

vi.mock("@/services/transport", async () => {
  const actual = await vi.importActual<Record<string, unknown>>("@/services/transport");
  return {
    ...actual,
    getTransport: async () => ({
      getExecutionSettings: mockGetExec,
      updateExecutionSettings: mockUpdateExec,
    }),
  };
});

import { PersistenceCard } from "./PersistenceCard";

const BASE_EXEC: ExecutionSettings & { restartRequired: boolean } = {
  maxParallelAgents: 2,
  setupComplete: true,
  featureFlags: {},
  restartRequired: false,
};

beforeEach(() => {
  mockGetExec.mockReset();
  mockUpdateExec.mockReset();
  mockGetExec.mockResolvedValue({ success: true, data: BASE_EXEC });
  mockUpdateExec.mockResolvedValue({
    success: true,
    data: {
      ...BASE_EXEC,
      featureFlags: { surreal_backend: true },
      restartRequired: true,
    },
  });
});

describe("PersistenceCard", () => {
  it("renders the Persistence header after loading settings", async () => {
    render(<PersistenceCard />);
    await waitFor(() =>
      expect(
        screen.getByRole("heading", { name: /persistence/i, level: 2 }),
      ).toBeInTheDocument(),
    );
  });

  it("defaults to SQLite when feature flag is unset", async () => {
    render(<PersistenceCard />);
    const select = (await screen.findByLabelText(
      /knowledge backend/i,
    )) as HTMLSelectElement;
    expect(select.value).toBe("sqlite");
  });

  it("shows SurrealDB selected when feature flag is true", async () => {
    mockGetExec.mockResolvedValue({
      success: true,
      data: { ...BASE_EXEC, featureFlags: { surreal_backend: true } },
    });
    render(<PersistenceCard />);
    const select = (await screen.findByLabelText(
      /knowledge backend/i,
    )) as HTMLSelectElement;
    expect(select.value).toBe("surreal");
  });

  it("calls updateExecutionSettings with surreal_backend=true on switch", async () => {
    render(<PersistenceCard />);
    const select = (await screen.findByLabelText(
      /knowledge backend/i,
    )) as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "surreal" } });
    await waitFor(() => expect(mockUpdateExec).toHaveBeenCalledTimes(1));
    const call = mockUpdateExec.mock.calls[0][0] as ExecutionSettings;
    expect(call.featureFlags?.surreal_backend).toBe(true);
  });

  it("calls updateExecutionSettings with surreal_backend=false on switch back", async () => {
    mockGetExec.mockResolvedValue({
      success: true,
      data: { ...BASE_EXEC, featureFlags: { surreal_backend: true } },
    });
    mockUpdateExec.mockResolvedValue({
      success: true,
      data: {
        ...BASE_EXEC,
        featureFlags: { surreal_backend: false },
        restartRequired: true,
      },
    });
    render(<PersistenceCard />);
    const select = (await screen.findByLabelText(
      /knowledge backend/i,
    )) as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "sqlite" } });
    await waitFor(() => expect(mockUpdateExec).toHaveBeenCalledTimes(1));
    const call = mockUpdateExec.mock.calls[0][0] as ExecutionSettings;
    expect(call.featureFlags?.surreal_backend).toBe(false);
  });

  it("shows the restart banner when SurrealDB is selected", async () => {
    mockGetExec.mockResolvedValue({
      success: true,
      data: { ...BASE_EXEC, featureFlags: { surreal_backend: true } },
    });
    render(<PersistenceCard />);
    await waitFor(() =>
      expect(
        screen.getByText(/SurrealDB selected/i),
      ).toBeInTheDocument(),
    );
    expect(
      screen.getByText(/npm run daemon:surreal:watch/i),
    ).toBeInTheDocument();
  });

  it("surfaces a save error when the transport fails", async () => {
    mockUpdateExec.mockResolvedValue({
      success: false,
      error: "kaboom",
    });
    render(<PersistenceCard />);
    const select = (await screen.findByLabelText(
      /knowledge backend/i,
    )) as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "surreal" } });
    await waitFor(() =>
      expect(screen.getByText(/kaboom/i)).toBeInTheDocument(),
    );
  });

  it("documents the recovery path", async () => {
    render(<PersistenceCard />);
    await waitFor(() =>
      expect(screen.getByText(/Recovery:/i)).toBeInTheDocument(),
    );
    expect(
      screen.getByText(/zero-stores-surreal-recovery/i),
    ).toBeInTheDocument();
  });
});
