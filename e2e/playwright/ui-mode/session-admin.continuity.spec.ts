import { expect } from "@playwright/test";
import { bootFullMode } from "../lib/harness-full";

// Administration continuity (completion AC5 / original AC11): Agents, Settings
// and Integrations render inside the shell, returning preserves the selected
// conversation, navigation never mutates sessions, and no hook-management
// control exists. Seeded fresh vault, loopback, zero-drift asserted.
const { test, handle } = bootFullMode({fixture: "session-shell", freshVault: true, sameOrigin: true, localOnly: true});

test("administration pages preserve the conversation and expose no hook management", async ({page, request}) => {
  test.setTimeout(180_000);
  await page.setViewportSize({width: 1280, height: 800});
  await page.goto(handle.uiUrl("/session"));
  await page.locator("textarea").first().fill("What is 2 + 2? One-line answer.");
  await page.locator('button[title="Send message"]').click();
  const answer = page.locator(".quick-chat__assistant").last();
  await answer.waitFor({timeout: 30_000});
  const finalText = await answer.innerText();

  for (const destination of ["Agents", "Settings", "Integrations"]) {
    await page.getByRole("link", {name: destination}).click();
    await expect(page.getByRole("link", {name: "Back to conversation"})).toBeVisible();
    // Navigation must never create or reset conversations.
    await expect(page.locator(".session-shell__alert")).toHaveCount(0);
    await page.getByRole("link", {name: "Back to conversation"}).click();
    await expect(page.locator("textarea").first()).toBeVisible();
    await expect(page.locator(".quick-chat__assistant").last()).toHaveText(finalText, {useInnerText: true});
  }

  // Hook management stays absent everywhere in the shell.
  await page.getByRole("link", {name: "Settings"}).click();
  await expect(page.getByText(/hook/i)).toHaveCount(0);

  await handle.assertZeroDrift(request);
});
