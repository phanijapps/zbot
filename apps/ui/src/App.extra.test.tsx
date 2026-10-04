// ============================================================================
// App — initialization and error state tests
// Tests the default App export and VersionBadge/ResearchV2Redirect internals.
// ============================================================================

import { StrictMode } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

// ─── Mock transport ───────────────────────────────────────────────────────────

const health = vi.fn();
const connect = vi.fn();
const disconnect = vi.fn();
const commissioningStatus = vi.fn();
const createChatSession = vi.fn();
const deleteChatSession = vi.fn();

vi.mock("@/services/transport", () => ({
  initializeTransport: vi.fn(async () => {}),
  getTransport: vi.fn(async () => ({ health, connect, disconnect, getCommissioningStatus: commissioningStatus, createChatSession, deleteChatSession, listSessionsFull: async () => ({success:true,data:[]}) })),
}));

// Mock all the heavy child pages so they don't need their own transport
vi.mock("./features/commissioning", async () => {
  const actual = await vi.importActual<typeof import("./features/commissioning/CommissioningGuard")>("./features/commissioning/CommissioningGuard");
  return { CommissioningGuard: actual.CommissioningGuard, CommissioningScreen: () => <div>CommissioningScreen</div> };
});
vi.mock("./features/agent/WebAgentsPanel", () => ({ WebAgentsPanel: () => <div>WebAgentsPanel</div> }));
vi.mock("./features/settings/WebSettingsPanel", () => ({ WebSettingsPanel: () => <div>WebSettingsPanel</div> }));
vi.mock("./features/integrations/WebIntegrationsPanel", () => ({ WebIntegrationsPanel: () => <div>WebIntegrationsPanel</div> }));
vi.mock("./features/memory", () => ({ MemoryTab: () => <div>MemoryTab</div> }));
vi.mock("./features/observatory", () => ({ ObservatoryPage: () => <div>ObservatoryPage</div> }));
vi.mock("./features/chat-v2", () => ({ QuickChat: () => <div>QuickChat</div> }));
vi.mock("./features/research-v2", () => ({ ResearchPage: () => <div>ResearchPage</div> }));
vi.mock("./features/mission-control", () => ({ MissionControlPage: () => <div>MissionControlPage</div> }));
vi.mock("./components/AccentPicker", () => ({ AccentPicker: () => <button aria-label="theme accent">theme</button> }));

import { initializeTransport } from "@/services/transport";
import App from "./App";

