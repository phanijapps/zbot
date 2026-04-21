import { expect } from "@playwright/test";
import { bootUIMode } from "../../lib/harness-ui";

const { test, handle } = bootUIMode({ fixture: "simple-qa" });

test.describe("regression: URL silent after New Research (Mode UI)", () => {
  test("clicking New Research after opening an existing session then sending must flip URL to new :sessionId", async ({ page, request }) => {
    await page.goto(handle.uiUrl("/research-v2/sess-synthetic-simple-qa-0000"));
    await expect(page.locator(".research-msg--assistant").first())
      .toBeVisible({ timeout: 5000 });

    await page.getByRole("button", { name: /New research/i }).click();
    await expect.poll(() => page.url()).toMatch(/\/research-v2$/);

    await page.locator("textarea").fill("what is 2+2? one-line answer");
    await page.locator('button[title="Send message"]').click();

    await expect.poll(() => page.url(), { timeout: 2000 })
      .toMatch(/\/research-v2\/sess-/);

    await handle.assertZeroDrift(request);
  });
});
