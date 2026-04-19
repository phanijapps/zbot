import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';

/**
 * Quick Chat v2 E2E tests.
 *
 * Structure:
 * - "Page load" tests run without a daemon — they verify rendering only.
 * - "Agent-backed" tests require the gateway daemon to be running at
 *   http://localhost:18791 and the `quick-chat` agent to be installed
 *   under ~/Documents/zbot/agents/quick-chat/. They skip if the daemon
 *   isn't reachable so CI doesn't false-positive.
 */

const API_BASE = 'http://localhost:18791';

async function daemonReachable(): Promise<boolean> {
  try {
    const res = await fetch(`${API_BASE}/api/health`, {
      signal: AbortSignal.timeout(2000),
    });
    return res.ok;
  } catch {
    return false;
  }
}

async function openQuickChat(page: Page) {
  await page.goto('/chat-v2');
  // Wait for either the empty state or an existing conversation to render.
  await page.waitForSelector('.quick-chat', { state: 'visible', timeout: 15_000 });
}

async function sendPrompt(page: Page, text: string) {
  const textarea = page.getByPlaceholder('Type a message...');
  await textarea.fill(text);
  await textarea.press('Enter');
}

async function waitForAssistantReply(page: Page, timeoutMs = 60_000): Promise<string> {
  const assistant = page.locator('.quick-chat__assistant').last();
  await assistant.waitFor({ state: 'visible', timeout: timeoutMs });
  // Assistant bubble streams — give it a moment to settle, then snapshot.
  await page.waitForTimeout(500);
  const text = (await assistant.textContent()) ?? '';
  return text.trim();
}

// ===========================================================================
// Page-load tests (no daemon required)
// ===========================================================================

test.describe('Quick Chat v2 — page load', () => {
  test('empty state renders with composer and New chat button', async ({ page }) => {
    await openQuickChat(page);

    await expect(page.getByRole('heading', { name: 'Quick chat' })).toBeVisible();
    await expect(page.getByPlaceholder('Type a message...')).toBeVisible();
    await expect(page.getByRole('button', { name: /New chat/i })).toBeVisible();
  });

  test('ward chip is present (either active ward name or "no ward")', async ({ page }) => {
    await openQuickChat(page);

    const chip = page.locator('.quick-chat__ward-chip');
    await expect(chip).toBeVisible();
    const text = (await chip.textContent())?.trim() ?? '';
    expect(text.length).toBeGreaterThan(0);
  });

  test('status pill is hidden while session is idle', async ({ page }) => {
    await openQuickChat(page);

    const pill = page.locator('[data-testid="status-pill"]');
    await expect(pill).toHaveCount(0);
  });

  test('New chat button navigates back to /chat-v2 root', async ({ page }) => {
    await openQuickChat(page);

    // Simulate a session-scoped URL; the app should redirect/clear on New chat.
    await page.goto('/chat-v2/sess-deadbeef');
    await page.waitForSelector('.quick-chat', { state: 'visible' });

    await page.getByRole('button', { name: /New chat/i }).click();
    await expect(page).toHaveURL(/\/chat-v2\/?$/);
  });
});

// ===========================================================================
// Agent-backed tests (require running gateway daemon + quick-chat agent)
// ===========================================================================

test.describe('Quick Chat v2 — agent-backed', () => {
  test.beforeEach(async ({}, testInfo) => {
    const ok = await daemonReachable();
    testInfo.skip(!ok, `gateway daemon not reachable at ${API_BASE}`);
  });

  test('answers "what is the time now?" with a grounded response', async ({ page }) => {
    test.setTimeout(90_000);
    await openQuickChat(page);

    await sendPrompt(page, 'What is the time now?');

    // User bubble appears immediately.
    await expect(page.locator('.quick-chat__user-bubble').last()).toContainText(/time/i);

    // Assistant reply arrives.
    const reply = await waitForAssistantReply(page);

    // Must be non-empty and mention something time-shaped. The agent has no
    // clock tool in its allowlist, so the expected behavior is either:
    //   (a) answer from general knowledge of the session's date, OR
    //   (b) explicitly state it has no time access.
    expect(reply.length).toBeGreaterThan(0);
    expect(reply).toMatch(/(time|clock|date|don'?t have|cannot|unable|\d)/i);
  });

  test('answers "what skills and agents do you have?" by listing capabilities', async ({ page }) => {
    test.setTimeout(90_000);
    await openQuickChat(page);

    await sendPrompt(page, 'What skills and agents do you have access to?');

    await expect(page.locator('.quick-chat__user-bubble').last()).toContainText(/skills|agents/i);

    const reply = await waitForAssistantReply(page);

    expect(reply.length).toBeGreaterThan(0);
    // Reply must discuss skills, agents, or tools — the quick-chat prompt
    // explicitly names memory, load_skill, delegate_to_agent, ward, grep,
    // graph_query, ingest, multimodal_analyze, respond.
    expect(reply).toMatch(/(skill|agent|tool|memory|delegate|ward)/i);
  });

  test('status pill appears while the agent is working', async ({ page }) => {
    test.setTimeout(90_000);
    await openQuickChat(page);

    await sendPrompt(page, 'Say hello in one short sentence.');

    // Pill should appear within a few seconds of sending.
    const pill = page.locator('[data-testid="status-pill"]');
    await expect(pill).toBeVisible({ timeout: 15_000 });

    // And should disappear once the turn completes.
    await waitForAssistantReply(page);
    await expect(pill).toHaveCount(0, { timeout: 30_000 });
  });

  test('URL updates to /chat-v2/:sessionId after first message', async ({ page }) => {
    test.setTimeout(90_000);
    await openQuickChat(page);

    await expect(page).toHaveURL(/\/chat-v2\/?$/);

    await sendPrompt(page, 'Hi.');
    await waitForAssistantReply(page);

    await expect(page).toHaveURL(/\/chat-v2\/sess-[\w-]+$/);
  });
});