// STUB: AC11 — real App routing/guard, existing page doubles, no production migration yet.
describe("Desktop administration routes", () => {
  const pages = [["/agents", "WebAgentsPanel"], ["/settings", "WebSettingsPanel"], ["/integrations", "WebIntegrationsPanel"]];

  it.each(pages)("renders %s with desktop navigation and reload-safe session return", async (path, content) => {
    window.history.replaceState({}, "", `${path}?returnTo=%2Fsession%2Fsess-kept`);
    const first = render(<App />);
    expect(await screen.findByText(content)).toBeVisible();
    expect(screen.queryByRole("navigation", {name:"Primary"})).not.toBeInTheDocument();
    expect(screen.queryByRole("navigation", {name:"Mobile primary"})).not.toBeInTheDocument();
    expect(screen.queryByRole("link", {name:"z-Bot home"})).not.toBeInTheDocument();
    expect(screen.getByRole("complementary", {name:"Conversation navigation"})).toBeVisible();
    expect(screen.queryByRole("button", {name:/theme accent/i})).not.toBeInTheDocument();
    expect(screen.getByRole("link", {name:"Back to conversation"})).toHaveAttribute("href", "/session/sess-kept");
    first.unmount();
    render(<App />);
    await screen.findByText(content);
    expect(screen.queryByRole("navigation", {name:"Primary"})).not.toBeInTheDocument();
    expect(screen.queryByRole("navigation", {name:"Mobile primary"})).not.toBeInTheDocument();
    expect(screen.queryByRole("link", {name:"z-Bot home"})).not.toBeInTheDocument();
    expect(screen.getByRole("link", {name:"Back to conversation"})).toHaveAttribute("href", "/session/sess-kept");
    expect(createChatSession).not.toHaveBeenCalled();
    expect(deleteChatSession).not.toHaveBeenCalled();
  });

  it.each(pages)("keeps %s behind the real commissioning guard", async (path, content) => {
    commissioningStatus.mockResolvedValue({success:true,data:{state:"pending"}});
    window.history.replaceState({}, "", path);
    render(<App />);
    expect(await screen.findByText("CommissioningScreen")).toBeVisible();
    expect(screen.queryByText(content)).not.toBeInTheDocument();
    expect(window.location.pathname).toBe("/commission");
    expect(createChatSession).not.toHaveBeenCalled();
    expect(deleteChatSession).not.toHaveBeenCalled();
  });

  it.each(pages)("rejects an external return target on %s", async (path, content) => {
    window.history.replaceState({}, "", `${path}?returnTo=${encodeURIComponent("https://example.test/session")}`);
    render(<App />);
    await screen.findByText(content);
    expect(screen.getByRole("link", {name:"Back to conversation"})).toHaveAttribute("href", "/session");
  });

  it.each([
    ["/providers", "/settings", "", "WebSettingsPanel"],
    ["/skills", "/agents", "skills", "WebAgentsPanel"],
    ["/hooks", "/agents", "schedules", "WebAgentsPanel"],
    ["/connectors", "/integrations", "plugins", "WebIntegrationsPanel"],
    ["/mcps", "/integrations", "", "WebIntegrationsPanel"],
  ])("preserves the legacy %s alias and target tab", async (alias, path, tab, content) => {
    for (const target of [null, "/session/sess-kept", "https://example.test/session"]) {
      window.history.replaceState({}, "", target ? `${alias}?returnTo=${encodeURIComponent(target)}` : alias);
      const view = render(<App />);
      expect(await screen.findByText(content)).toBeVisible();
      expect(window.location.pathname).toBe(path);
      expect(new URLSearchParams(window.location.search).get("tab") ?? "").toBe(tab);
      expect(screen.getByRole("link", {name:"Back to conversation"})).toHaveAttribute("href", target === "/session/sess-kept" ? target : "/session");
      expect(screen.getByRole("complementary", {name:"Conversation navigation"})).toBeVisible();
      expect(createChatSession).not.toHaveBeenCalled();
      expect(deleteChatSession).not.toHaveBeenCalled();
      view.unmount();
    }
  });
});

// STUB: AC8 — observable route chrome and refresh-safe return destination.
describe("Desktop knowledge routes", () => {
  it.each([["/memory", "MemoryTab"], ["/observatory", "ObservatoryPage"]])("renders %s with desktop navigation, not legacy chrome", async (path, content) => {
    window.history.replaceState({}, "", `${path}?returnTo=${encodeURIComponent("/session/sess-kept")}`);
    render(<App />);
    expect(await screen.findByText(content)).toBeVisible();
    expect(screen.getByRole("complementary", {name:"Conversation navigation"})).toBeInTheDocument();
    expect(screen.queryByRole("button", {name:/theme accent/i})).not.toBeInTheDocument();
    expect(screen.getByRole("link", {name:"Back to conversation"})).toHaveAttribute("href", "/session/sess-kept");
  });

  it("retains the selected conversation across a knowledge-page remount", async () => {
    window.history.replaceState({}, "", "/observatory?returnTo=%2Fsession%2Fsess-kept");
    const first = render(<App />);
    await screen.findByText("ObservatoryPage");
    first.unmount();
    render(<App />);
    await screen.findByText("ObservatoryPage");
    expect(screen.getByRole("link", {name:"Back to conversation"})).toHaveAttribute("href", "/session/sess-kept");
  });

  it.each(["https://example.com", "//example.com", "/session/sess-kept/../../settings", "/session/sess-kept?token=secret"])("rejects an unsafe return destination %s", async target => {
    window.history.replaceState({}, "", `/memory?returnTo=${encodeURIComponent(target)}`);
    render(<App />);
    await screen.findByText("MemoryTab");
    expect(screen.getByRole("link", {name:"Back to conversation"})).toHaveAttribute("href", "/session");
  });
});

