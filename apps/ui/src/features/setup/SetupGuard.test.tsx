// ============================================================================
// SetupGuard — setup check and navigation tests
// ============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { Transport } from "@/services/transport";

const getSetupStatus = vi.fn<Transport["getSetupStatus"]>();
const mockNavigate = vi.fn();

vi.mock("@/services/transport", () => ({
  getTransport: async () => ({ getSetupStatus }),
}));

vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>("react-router-dom");
  return {
    ...actual,
    useNavigate: () => mockNavigate,
  };
});

import { SetupGuard } from "./SetupGuard";

beforeEach(() => {
  getSetupStatus.mockReset();
  mockNavigate.mockReset();
  sessionStorage.removeItem("setupComplete");
});

function Wrapper({ path = "/" }: { path?: string }) {
  return (
    <MemoryRouter initialEntries={[path]}>
      <SetupGuard>
        <div data-testid="children">App Content</div>
      </SetupGuard>
    </MemoryRouter>
  );
}

describe("SetupGuard", () => {
  it("renders children immediately on /setup path without fetching", async () => {
    render(<Wrapper path="/setup" />);
    await waitFor(() => {
      expect(screen.getByTestId("children")).toBeInTheDocument();
    });
    expect(getSetupStatus).not.toHaveBeenCalled();
  });

  it("renders children when setupComplete is cached in sessionStorage", async () => {
    sessionStorage.setItem("setupComplete", "true");
    render(<Wrapper path="/" />);
    await waitFor(() => {
      expect(screen.getByTestId("children")).toBeInTheDocument();
    });
    expect(getSetupStatus).not.toHaveBeenCalled();
  });

  it("shows loading spinner while checking", () => {
    getSetupStatus.mockReturnValue(new Promise(() => { /* never resolves */ }));
    const { container } = render(<Wrapper path="/" />);
    // The spinner is rendered while the check is in progress
    expect(container.querySelector(".loading-spinner")).toBeInTheDocument();
  });

  it("renders children when setup is complete", async () => {
    getSetupStatus.mockResolvedValue({
      success: true,
      data: { setupComplete: true, hasProviders: false },
    });
    render(<Wrapper path="/" />);
    await waitFor(() => {
      expect(screen.getByTestId("children")).toBeInTheDocument();
    });
  });

  it("renders children when hasProviders is true", async () => {
    getSetupStatus.mockResolvedValue({
      success: true,
      data: { setupComplete: false, hasProviders: true },
    });
    render(<Wrapper path="/" />);
    await waitFor(() => {
      expect(screen.getByTestId("children")).toBeInTheDocument();
    });
  });

  it("navigates to /setup when neither setupComplete nor hasProviders", async () => {
    getSetupStatus.mockResolvedValue({
      success: true,
      data: { setupComplete: false, hasProviders: false },
    });
    render(<Wrapper path="/" />);
    await waitFor(() => {
      expect(mockNavigate).toHaveBeenCalledWith("/setup", { replace: true });
    });
  });

  it("does not block when transport throws", async () => {
    getSetupStatus.mockRejectedValue(new Error("network error"));
    render(<Wrapper path="/" />);
    await waitFor(() => {
      expect(screen.getByTestId("children")).toBeInTheDocument();
    });
  });

  it("caches setupComplete in sessionStorage after successful check", async () => {
    getSetupStatus.mockResolvedValue({
      success: true,
      data: { setupComplete: true, hasProviders: false },
    });
    render(<Wrapper path="/" />);
    await waitFor(() => {
      expect(screen.getByTestId("children")).toBeInTheDocument();
    });
    expect(sessionStorage.getItem("setupComplete")).toBe("true");
  });
});
