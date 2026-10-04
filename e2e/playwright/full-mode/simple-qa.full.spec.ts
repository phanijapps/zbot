import { expect, test as baseTest } from "@playwright/test";
import { bootFullMode } from "../lib/harness-full";

for (const mode of ["chat", "research"] as const) {
  baseTest.describe(`simple-qa ${mode} (Mode Full)`, () => {
    // Give each mode its own daemon, response queue and temporary vault.
    // Empty streamed text in simple-qa makes the respond argument authoritative.
    const base = mode === "chat" ? "/chat" : "/research";
    const { test, handle } = bootFullMode({
      fixture: "simple-qa",
      freshVault: true,
      sameOrigin: true,
      localOnly: true,
    });
    test("real daemon retains the respond-only answer after reload", async ({ page, request }) => {
      await page.goto(handle.uiUrl(base));
      await page.locator("textarea").fill("what is 2+2? one-line answer");
      await page.locator('button[title="Send message"]').click();
      if (mode === "research") {
        await expect.poll(() => page.url(), { timeout: 10_000 })
          .toMatch(/\/research\/sess-/);
      }
      const selector = mode === "chat" ? ".quick-chat__assistant" : ".research-msg--assistant";
      await expect(page.locator(selector).first()).toContainText("4", { timeout: 20_000 });
      await handle.assertZeroDrift(request);

      await page.goto(handle.uiUrl(new URL(page.url()).pathname));
      await expect(page.locator(selector)).toHaveCount(1, { timeout: 20_000 });
      await expect(page.locator(selector).first()).toContainText("4");
      await expect(page.getByText("LLM error", { exact: true })).toHaveCount(0);
      await handle.assertZeroDrift(request);
    });
  });
}
