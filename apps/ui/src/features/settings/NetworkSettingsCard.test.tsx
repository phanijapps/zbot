import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { NetworkSettingsCard } from "./NetworkSettingsCard";

beforeEach(() => {
  globalThis.fetch = vi.fn(async (url: RequestInfo | URL) => {
    const u = url.toString();
    if (u.endsWith("/api/network/info")) {
      return new Response(
        JSON.stringify({
          success: true,
          data: {
            exposeToLan: false,
            bindHost: "127.0.0.1",
            port: 18791,
            hostnameUrls: [],
            ipUrls: [],
            mdns: {
              active: false,
              interfaces: [],
              aliasClaimed: false,
              instanceId: "00000000-0000-0000-0000-000000000000",
            },
          },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      );
    }
    if (u.endsWith("/api/settings/network")) {
      return new Response(
        JSON.stringify({
          success: true,
          data: { exposeToLan: false, discovery: {}, advanced: { httpPort: 18791 } },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      );
    }
    return new Response("not mocked", { status: 404 });
  }) as unknown as typeof fetch;
});

describe("NetworkSettingsCard — off state", () => {
  it("renders the off-state copy when exposeToLan is false", async () => {
    render(<NetworkSettingsCard />);
    await waitFor(() => {
      expect(screen.getByText(/LAN exposure is off/i)).toBeInTheDocument();
    });
    expect(screen.queryByText(/agentzero\.local/i)).not.toBeInTheDocument();
  });
});