// jsdom doesn't implement matchMedia — provide a minimal stub for Sonner/Toaster
Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }),
});

beforeEach(() => {
  window.history.replaceState({}, "", "/");
  vi.mocked(initializeTransport).mockClear();
  health.mockReset();
  connect.mockReset();
  disconnect.mockReset();
  commissioningStatus.mockReset();
  commissioningStatus.mockResolvedValue({success:true,data:{state:"complete"}});
  createChatSession.mockReset();
  deleteChatSession.mockReset();
  health.mockResolvedValue({ success: true, data: { status: "ok", version: "1.0.0" } });
  connect.mockResolvedValue({ success: true });
  disconnect.mockResolvedValue(undefined);
});

describe("App — initialization flow", () => {
  it("shows loading spinner while initializing", () => {
    // Health never resolves — stay loading
    health.mockReturnValue(new Promise(() => {}));
    render(<App />);
    expect(screen.getByText(/connecting to gateway/i)).toBeInTheDocument();
  });

  it("renders the app after successful initialization", async () => {
    render(<App />);
    // Wait for initialization to complete (spinner disappears)
    await waitFor(() => {
      expect(screen.queryByText(/connecting to gateway/i)).toBeNull();
    });
    // Commissioning guard is mocked here; verify the app shell still starts.
    // Just check that the error state is NOT showing
    expect(screen.queryByText(/connection failed/i)).toBeNull();
  });

  it("does not duplicate initialization calls under React StrictMode", async () => {
    render(
      <StrictMode>
        <App />
      </StrictMode>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/connecting to gateway/i)).toBeNull();
    });

    expect(initializeTransport).toHaveBeenCalledTimes(1);
    expect(health).toHaveBeenCalledTimes(1);
    expect(connect).toHaveBeenCalledTimes(1);
    // The real commissioning guard resolves after gateway initialization.
    expect(await screen.findByText("v1.0.0")).toBeInTheDocument();
  });

  it("renders build date metadata from health in the version badge", async () => {
    health.mockResolvedValue({
      success: true,
      data: {
        status: "ok",
        version: "2026.5.3.develop",
        buildDate: "2026-06-14",
        buildTimestamp: "2026-06-14T15:30:00Z",
      },
    });

    render(<App />);

    const badge = await screen.findByText("v2026.5.3.develop (2026-06-14 15:30:00Z)");
    expect(badge).toHaveAttribute("title", "z-bot 2026.5.3.develop built 2026-06-14T15:30:00Z");
  });

  it("shows error state when health check fails", async () => {
    health.mockResolvedValue({ success: false, error: "daemon not running" });
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText(/connection failed/i)).toBeInTheDocument();
    });
    expect(screen.getByText(/daemon not running/i)).toBeInTheDocument();
  });

  it("shows error state when initialization throws", async () => {
    health.mockRejectedValue(new Error("socket error"));
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText(/connection failed/i)).toBeInTheDocument();
    });
    expect(screen.getByText(/socket error/i)).toBeInTheDocument();
  });

  it("retries connection when Retry button is clicked", async () => {
    health.mockResolvedValueOnce({ success: false, error: "first attempt failed" });
    health.mockResolvedValueOnce({ success: true, data: { status: "ok" } });

    render(<App />);
    await waitFor(() => screen.getByText(/connection failed/i));

    fireEvent.click(screen.getByRole("button", { name: /retry connection/i }));

    // Loading spinner appears again
    await waitFor(() => {
      expect(screen.queryByText(/connection failed/i)).toBeNull();
    });
    expect(health).toHaveBeenCalledTimes(2);
  }, 10000);
});
