import { expect, test as baseTest } from "@playwright/test";
import type { APIRequestContext, Page } from "@playwright/test";
import { chmodSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { execFileSync } from "node:child_process";
import { bootFullMode } from "../lib/harness-full";

interface HookEvent {
  event: string;
  event_id: string;
  invocation_id: string;
  session_id: string;
  agent_id: string;
  run_id: string | null;
  data: Record<string, unknown>;
}

baseTest.describe("external hook Stop (real daemon)", () => {
  const {test, handle} = bootFullMode({fixture: "simple-qa", freshVault: true, sameOrigin: true, localOnly: true});
  let output: string;
  let pids: string;
  test.beforeAll(() => {
    output = installPrograms(handle.dataDir());
    pids = join(handle.dataDir(), "config", "hooks", "pids.json");
    const script = join(handle.dataDir(), "config", "hooks", "hang.py");
    writeFileSync(script, `import json, os, sys, time
event = json.load(sys.stdin)
if event['event'] == 'before_model':
    child = os.fork()
    if child == 0:
        time.sleep(60)
        os._exit(0)
    with open(sys.argv[1], 'w') as output:
        json.dump([os.getpid(), child], output)
    time.sleep(60)
else:
    with open(sys.argv[1]) as source:
        pids = json.load(source)
    assert all(not os.path.exists('/proc/' + str(pid)) for pid in pids)
    time.sleep(0.25)
print('{"version":1,"action":"continue"}')
`, {mode: 0o600});
    const config = join(handle.dataDir(), "config", "hooks.json");
    const value = JSON.parse(readFileSync(config, "utf8"));
    value.hooks.push({id: "hang-before-model", event: "before_model", command: ["python3", script, pids], timeout_ms: 60_000});
    value.hooks.push({id: "verify-cleanup", event: "run_end", command: ["python3", script, pids]});
    writeFileSync(config, JSON.stringify(value), {mode: 0o600});
  });
  test.afterEach(async ({}, info) => {
    for (const path of [output, pids, join(dirname(handle.dataDir()), "zerod.log")]) {
      if (existsSync(path)) await info.attach(path.endsWith(".log") ? "daemon-log" : path.endsWith("pids.json") ? "process-ids" : "fixture-events", {path, contentType: "text/plain"});
    }
  });
  test("Stop reaps the command and descendant before observers and settles the same Activity row", async ({page, request}, info) => {
    const frames: {direction: string; payload: string}[] = [];
    page.on("websocket", socket => {
      socket.on("framesent", frame => frames.push({direction: "sent", payload: frame.payload.toString()}));
      socket.on("framereceived", frame => frames.push({direction: "received", payload: frame.payload.toString()}));
    });
    await page.setViewportSize({width: 1440, height: 900});
    await page.goto(handle.uiUrl("/session"));
    await expect(page.locator("textarea")).toBeEnabled();
    await page.locator("textarea").fill("Wait for the hook so I can stop this run.");
    await page.locator('button[title="Send message"]').click();
    await expect.poll(() => existsSync(pids), {timeout: 20_000}).toBeTruthy();
    const processes: number[] = JSON.parse(readFileSync(pids, "utf8"));
    expect(processes.every(pid => existsSync(`/proc/${pid}`))).toBeTruthy();
    const session = observations(output)[0].event.session_id;
    const url = handle.gatewayUrl(`/api/sessions/${session}/details`);
    const running = (await details(request, url)).find(row => row.hook?.hookId === "hang-before-model")!;
    expect(running.hook?.status).toBe("running");
    await page.getByRole("button", {name: "Stop chat", exact: true}).click();
    try {
      await expect.poll(() => processes.every(pid => !existsSync(`/proc/${pid}`))).toBeTruthy();
    } finally {
      await info.attach("stop-ws-and-running-row", {body: JSON.stringify({running, frames, processes: processes.map(pid => ({pid, present: existsSync(`/proc/${pid}`)}))}, null, 2), contentType: "application/json"});
    }
    await expect.poll(async () => (await details(request, url)).find(row => row.hook?.hookId === "verify-cleanup")?.hook?.status).toBe("completed");
    const settled = await details(request, url);
    const cancelled = settled.find(row => row.id === running.id)!;
    expect(cancelled.hook?.status).toBe("cancelled");
    expect(cancelled.occurredAt).toBe(running.occurredAt);
    expect(cancelled.sequence).toBe(running.sequence);
    expect(settled.some(row => row.hook?.status === "running")).toBeFalsy();
    await expect(page.locator(".session-shell__details").getByText("Hook cancelled", {exact: true})).toHaveCount(1);
    await page.goto(handle.uiUrl(`/session/${session}`));
    await expect(page.locator(".session-shell__details").getByText("Hook cancelled", {exact: true})).toHaveCount(1);
    expect(await details(request, url)).toEqual(settled);
    await handle.assertZeroDrift(request);
  });
});
interface Observation {
  event: HookEvent;
  kind: string;
  literal: string;
  environment: Record<string, string>;
}
interface HookRow {
  id: string;
  sequence: number;
  occurredAt: string;
  kind: string;
  label: string;
  hook?: {event: string; hookId: string; agentId: string; status: string};
}

// These programs belong exclusively to the fresh test vault, outside any ward
// or source checkout. Recording raw events here is fixture evidence; production
// Activity is required to contain only the bounded public metadata.
function installPrograms(vault: string): string {
  const directory = join(vault, "config", "hooks");
  mkdirSync(directory, {recursive: true, mode: 0o700});
  const output = join(directory, "observed.jsonl");
  const literal = "literal ; $(never-execute) & value";
  const python = join(directory, "observe.py");
  writeFileSync(python, `import json, os, sys
event = json.load(sys.stdin)
with open(sys.argv[1], 'a') as output:
    output.write(json.dumps({'event': event, 'kind': 'python', 'literal': sys.argv[2], 'environment': dict(os.environ)}) + '\\n')
print(json.dumps({'version': 1, 'action': 'continue', 'reason': 'PRIVATE-HOOK-REASON'}))
print('PRIVATE-HOOK-STDERR', file=sys.stderr)
`, {mode: 0o600});
  const node = join(directory, "observe.mjs");
  writeFileSync(node, `import fs from 'node:fs';
const event = JSON.parse(fs.readFileSync(0, 'utf8'));
fs.appendFileSync(process.argv[2], JSON.stringify({event, kind: 'node', literal: process.argv[3], environment: process.env}) + '\\n');
const response = {version: 1, action: 'continue'};
if (event.event === 'before_model') response.context = 'PRIVATE-HOOK-CONTEXT';
console.log(JSON.stringify(response));
`, {mode: 0o600});
  const source = join(directory, "observe.go");
  const native = join(directory, "observe-native");
  writeFileSync(source, `package main
import ("encoding/json"; "fmt"; "os"; "strings")
func main() {
  var event any
  if err := json.NewDecoder(os.Stdin).Decode(&event); err != nil { panic(err) }
  environment := map[string]string{}
  for _, value := range os.Environ() { pair := strings.SplitN(value, "=", 2); environment[pair[0]] = pair[1] }
  output, err := os.OpenFile(os.Args[1], os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0600)
  if err != nil { panic(err) }
  if err := json.NewEncoder(output).Encode(map[string]any{"event": event, "kind": "native", "literal": os.Args[2], "environment": environment}); err != nil { panic(err) }
  output.Close()
  fmt.Println("{\\\"version\\\":1,\\\"action\\\":\\\"continue\\\"}")
}
`, {mode: 0o600});
  execFileSync("go", ["build", "-o", native, source], {timeout: 60_000});
  chmodSync(native, 0o700);
  const commands = {python: ["python3", python, output, literal], node: ["node", node, output, literal], native: [native, output, literal]};
  const mapping = [
    ["python", "session_start"], ["python", "user_prompt"],
    ["native", "run_start"], ["native", "run_end"],
    ["node", "before_model"], ["node", "after_model"],
    ["node", "before_tool"], ["node", "after_tool"],
  ] as const;
  writeFileSync(join(vault, "config", "hooks.json"), JSON.stringify({version: 1, hooks: mapping.map(([kind, event]) => ({
    id: `${kind}-${event.replaceAll("_", "-")}`, event, command: commands[kind],
  }))}), {mode: 0o600});
  chmodSync(join(vault, "config", "hooks.json"), 0o600);
  return output;
}
function observations(path: string): Observation[] {
  try { return readFileSync(path, "utf8").trim().split("\n").filter(Boolean).map(line => JSON.parse(line)); }
  catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") return []; throw error; }
}
async function details(request: APIRequestContext, url: string): Promise<HookRow[]> {
  const response = await request.get(url);
  expect(response.ok()).toBeTruthy();
  return (await response.json()).activity.filter((row: HookRow) => row.kind === "hook");
}
async function captureActivity(page: Page, name: string) {
  const directory = process.env.ZBOT_HOOKS_QA_DIR;
  if (!directory) return;
  mkdirSync(directory, {recursive: true});
  const captures = [];
  for (const width of [759, 760, 1100]) for (const height of [600, 900]) {
    await page.setViewportSize({width, height});
    const toggle = page.getByRole("button", {name: "Toggle session details"});
    if (await toggle.isVisible() && await toggle.getAttribute("aria-expanded") !== "true") await toggle.click();
    await expect(page.locator(".session-shell__details")).toBeVisible();
    await page.evaluate(() => window.scrollTo(0, 0));
    const geometry = await page.evaluate(() => ({
      scrollY: window.scrollY,
      scrollMax: document.documentElement.scrollHeight - window.innerHeight,
      horizontalOverflow: document.documentElement.scrollWidth > window.innerWidth,
    }));
    const screenshot = `${name}-${width}x${height}-rest.png`;
    await page.screenshot({path: join(directory, screenshot)});
    captures.push({route: new URL(page.url()).pathname, width, height, screenshot, ...geometry, pageScrollable: geometry.scrollMax > 0});
    expect(geometry.horizontalOverflow).toBeFalsy();
    if (geometry.scrollMax > 0) {
      await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
      const scrolled = `${name}-${width}x${height}-scrolled.png`;
      await page.screenshot({path: join(directory, scrolled)});
      captures.push({...captures[captures.length - 1], screenshot: scrolled, scrollY: await page.evaluate(() => window.scrollY)});
    }
  }
  writeFileSync(join(directory, `${name}-captures.json`), JSON.stringify(captures, null, 2));
  const shell = await page.locator(".session-shell").evaluate(element => element.outerHTML);
  writeFileSync(join(directory, `${name}-rendered.html`), `<!doctype html><html lang="en"><head><title>Hook Activity</title></head><body>${shell}</body></html>`);
  await page.setViewportSize({width: 1440, height: 900});
  await page.keyboard.press("Tab");
  const summary = page.locator(".session-activity summary").first();
  await summary.focus();
  await page.keyboard.press("Enter");
  await expect(page.locator(".session-activity details").first()).toHaveAttribute("open", "");
  const focus = await summary.evaluate(element => {
    const rect = element.getBoundingClientRect();
    const style = getComputedStyle(element);
    return {width: rect.width, height: rect.height, outlineWidth: style.outlineWidth, outlineColor: style.outlineColor, outlineOffset: style.outlineOffset};
  });
  expect(focus.width).toBeGreaterThanOrEqual(24);
  expect(focus.height).toBeGreaterThanOrEqual(24);
  expect(parseFloat(focus.outlineWidth)).toBeGreaterThanOrEqual(2);
  writeFileSync(join(directory, `${name}-focus.json`), JSON.stringify(focus, null, 2));
  await page.screenshot({path: join(directory, `${name}-keyboard-expanded.png`)});
  if (process.env.ZBOT_HOOKS_AXE_SCRIPT) {
    await page.addScriptTag({path: process.env.ZBOT_HOOKS_AXE_SCRIPT});
    const result = await page.evaluate(async () => {
      const axe = (window as unknown as {axe: {run: (selector: string, options: object) => Promise<{violations: unknown[]}>}}).axe;
      return await axe.run(".session-activity", {runOnly: {type: "tag", values: ["wcag2a", "wcag2aa", "wcag21aa"]}});
    });
    writeFileSync(join(directory, `${name}-axe.json`), JSON.stringify(result, null, 2));
    expect(result.violations).toEqual([]);
  }
}

for (const mode of ["chat", "research"] as const) {
  baseTest.describe(`external hooks ${mode} (real daemon)`, () => {
    const {test, handle} = bootFullMode({fixture: mode === "chat" ? "simple-qa" : "external-hooks", freshVault: true, sameOrigin: true, localOnly: true});
    let output: string;
    test.beforeAll(() => { output = installPrograms(handle.dataDir()); });
    test("file commands reach actual runs and bounded Activity survives reload", async ({page, request}) => {
      await page.setViewportSize({width: 1440, height: 900});
      await page.goto(handle.uiUrl(mode === "chat" ? "/session" : "/session?mode=research"));
      await expect(page.locator("textarea")).toBeEnabled();
      // Merely bootstrapping and opening the page must execute no hook.
      expect(observations(output)).toHaveLength(0);
      await page.locator("textarea").fill(mode === "chat" ? "what is 2+2? one-line answer" : "Delegate a short proof to builder-agent, then report completion.");
      await page.locator('button[title="Send message"]').click();
      await expect.poll(() => observations(output).filter(row => row.event.event === "run_end").length, {timeout: 30_000}).toBe(mode === "chat" ? 1 : 3);
      const observed = observations(output);
      const session = observed[0].event.session_id;
      const expectedCount = mode === "chat" ? 8 : 20;
      expect(observed).toHaveLength(expectedCount);
      expect(new Set(observed.map(row => row.event.event_id)).size).toBe(expectedCount);
      expect(new Set(observed.map(row => row.event.invocation_id)).size).toBe(1);
      expect(observed.filter(row => row.event.event === "session_start")).toHaveLength(1);
      expect(observed.filter(row => row.event.event === "user_prompt")).toHaveLength(1);
      expect(observed.filter(row => row.event.event === "run_start")).toHaveLength(mode === "chat" ? 1 : 3);
      expect(observed.every(row => row.event.session_id === session)).toBeTruthy();
      for (const row of observed) {
        expect(row.literal).toBe("literal ; $(never-execute) & value");
        expect(row.environment).not.toHaveProperty("HOME");
        expect(JSON.stringify(row.environment)).not.toContain("sk-mock");
      }
      expect(new Set(observed.map(row => row.kind))).toEqual(new Set(["python", "node", "native"]));
      if (mode === "research") {
        expect(observed.some(row => row.event.agent_id === "builder-agent")).toBeTruthy();
        expect(new Set(observed.filter(row => row.event.event === "run_start").map(row => row.event.run_id)).size).toBe(3);
        await expect(page.locator(".research-msg--assistant").last()).toContainText("Delegated hook proof complete.");
      } else {
        await expect(page.locator(".quick-chat__assistant").first()).toContainText("4");
        // The default shell learns QuickChat's confirmed ID without a route
        // change or second bootstrap, and displays the real Activity immediately.
        expect(new URL(page.url()).pathname).toBe("/session");
      }
      const url = handle.gatewayUrl(`/api/sessions/${session}/details`);
      await expect.poll(async () => (await details(request, url)).filter(row => row.hook?.status === "completed").length).toBe(expectedCount);
      const before = await details(request, url);
      expect(JSON.stringify(before)).not.toMatch(/PRIVATE-HOOK|sk-mock|observe\.py|observe\.mjs|observe-native/);
      await expect(page.locator(".session-shell__details").getByText("Hook completed", {exact: true})).toHaveCount(expectedCount);
      if (mode === "chat") await captureActivity(page, "default-chat");
      await page.goto(handle.uiUrl(`/session/${session}`));
      await expect(page.locator(".session-shell__details").getByText("Hook completed", {exact: true})).toHaveCount(expectedCount);
      if (mode === "chat") await captureActivity(page, "selected-chat");
      expect(await details(request, url)).toEqual(before);
      expect(observations(output)).toHaveLength(expectedCount);
      await handle.assertZeroDrift(request);
    });
  });
}
