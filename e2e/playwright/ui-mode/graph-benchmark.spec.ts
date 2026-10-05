import { expect, test } from "@playwright/test";
import { spawn, type ChildProcess } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";

// Graph renderer benchmark (spec AC5): loads the dev-only benchmark route on
// the Vite dev server, waits for both renderer runs to complete, and persists
// the measured numbers to docs/specs/observatory-graph-completeness/benchmark-runs.md.
// Generated synthetic scene only (17,000 entities / 5,000 edges), seeded.
let vite: ChildProcess | undefined;

test.beforeAll(async () => {
  vite = spawn("npm", ["run", "dev", "--", "--port", "3210", "--strictPort"], {
    cwd: path.join(__dirname, "..", "..", "..", "apps", "ui"),
    stdio: "ignore",
    detached: false,
  });
  // Wait for the dev server to accept connections.
  const deadline = Date.now() + 60_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch("http://127.0.0.1:3210/");
      if (response.ok) return;
    } catch {
      // not up yet
    }
    await new Promise(resolve => setTimeout(resolve, 500));
  }
  throw new Error("vite dev server did not start");
});

test.afterAll(async () => {
  vite?.kill("SIGTERM");
});

test("benchmark d3 vs cosmos.gl and persist measurements", async ({page}) => {
  test.setTimeout(300_000);
  await page.goto("http://127.0.0.1:3210/observatory/benchmark");
  await page.setViewportSize({width: 1280, height: 900});
  const results = page.locator("#benchmark-results");
  await expect(results).toContainText('"renderer": "cosmos.gl', {timeout: 240_000});
  const payload = JSON.parse((await results.textContent()) ?? "{}") as {
    scene: { entities: number; edges: number };
    results: Array<{
      renderer: string;
      firstUsableViewMs: number;
      inputFeedbackP95Ms: number;
      peakHeapMiB: number;
      mountCycles: { beforeMiB: number; afterMiB: number };
      error?: string;
    }>;
  };

  const host = os.hostname();
  const cpus = os.cpus()[0]?.model ?? "unknown";
  const report = [
    "# Graph renderer benchmark runs (raw measurements; verdict lives in benchmark.md)",
    "",
    `Scene: ${payload.scene.entities.toLocaleString()} entities / ${payload.scene.edges.toLocaleString()} edges (seeded synthetic; generated content only).`,
    `Host: ${host}; CPU: ${cpus}; Browser: desktop Chromium (Playwright); ${new Date().toISOString()}.`,
    "",
    "Budgets (AC5): first usable view ≤ 5000 ms; input→visible p95 ≤ 150 ms across 30 pan/zoom interactions after warmup; peak JS heap ≤ 256 MiB; no monotonic growth over ten mount/unmount cycles.",
    "",
    "| Renderer | First usable view (ms) | Input p95 (ms) | Peak heap (MiB) | Mount cycles before→after (MiB) |",
    "| --- | --- | --- | --- | --- |",
    ...payload.results.map(
      (r) =>
        `| ${r.renderer} | ${r.firstUsableViewMs} | ${r.inputFeedbackP95Ms} | ${r.peakHeapMiB} | ${r.mountCycles.beforeMiB} → ${r.mountCycles.afterMiB} |`
    ),
    "",
    ...payload.results.flatMap((r) =>
      r.error ? [`> ${r.renderer} error: ${r.error}`] : []
    ),
    "",
    "D3 is the benchmark baseline; cosmos.gl is the ratified production renderer (see benchmark.md for the verdict). This file records raw runs.",
    "",
  ].join("\n");

  const destination = path.join(
    __dirname,
    "..",
    "..",
    "..",
    "docs",
    "specs",
    "observatory-graph-completeness",
    "benchmark-runs.md"
  );
  fs.mkdirSync(path.dirname(destination), {recursive: true});
  fs.writeFileSync(destination, report);
});
