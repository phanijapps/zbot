import { expect } from "@playwright/test";
import { bootUIMode } from "../lib/harness-ui";

const { test, handle } = bootUIMode({ fixture: "simple-qa" });

test.describe("simple-qa (Mode UI)", () => {
  test("opens /research-v2, sends prompt, sees respond + completed state", async ({ page, request }) => {
    await page.goto(handle.uiUrl("/research-v2"));
    await expect(page.getByRole("textbox", { name: /message/i })).toBeVisible();

    const ta = page.locator("textarea");
    await ta.fill("what is 2+2? one-line answer");
    await page.locator('button[title="Send message"]').click();

    await expect.poll(() => page.url(), { timeout: 2000 })
      .toMatch(/\/research-v2\/sess-/);

    await expect(page.locator(".research-msg--assistant").first())
      .toContainText(/4/, { timeout: 5000 });

    await handle.assertZeroDrift(request);
  });
});
