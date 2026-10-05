import { expect } from "@playwright/test";
import { bootFullMode } from "../lib/harness-full";

// Real-startup proof of the effective default bind: fresh seeded vault, NO
// --local-only flag — the daemon must come up loopback by default (original
// shell AC13 default-startup half; settings-backed resolution, not the CLI
// arg, decides the effective bind).
const { test, handle } = bootFullMode({fixture: "session-shell", freshVault: true, sameOrigin: true});

test("fresh vault daemon binds loopback by default", async ({request}) => {
  test.setTimeout(60_000);
  const response = await request.get(handle.gatewayUrl("/api/network/info"));
  expect(response.ok()).toBeTruthy();
  const body = await response.json();
  expect(body.success).toBe(true);
  expect(body.data.bindHost).toBe("127.0.0.1");
  expect(body.data.exposeToLan).toBe(false);
});
