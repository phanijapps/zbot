import { expect } from "@playwright/test";
import { bootFullMode } from "../lib/harness-full";

// Visual + DOM capture of the completed inspector tabs through the real
// loopback daemon + served UI (fresh seeded vault, local-only). Generated
// content only; asserts the real panels render behind each tab.
const { test, handle } = bootFullMode({fixture: "session-shell", freshVault: true, sameOrigin: true, localOnly: true});

test("capture Activity, Sources and Files tabs", async ({page}) => {
  test.setTimeout(120_000);
  await page.setViewportSize({width: 1280, height: 800});
  await page.goto(handle.uiUrl("/session"));
  await page.locator("textarea").first().fill("What is 2 + 2? One-line answer.");
  await page.locator('button[title="Send message"]').click();
  await page.locator(".quick-chat__assistant").last().waitFor({timeout: 30_000});
  await expect(page.getByRole("region", {name: "Recorded activity"})).toBeVisible();
  await page.screenshot({path: "test-results/session-tabs-activity.png", fullPage: false});
  await page.getByRole("tab", {name: "Sources"}).click();
  await expect(page.getByRole("region", {name: "Recorded sources"})).toBeVisible();
  await expect(page.getByRole("tabpanel")).not.toContainText("will be available in the next session update");
  await page.screenshot({path: "test-results/session-tabs-sources.png", fullPage: false});
  await page.getByRole("tab", {name: "Files"}).click();
  await expect(page.getByRole("region", {name: "Session files"})).toBeVisible();
  await expect(page.getByRole("tabpanel")).not.toContainText("will be available in the next session update");
  await page.screenshot({path: "test-results/session-tabs-files.png", fullPage: false});
});
