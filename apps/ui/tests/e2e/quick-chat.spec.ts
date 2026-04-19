import { test, expect } from './fixtures';
import type { Page, Route } from '@playwright/test';

/**
 * Quick Chat v2 E2E tests.
 *
 * Layout:
 *   • "Page load" — no daemon, no mocks. Just rendering + routing.
 *   • "Route-mocked" — intercepts /api/chat/init + /api/.../artifacts so
 *     we can drive UI flows (artifact cards, slide-out) without needing
 *     the agent to actually produce a file.
 *   • "Agent-backed" — auto-skip if the gateway isn't running; live send
 *     of simple prompts to verify the pill + reply render.
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
  await page.waitForTimeout(500);
  return ((await assistant.textContent()) ?? '').trim();
}

// ===========================================================================
// Page-load tests (no daemon required)
// ===========================================================================

test.describe('Quick Chat v2 — page load', () => {
  test('renders composer and Clear button', async ({ page }) => {
    await openQuickChat(page);

    // The empty-state heading only shows when the reserved session has no
    // history — the composer + Clear button are always visible once
    // bootstrapped, so they're the portable page-load assertion.
    await expect(page.getByPlaceholder('Type a message...')).toBeVisible();
    await expect(page.getByRole('button', { name: /Clear chat/i })).toBeVisible();
  });

  test('does NOT render a legacy "New chat" button', async ({ page }) => {
    await openQuickChat(page);
    await expect(page.getByText(/New chat/i)).toHaveCount(0);
  });

  test('status pill is hidden while session is idle', async ({ page }) => {
    await openQuickChat(page);
    await expect(page.locator('[data-testid="status-pill"]')).toHaveCount(0);
  });
});

// ===========================================================================
// Route-mocked tests — drive UI behaviours without the real agent
// ===========================================================================

const STUB_SESSION = {
  sessionId: 'sess-chat-e2e',
  conversationId: 'chat-e2e',
  created: false,
};

const STUB_ARTIFACTS = [
  {
    id: 'art-e2e-1',
    sessionId: STUB_SESSION.sessionId,
    filePath: '/tmp/notes-2026-04-19.md',
    fileName: 'notes-2026-04-19.md',
    fileType: 'md',
    fileSize: 420,
    label: 'daily note',
    createdAt: new Date().toISOString(),
  },
];

const STUB_ARTIFACT_CONTENT = `# Today
2026-04-19 12:34 EDT

A short note written in the scratch ward.
`;

async function installApiStubs(page: Page) {
  // Stub chat/init (idempotent).
  await page.route(`${API_BASE}/api/chat/init`, async (route: Route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(STUB_SESSION),
    });
  });

  // Stub root-scoped messages (empty history).
  await page.route(
    new RegExp(`${API_BASE}/api/executions/v2/sessions/${STUB_SESSION.sessionId}/messages.*`),
    (route: Route) => route.fulfill({ status: 200, contentType: 'application/json', body: '[]' })
  );

  // Stub artifact list (one file).
  await page.route(
    `${API_BASE}/api/sessions/${STUB_SESSION.sessionId}/artifacts`,
    (route: Route) =>
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(STUB_ARTIFACTS) })
  );

  // Stub artifact content (served on slide-out open).
  await page.route(
    `${API_BASE}/api/artifacts/${STUB_ARTIFACTS[0].id}/content`,
    (route: Route) =>
      route.fulfill({
        status: 200,
        contentType: 'text/markdown',
        body: STUB_ARTIFACT_CONTENT,
      })
  );
}

test.describe('Quick Chat v2 — route-mocked', () => {
  test('renders artifact card + opens slide-out with file content', async ({ page }) => {
    await installApiStubs(page);

    // Seed the UI with a visible user message so the artifact strip renders
    // (the strip is hidden on the empty state).
    await page.addInitScript(() => {
      // Nothing — user message is appended by driving the composer below.
    });

    await openQuickChat(page);
    await sendPrompt(page, 'create a scratch note');

    // Wait for the artifact card to appear after the turn-complete refresh.
    const artifactCard = page.getByTestId('quick-chat-artifact').first();
    await artifactCard.waitFor({ state: 'visible', timeout: 10_000 });
    await expect(artifactCard).toContainText('notes-2026-04-19.md');

    // Click the card — slide-out opens.
    await artifactCard.click();
    const slideOut = page.locator('.artifact-slideout').first();
    await expect(slideOut).toBeVisible();

    // Slide-out shows the markdown filename header and content.
    await expect(slideOut).toContainText('notes-2026-04-19.md');
    await expect(slideOut).toContainText('A short note written in the scratch ward');

    // Close via the X button.
    await slideOut.getByTitle('Close').click();
    await expect(slideOut).toHaveCount(0);
  });

  test('Clear button triggers confirm + DELETE /api/chat/session', async ({ page }) => {
    await installApiStubs(page);

    let deleteHit = false;
    await page.route(`${API_BASE}/api/chat/session`, (route: Route) => {
      if (route.request().method() === 'DELETE') {
        deleteHit = true;
      }
      return route.fulfill({ status: 204, body: '' });
    });

    await openQuickChat(page);

    // Auto-accept the browser confirm.
    page.once('dialog', (d) => d.accept());

    await page.getByRole('button', { name: /Clear chat/i }).click();
    // Give the async DELETE a moment to hit the route handler.
    await page.waitForTimeout(500);
    expect(deleteHit).toBe(true);
  });
});

// ===========================================================================
// Agent-backed tests (auto-skip if daemon not reachable at :18791)
// ===========================================================================

test.describe('Quick Chat v2 — agent-backed', () => {
  test.beforeEach(async ({}, testInfo) => {
    const ok = await daemonReachable();
    testInfo.skip(!ok, `gateway daemon not reachable at ${API_BASE}`);
  });

  test('answers a simple prompt', async ({ page }) => {
    test.setTimeout(90_000);
    await openQuickChat(page);
    await sendPrompt(page, 'Say hello in one short sentence.');
    await expect(page.locator('.quick-chat__user-bubble').last()).toContainText(/hello/i);
    const reply = await waitForAssistantReply(page);
    expect(reply.length).toBeGreaterThan(0);
  });

  test('status pill appears while the agent is working', async ({ page }) => {
    test.setTimeout(90_000);
    await openQuickChat(page);
    await sendPrompt(page, 'What is 2 plus 2?');
    const pill = page.locator('[data-testid="status-pill"]');
    await expect(pill).toBeVisible({ timeout: 15_000 });
    await waitForAssistantReply(page);
    await expect(pill).toHaveCount(0, { timeout: 30_000 });
  });
});
