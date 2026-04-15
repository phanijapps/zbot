// ============================================================================
// HttpTransport — ward content + unified hybrid search tests
// ============================================================================

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { HttpTransport } from "../http";
import type { HybridSearchRequest } from "../types";

const HTTP_URL = "http://localhost:3000";

function okJson(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

describe("HttpTransport.getWardContent", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("hits GET /api/wards/:ward_id/content and returns counts", async () => {
    const fetchMock = vi.fn<typeof fetch>(async () =>
      okJson({
        ward_id: "wardA",
        summary: { title: "wardA" },
        facts: [],
        wiki: [],
        procedures: [],
        episodes: [],
        counts: { facts: 0, wiki: 0, procedures: 0, episodes: 0 },
      })
    );
    globalThis.fetch = fetchMock as unknown as typeof fetch;

    const t = new HttpTransport();
    await t.initialize({ httpUrl: HTTP_URL, wsUrl: "ws://localhost:3000" });

    const r = await t.getWardContent("wardA");
    expect(r.success).toBe(true);
    expect(r.data?.ward_id).toBe("wardA");
    expect(r.data?.counts.facts).toBe(0);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe(`${HTTP_URL}/api/wards/wardA/content`);
    expect((init as RequestInit | undefined)?.method).toBe("GET");
  });

  it("URL-encodes ward ids with special characters", async () => {
    const fetchMock = vi.fn<typeof fetch>(async () =>
      okJson({
        ward_id: "a/b",
        summary: { title: "a/b" },
        facts: [],
        wiki: [],
        procedures: [],
        episodes: [],
        counts: { facts: 0, wiki: 0, procedures: 0, episodes: 0 },
      })
    );
    globalThis.fetch = fetchMock as unknown as typeof fetch;

    const t = new HttpTransport();
    await t.initialize({ httpUrl: HTTP_URL, wsUrl: "ws://localhost:3000" });
    await t.getWardContent("a/b");
    expect(fetchMock.mock.calls[0][0]).toBe(`${HTTP_URL}/api/wards/a%2Fb/content`);
  });

  it("returns an error on non-2xx responses", async () => {
    globalThis.fetch = vi.fn(
      async () => new Response("nope", { status: 500, statusText: "Internal Server Error" })
    ) as unknown as typeof fetch;

    const t = new HttpTransport();
    await t.initialize({ httpUrl: HTTP_URL, wsUrl: "ws://localhost:3000" });

    const r = await t.getWardContent("wardX");
    expect(r.success).toBe(false);
    expect(r.error).toMatch(/HTTP 500/);
  });
});

describe("HttpTransport.searchMemoryHybrid", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("POSTs /api/memory/search with the request body and parses the unified response", async () => {
    const responseBody = {
      facts: { hits: [], latency_ms: 1 },
      wiki: { hits: [], latency_ms: 2 },
      procedures: { hits: [], latency_ms: 3 },
      episodes: { hits: [], latency_ms: 4 },
    };
    const fetchMock = vi.fn<typeof fetch>(async () => okJson(responseBody));
    globalThis.fetch = fetchMock as unknown as typeof fetch;

    const t = new HttpTransport();
    await t.initialize({ httpUrl: HTTP_URL, wsUrl: "ws://localhost:3000" });

    const req: HybridSearchRequest = {
      query: "hello",
      mode: "hybrid",
      types: ["facts", "wiki"],
      ward_ids: ["wardA"],
      filters: { confidence_gte: 0.5 },
      limit: 10,
    };
    const r = await t.searchMemoryHybrid(req);

    expect(r.success).toBe(true);
    expect(r.data?.facts.latency_ms).toBe(1);
    expect(r.data?.wiki.latency_ms).toBe(2);
    expect(r.data?.procedures.latency_ms).toBe(3);
    expect(r.data?.episodes.latency_ms).toBe(4);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe(`${HTTP_URL}/api/memory/search`);
    const typedInit = init as RequestInit;
    expect(typedInit.method).toBe("POST");
    expect(
      (typedInit.headers as Record<string, string>)["Content-Type"]
    ).toBe("application/json");
    expect(JSON.parse(typedInit.body as string)).toEqual(req);
  });

  it("returns an error on non-2xx responses", async () => {
    globalThis.fetch = vi.fn(
      async () => new Response("bad", { status: 400, statusText: "Bad Request" })
    ) as unknown as typeof fetch;

    const t = new HttpTransport();
    await t.initialize({ httpUrl: HTTP_URL, wsUrl: "ws://localhost:3000" });

    const r = await t.searchMemoryHybrid({ query: "x" });
    expect(r.success).toBe(false);
    expect(r.error).toMatch(/HTTP 400/);
  });
});
