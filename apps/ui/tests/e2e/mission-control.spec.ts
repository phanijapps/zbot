import { test, expect } from './fixtures';

// ============================================================================
// MISSION CONTROL — E2E regression guard
//
// Tests run against the live daemon (reuseExistingServer = true in config).
// All assertions are structural / behavioral — no hardcoded session IDs or
// titles so the suite stays green as historical data accumulates.
//
// Key regression covered:
//   Token counts — before the fix, every subagent card in a session that
//   delegated the same agent type multiple times (e.g. 7× builder-agent)
//   displayed the same combined total instead of its individual run count.
//   The suite asserts that a multi-subagent session shows ≥ 2 distinct token
//   strings across its depth-1 agent group headers.
// ============================================================================

test.describe('Mission Control — Page Load', () => {
  test('page loads with KPI strip and session list', async ({ mcPage, page }) => {
    await mcPage.goto();

    await expect(mcPage.kpiStrip()).toBeVisible();
    await expect(page.locator('.kpi-strip__num').first()).toBeVisible();

    // At least the list container is present (may be empty if daemon has no history)
    await expect(mcPage.sessionList()).toBeVisible();
  });

  test('KPI strip shows labelled cells', async ({ mcPage, page }) => {
    await mcPage.goto();

    // Labels are inside .kpi-strip__label divs; check a few that are always present
    const labels = page.locator('.kpi-strip__label');
    await expect(labels.filter({ hasText: 'RUNNING' }).first()).toBeVisible();
    await expect(labels.filter({ hasText: 'QUEUED' }).first()).toBeVisible();
    // "DONE · 24H" cell — match by containing "DONE" to avoid middot encoding issues
    await expect(labels.filter({ hasText: 'DONE' }).first()).toBeVisible();
  });
});

test.describe('Mission Control — Session List', () => {
  test.beforeEach(async ({ mcPage }) => {
    await mcPage.goto();
  });

  test('status filter chips are present and pressable', async ({ page }) => {
    // All five filter buttons must be present
    for (const label of ['RUNNING', 'QUEUED', 'DONE', 'FAILED', 'PAUSED']) {
      await expect(page.getByRole('button', { name: label })).toBeVisible();
    }
  });

  test('toggling a filter chip changes its active state', async ({ page }) => {
    await page.goto('/mission-control');
    await page.waitForSelector('.session-list-panel__chip', { state: 'visible', timeout: 10_000 });

    const chip = page.getByRole('button', { name: 'DONE' });
    const wasOn = await chip.evaluate((el) => el.classList.contains('session-list-panel__chip--on'));

    await chip.click();
    await page.waitForTimeout(200);

    const isNowOn = await chip.evaluate((el) => el.classList.contains('session-list-panel__chip--on'));

    // Click must have toggled the chip state
    expect(isNowOn).toBe(!wasOn);

    // Restore
    await chip.click();
  });

  test('search box filters session rows by title', async ({ mcPage, page }) => {
    const allRows = await page.locator('.session-list-panel__row').count();
    if (allRows === 0) {
      test.skip(); // nothing to filter
      return;
    }

    // Type something unlikely to match every session
    await mcPage.searchBox().fill('zzz_unlikely_match_xyz');
    await page.waitForTimeout(300);

    const filteredRows = await page.locator('.session-list-panel__row').count();
    expect(filteredRows).toBeLessThan(allRows);

    // Clear the search to restore the list
    await mcPage.searchBox().fill('');
  });

  test('session rows show token pair badges when tokens are available', async ({ page }) => {
    await page.goto('/mission-control');
    await page.waitForSelector('.session-list-panel__row', { state: 'visible', timeout: 15_000 });

    // At least one row with a token-pair badge should exist (live data has history)
    const tokenBadges = page.locator('.session-list-panel__row .token-pair');
    const count = await tokenBadges.count();

    // We don't assert > 0 because a freshly booted daemon could have no history yet;
    // instead we verify the badge structure when one exists.
    if (count > 0) {
      await expect(tokenBadges.first().locator('.token-pair__in')).toBeVisible();
      await expect(tokenBadges.first().locator('.token-pair__out')).toBeVisible();
    }
  });
});

