import { expect } from "@playwright/test";
import { bootFullMode } from "../lib/harness-full";
import * as fs from "node:fs";

// Rendered-HTML capture for offline a11y/standard validation of /session
// and /session/:id (backlog: pre-existing-session-shell-html-validation).
const { test, handle } = bootFullMode({fixture: "session-shell", freshVault: true, sameOrigin: true, localOnly: true});

test("dump rendered session HTML", async ({page}) => {
  test.setTimeout(120_000);
  await page.setViewportSize({width: 1280, height: 800});
  await page.goto(handle.uiUrl("/session"));
  await page.locator("textarea").first().waitFor({timeout: 30_000});
  fs.mkdirSync("test-results/html", {recursive: true});
  fs.writeFileSync("test-results/html/session.html", await page.content());
  await page.locator("textarea").first().fill("What is 2 + 2? One-line answer.");
  await page.locator('button[title="Send message"]').click();
  await page.locator(".quick-chat__assistant").last().waitFor({timeout: 30_000});
  fs.writeFileSync("test-results/html/session-populated.html", await page.content());
});
