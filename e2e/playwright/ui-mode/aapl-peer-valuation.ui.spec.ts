import { expect } from "@playwright/test";
import { bootUIMode } from "../lib/harness-ui";

const { test, handle } = bootUIMode({ fixture: "aapl-peer-valuation" });

test.describe("aapl-peer-valuation (Mode UI)", () => {
  test("renders the recorded analysis — title, root respond verdict, subagent count", async ({ page, request }) => {
    await page.goto(handle.uiUrl("/research-v2"));

    await page.locator("textarea").fill("Run an AAPL peer valuation.");
    await page.locator('button[title="Send message"]').click();

    await expect.poll(() => page.url(), { timeout: 5000 })
      .toMatch(/\/research-v2\/sess-f0e9b78c/);

    await expect(page.locator("h1, h2, header, [class*='title']").first())
      .toContainText(/AAPL/i, { timeout: 10_000 });

    await expect(page.locator(".research-msg--assistant").first())
      .toContainText(/OVERVALUED/i, { timeout: 10_000 });

    await handle.assertZeroDrift(request);
  });
});