test.describe('Mission Control — Session Selection', () => {
  test.beforeEach(async ({ mcPage }) => {
    await mcPage.goto();
  });

  test('clicking a session row populates the detail pane', async ({ mcPage, page }) => {
    const rows = page.locator('.session-list-panel__row');
    const count = await rows.count();
    if (count === 0) {
      test.skip();
      return;
    }

    await rows.first().click();

    // Detail pane should become non-empty
    await expect(mcPage.detailPane()).toBeVisible({ timeout: 10_000 });
  });

  test('selected session shows status badge and elapsed time in header', async ({ mcPage, page }) => {
    const rows = page.locator('.session-list-panel__row');
    if (await rows.count() === 0) { test.skip(); return; }

    await rows.first().click();
    await expect(mcPage.detailPane()).toBeVisible({ timeout: 10_000 });

    // Status badge (running / completed / error / ...)
    await expect(page.locator('.session-detail-pane__status').first()).toBeVisible();
    // Elapsed time text "X elapsed"
    await expect(page.getByText(/elapsed/i).first()).toBeVisible();
  });

  test('selected session highlights its row in the list', async ({ mcPage, page }) => {
    const rows = page.locator('.session-list-panel__row');
    if (await rows.count() === 0) { test.skip(); return; }

    await rows.first().click();
    await page.waitForTimeout(200);

    // The first row should now carry the --active modifier class
    await expect(rows.first()).toHaveClass(/session-list-panel__row--active/);
  });

  test('action buttons (Pause, Stop, Retry, Open in Research) are present', async ({ mcPage, page }) => {
    const rows = page.locator('.session-list-panel__row');
    if (await rows.count() === 0) { test.skip(); return; }

    await rows.first().click();
    await expect(mcPage.detailPane()).toBeVisible({ timeout: 10_000 });

    const actions = page.locator('.session-detail-pane__actions');
    await expect(actions.getByRole('button', { name: 'Pause', exact: true })).toBeVisible();
    await expect(actions.getByRole('button', { name: 'Stop', exact: true })).toBeVisible();
    await expect(actions.getByRole('button', { name: 'Retry', exact: true })).toBeVisible();
    await expect(actions.getByRole('button', { name: /open in research/i })).toBeVisible();
  });
});

test.describe('Mission Control — Tools Pane', () => {
  test.beforeEach(async ({ mcPage }) => {
    await mcPage.goto();
  });

  test('tools pane shows root agent group after selecting a session', async ({ mcPage, page }) => {
    const rows = page.locator('.session-list-panel__row');
    if (await rows.count() === 0) { test.skip(); return; }

    await rows.first().click();
    await mcPage.waitForTrace();

    // The root group at depth-0 must be present
    await expect(page.locator('.agent-tool-group--depth-0')).toBeVisible();
  });

  test('root agent group header shows duration and tool count', async ({ mcPage, page }) => {
    const rows = page.locator('.session-list-panel__row');
    if (await rows.count() === 0) { test.skip(); return; }

    await rows.first().click();
    await mcPage.waitForTrace();

    const rootHead = page.locator('.agent-tool-group--depth-0 .agent-tool-group__head').first();
    await expect(rootHead).toBeVisible();

    // Meta span shows "N tools"
    await expect(rootHead.locator('.agent-tool-group__meta')).toContainText(/\d+ tool/);
  });

  test('clicking a tool row opens the detail popover', async ({ mcPage, page }) => {
    const rows = page.locator('.session-list-panel__row');
    if (await rows.count() === 0) { test.skip(); return; }

    await rows.first().click();
    await mcPage.waitForTrace();

    // Find any clickable tool row
    const toolRow = page.locator('.agent-tool-group__row--clickable').first();
    const hasRows = await toolRow.isVisible().catch(() => false);
    if (!hasRows) { test.skip(); return; }

    await toolRow.click();

    // Popover should appear
    await expect(page.locator('.tool-popover')).toBeVisible({ timeout: 5_000 });
    await expect(page.locator('.tool-popover__name')).toBeVisible();

    // Close it
    await page.locator('.tool-popover__close').click();
    await expect(page.locator('.tool-popover')).not.toBeVisible();
  });
});

test.describe('Mission Control — Subagent Groups', () => {
  test.beforeEach(async ({ mcPage }) => {
    await mcPage.goto();
  });

  test('collapsed subagent can be expanded by clicking its header', async ({ mcPage, page }) => {
    const found = await mcPage.selectFirstMultiSubagentSession();
    if (!found) { test.skip(); return; }

    await mcPage.waitForTrace();

    // Find a collapsed (non-expanded) subagent button
    const collapsed = page.getByRole('button', { name: /^Expand / }).first();
    if (!(await collapsed.isVisible().catch(() => false))) { test.skip(); return; }

    const agentName = (await collapsed.getAttribute('aria-label') ?? '').replace('Expand ', '');
    await collapsed.click();

    // The button should now say "Collapse" (use .first() in case multiple same-name agents exist)
    await expect(page.getByRole('button', { name: `Collapse ${agentName}` }).first()).toBeVisible({ timeout: 3_000 });
  });

  test('subagent header shows status badge', async ({ mcPage, page }) => {
    const found = await mcPage.selectFirstMultiSubagentSession();
    if (!found) { test.skip(); return; }

    await mcPage.waitForTrace();

    const subagentHead = page.locator('.agent-tool-group--depth-1 .agent-tool-group__head').first();
    if (!(await subagentHead.isVisible().catch(() => false))) { test.skip(); return; }

    // Status span carries a modifier class like --completed / --running / --failed
    await expect(subagentHead.locator('.agent-tool-group__status')).toBeVisible();
  });
});

