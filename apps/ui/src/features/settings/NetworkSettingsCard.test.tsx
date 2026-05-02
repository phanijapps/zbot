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

type NetworkInfo = {
  exposeToLan: boolean;
  bindHost: string;
  port: number;
  hostnameUrls: string[];
  ipUrls: string[];
  mdns: {
    active: boolean;
    interfaces: string[];
    aliasClaimed: boolean;
    instanceId: string;
  };
};

function mockNetworkInfoOn(overrides: Partial<NetworkInfo> = {}) {
  const base: NetworkInfo = {
    exposeToLan: true,
    bindHost: "0.0.0.0",
    port: 18791,
    hostnameUrls: ["http://agentzero.local", "http://phani-mbp-agentzero.local"],
    ipUrls: ["http://192.168.1.42:18791"],
    mdns: {
      active: true,
      interfaces: ["en0"],
      aliasClaimed: true,
      instanceId: "uuid",
    },
    ...overrides,
  };
  globalThis.fetch = vi.fn(async (url: RequestInfo | URL) => {
    const u = url.toString();
    if (u.endsWith("/api/network/info")) {
      return new Response(JSON.stringify({ success: true, data: base }), {
        status: 200,
      });
    }
    return new Response("not mocked", { status: 404 });
  }) as unknown as typeof fetch;
}

describe("NetworkSettingsCard — on state", () => {
  it("renders all URLs and a QR code", async () => {
    mockNetworkInfoOn();
    render(<NetworkSettingsCard />);
    await waitFor(() => {
      expect(screen.getByText("http://agentzero.local")).toBeInTheDocument();
    });
    expect(screen.getByText("http://phani-mbp-agentzero.local")).toBeInTheDocument();
    expect(screen.getByText("http://192.168.1.42:18791")).toBeInTheDocument();
    expect(screen.getByTestId("network-qr")).toBeInTheDocument();
  });

  it("renders alias collision note when aliasClaimed is false", async () => {
    mockNetworkInfoOn({
      hostnameUrls: ["http://phani-mbp-agentzero.local"],
      mdns: {
        active: true,
        interfaces: ["en0"],
        aliasClaimed: false,
        instanceId: "uuid",
      },
    });
    render(<NetworkSettingsCard />);
    await waitFor(() => {
      expect(screen.getByText(/already in use on this network/i)).toBeInTheDocument();
    });
  });

  it("renders mdns failure warning when active=false but exposeToLan=true", async () => {
    mockNetworkInfoOn({
      mdns: {
        active: false,
        interfaces: [],
        aliasClaimed: false,
        instanceId: "uuid",
      },
    });
    render(<NetworkSettingsCard />);
    await waitFor(() => {
      expect(screen.getByText(/mDNS responder failed to start/i)).toBeInTheDocument();
    });
    expect(screen.getByText("http://192.168.1.42:18791")).toBeInTheDocument();
  });
});
