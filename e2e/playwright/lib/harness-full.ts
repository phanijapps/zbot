import { spawnSync } from "node:child_process";
import { test as baseTest, expect } from "@playwright/test";
import type { APIRequestContext } from "@playwright/test";

interface BootFullSummary {
  run_dir: string;
  mock_llm_url: string;
  gateway_http_url: string;
  gateway_ws_url: string;
  ui_url: string;
  vault: string;
  fixture: string;
}

export interface FullHarnessHandle {
  uiUrl(path: string): string;
  gatewayUrl(path: string): string;
  mockLlmUrl(path: string): string;
  assertZeroDrift(request: APIRequestContext): Promise<void>;
}

export function bootFullMode(opts: {
  fixture: string;
}): { handle: FullHarnessHandle; test: typeof baseTest } {
  let summary: BootFullSummary;
  let teardownCalled = false;
  const test = baseTest.extend<{}>({});

  test.beforeAll(async () => {
    const result = spawnSync(
      "bash",
      ["../scripts/boot-full-mode.sh", opts.fixture],
      { encoding: "utf-8", timeout: 120_000 },
    );
    if (result.status !== 0) {
      throw new Error(
        `boot-full-mode.sh failed (exit ${result.status}):\n` +
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

  const handle: FullHarnessHandle = {
    uiUrl(path: string) {
      const url = new URL(path, summary.ui_url);
      url.searchParams.set("gateway_http", summary.gateway_http_url);
      url.searchParams.set("gateway_ws", summary.gateway_ws_url);
      return url.toString();
    },
    gatewayUrl(path: string) {
      return new URL(path, summary.gateway_http_url).toString();
    },
    mockLlmUrl(path: string) {
      return new URL(path, summary.mock_llm_url).toString();
    },
    async assertZeroDrift(request: APIRequestContext) {
      const r = await request.get(
        new URL("/__replay/status", summary.mock_llm_url).toString(),
      );
      expect(r.ok()).toBeTruthy();
      const body = await r.json();
      expect(body.drift_count).toBe(0);
    },
  };

  return { handle, test };
}