// ============================================================================
// REGRESSION: Token uniqueness across repeated same-agent delegations
//
// Scenario reproduced by the FLUX LoRA Training session which delegates to
// builder-agent 7 times. Before the fix, sumExecutionTokensByAgent collapsed
// all 7 runs into one map entry keyed by "builder-agent", so every card
// showed the same combined total. After the fix, each card looks up by its
// unique executionId and shows its own run's counts.
// ============================================================================
test.describe('Mission Control — Token Counts [REGRESSION]', () => {
  test.beforeEach(async ({ mcPage }) => {
    await mcPage.goto();
  });

  test('session header shows combined in/out token totals', async ({ mcPage, page }) => {
    const rows = page.locator('.session-list-panel__row');
    // Find a session row that already has a token-pair badge
    const rowWithTokens = rows.filter({ has: page.locator('.token-pair') }).first();
    if (!(await rowWithTokens.isVisible().catch(() => false))) { test.skip(); return; }

    await rowWithTokens.click();
    await expect(mcPage.detailPane()).toBeVisible({ timeout: 10_000 });

    // Detail pane header should carry a TokenPair
    const headerTokens = page.locator('.session-detail-pane__meta .token-pair');
    await expect(headerTokens).toBeVisible({ timeout: 5_000 });

    const inText = await headerTokens.locator('.token-pair__in').textContent();
    const outText = await headerTokens.locator('.token-pair__out').textContent();

    // Both must be non-empty numeric strings (e.g. "6.0M tok", "49.9k tok")
    expect(inText?.trim()).toMatch(/[\d.]+/);
    expect(outText?.trim()).toMatch(/[\d.]+/);
  });

  test('subagent cards in a multi-delegation session show distinct token counts', async ({ mcPage, page }) => {
    // Look for the FLUX LoRA session specifically — it has 7× builder-agent
    // delegations, which is the exact scenario that triggered the original bug.
    // Fall back to any session with 3+ subagents.
    let found = await mcPage.selectSession('FLUX LoRA Training');
    if (!found) {
      // Try any session with many subagents
      const rows = page.locator('.session-list-panel__row').filter({ hasText: /[5-9]\d* ↳|[1-9]\d+ ↳/ });
      if (await rows.count() === 0) { test.skip(); return; }
      await rows.first().click();
      await expect(mcPage.detailPane()).toBeVisible({ timeout: 10_000 });
      found = true;
    }

    await mcPage.waitForTrace();

    // Collect visible token strings from all depth-1 agent group headers
    const tokenEls = mcPage.subagentTokens();
    const count = await tokenEls.count();

    if (count < 2) {
      // Not enough subagents with token displays to compare — skip gracefully
      test.skip();
      return;
    }

    const tokenTexts = await Promise.all(
      Array.from({ length: count }, (_, i) => tokenEls.nth(i).textContent()),
    );

    const nonEmpty = tokenTexts.filter(Boolean) as string[];
    if (nonEmpty.length < 2) { test.skip(); return; }

    // REGRESSION ASSERTION: at least two subagent cards must show DIFFERENT
    // token counts. Before the fix they were all identical (combined total).
    const unique = new Set(nonEmpty.map((t) => t.trim()));
    expect(unique.size).toBeGreaterThan(1);
  });

  test('root agent group shows its own token slice, not the full session total', async ({ mcPage, page }) => {
    // Select a session that has subagents (so root tokens < total tokens)
    let found = await mcPage.selectSession('FLUX LoRA Training');
    if (!found) {
      const rows = page.locator('.session-list-panel__row').filter({ hasText: / ↳/ });
      if (await rows.count() === 0) { test.skip(); return; }
      await rows.first().click();
      await expect(mcPage.detailPane()).toBeVisible({ timeout: 10_000 });
    }

    await mcPage.waitForTrace();

    // Root group tokens (depth-0 header)
    const rootTokenEl = page.locator('.agent-tool-group--depth-0 .agent-tool-group__tokens').first();
    if (!(await rootTokenEl.isVisible().catch(() => false))) { test.skip(); return; }

    const rootTokenText = (await rootTokenEl.textContent())?.trim() ?? '';

    // Session header (combined total across all agents)
    const headerTokenEl = page.locator('.session-detail-pane__meta .token-pair');
    if (!(await headerTokenEl.isVisible().catch(() => false))) { test.skip(); return; }

    const headerTokenText = (await headerTokenEl.textContent())?.trim() ?? '';

    // When subagents exist, the combined total must differ from the root-only slice
    expect(rootTokenText).not.toBe('');
    expect(headerTokenText).not.toBe('');
    expect(rootTokenText).not.toBe(headerTokenText);
  });
});
