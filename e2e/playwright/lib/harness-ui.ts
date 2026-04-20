import { spawnSync } from "node:child_process";
import { test as baseTest, expect } from "@playwright/test";
import type { APIRequestContext } from "@playwright/test";

export interface UIHarnessHandle {
  uiUrl(path: string): string;
  mockGatewayUrl(path: string): string;
  assertZeroDrift(request: APIRequestContext): Promise<void>;
}

interface BootSummary {
  run_dir: string;
  mock_gateway_url: string;
  ui_url: string;
  fixture: string;
}

export function bootUIMode(opts: {
  fixture: string;
}): { handle: UIHarnessHandle; test: typeof baseTest } {
  let summary: BootSummary;
  let teardownCalled = false;

  const test = baseTest.extend<{}>({});

  test.beforeAll(async () => {
    const result = spawnSync(
      "bash",
      ["../scripts/boot-ui-mode.sh", opts.fixture],
      { encoding: "utf-8", timeout: 30_000 },
    );
    if (result.status !== 0) {
      throw new Error(
        `boot-ui-mode.sh failed (exit ${result.status}):\n` +
        `stdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
      );
    }
    summary = JSON.parse(result.stdout.trim().split("\n").pop() ?? "{}");
  });

  test.afterAll(async () => {
    if (summary && !teardownCalled) {
      teardownCalled = true;
      spawnSync("bash", ["../scripts/teardown.sh", summary.run_dir], {
        timeout: 10_000,
      });
    }
  });

  const handle: UIHarnessHandle = {
    uiUrl(path: string) {
      // UI reads the gateway from ?gateway_http= + ?gateway_ws= query params
      // (apps/ui/src/services/transport/index.ts:102-103). Append them to
      // every navigation so the UI talks to mock-gateway, not localhost:18791.
      const url = new URL(path, summary.ui_url);
      const httpBase = summary.mock_gateway_url;
      const wsBase = httpBase.replace(/^http/, "ws");
      url.searchParams.set("gateway_http", httpBase);
      url.searchParams.set("gateway_ws", wsBase);
      return url.toString();
    },
    mockGatewayUrl(path: string) {
      return new URL(path, summary.mock_gateway_url).toString();
    },
    async assertZeroDrift(request: APIRequestContext) {
      const r = await request.get(
        new URL("/__replay/status", summary.mock_gateway_url).toString(),
      );
      expect(r.ok()).toBeTruthy();
      const body = await r.json();
      expect(body).toHaveProperty("consumed");
    },
  };

  return { handle, test };
}
