import { expect } from "@playwright/test";
import { bootFullMode } from "../lib/harness-full";

const { test, handle } = bootFullMode({ fixture: "aapl-peer-valuation" });

test.describe("aapl-peer-valuation (Mode Full)", () => {
  // Mode Full mismatch note: the fixture has 19 recorded LLM responses
  // captured from the original run, but a live zerod orchestrator makes
  // more LLM calls than that (one after every tool result). The 19
  // responses exhaust before the subagent chain finishes. We assert only
  // the signals that are deterministic in Mode Full: title update,
  // delegation dispatches, artifact hydration. Full multi-turn replay
  // needs either synthetic-response fallback in mock-llm or a richer
  // fixture captured against a tool-result-per-turn loop.
  test("real zerod delegates to planner-agent and renders recorded artifact", async ({ page }) => {
    await page.goto(handle.uiUrl("/research"));

    await page.locator("textarea").fill("Run an AAPL peer valuation.");
    await page.locator('button[title="Send message"]').click();

    await expect.poll(() => page.url(), { timeout: 15_000 })
      .toMatch(/\/research\/sess-/);

    // Title reflects the intent / recorded session title.
    // Target the research page title specifically — `header` matches the
    // global topbar which has no session content.
    await expect(page.locator("[class*='research-page__title']").first())
      .toContainText(/AAPL/i, { timeout: 20_000 });

    // Root agent delegated; planner-agent subagent card renders.
    await expect(page.getByText(/planner-agent/i).first())
      .toBeVisible({ timeout: 20_000 });

    // Artifacts REST hydration produces the recorded report pill.
    await expect(page.getByText(/aapl-valuation-report\.md/i))
      .toBeVisible({ timeout: 20_000 });
  });
});
