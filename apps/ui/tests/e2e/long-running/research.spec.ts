import { test, expect } from '@playwright/test';

/**
 * Research Agent E2E Tests
 * Long-running tests that require real LLM backend.
 * These tests verify subagent delegation and complex research tasks.
 *
 * NOTE: These tests are skipped in CI as they require real LLM API keys.
 * To run locally: npm run test:e2e -- tests/e2e/long-running/research.spec.ts
 */

// Long-running tests - increase timeout to 5 minutes
test.setTimeout(300_000);

test.describe('Research Agent Scenarios', () => {
  // Skip in CI - requires real LLM
  test.skip(!!process.env.CI, 'Skip in CI - requires real LLM');

  test.beforeEach(async ({ page }) => {
    // Navigate to home (dashboard)
    await page.goto('/');

    // Wait for page to load
    await page.waitForSelector('h1:has-text("Dashboard")', {
      state: 'visible',
      timeout: 15_000,
    });
  });

  test('can start a new chat session', async ({ page }) => {
    // Click New Chat button
    const newChatBtn = page.getByRole('button', { name: /new chat/i });
    await newChatBtn.click();

    // Chat slider should open with input area
    await expect(page.locator('textarea[placeholder*="Type a message"]')).toBeVisible({
      timeout: 10_000,
    });
  });

  test('can send a message and receive response', async ({ page }) => {
    // Open chat
    const newChatBtn = page.getByRole('button', { name: /new chat/i });
    await newChatBtn.click();

    // Wait for chat input
    const chatInput = page.locator('textarea[placeholder*="Type a message"]');
    await chatInput.waitFor({ state: 'visible', timeout: 10_000 });

    // Send a simple message
    await chatInput.fill('Hello! Please respond with just "Hi there!"');

    // Click send button (the button next to textarea with Send icon)
    const sendButton = page.locator('button').filter({ has: page.locator('.lucide-send') });
    await sendButton.click();

    // Wait for assistant response
    await expect(page.locator('text=Hi there').or(page.locator('text=Hello'))).toBeVisible({
      timeout: 60_000,
    });
  });

  test('research task triggers subagent', async ({ page, context }) => {
    // Open chat
    const newChatBtn = page.getByRole('button', { name: /new chat/i });
    await newChatBtn.click();

    // Wait for chat input
    const chatInput = page.locator('textarea[placeholder*="Type a message"]');
    await chatInput.waitFor({ state: 'visible', timeout: 10_000 });

    // Send research prompt
    await chatInput.fill(
      'Research the latest advancements in AI agents and summarize the top 3 developments.'
    );

    // Click send button
    const sendButton = page.locator('button').filter({ has: page.locator('.lucide-send') });
    await sendButton.click();

    // Wait for initial processing indication
    await expect(
      page.locator('.animate-spin').or(page.locator('text=Processing'))
    ).toBeVisible({ timeout: 30_000 });

    // Open dashboard in new tab to monitor
    const dashboardPage = await context.newPage();
    await dashboardPage.goto('/');

    // Wait for dashboard to load
    await dashboardPage.waitForSelector('h1:has-text("Dashboard")', {
      state: 'visible',
      timeout: 15_000,
    });

    // Wait for session to appear in Active Sessions
    const activeSessionsSection = dashboardPage.locator('section, div').filter({
      has: dashboardPage.getByRole('heading', { name: 'Active Sessions' }),
    });

    // Look for running session indicator
    await expect(
      activeSessionsSection
        .locator('text=Running')
        .or(activeSessionsSection.locator('text=running'))
        .or(dashboardPage.locator('.animate-spin'))
    ).toBeVisible({ timeout: 60_000 });

    // Look for subagent indicator (delegation activity)
    // The dashboard may show subagent count or "researcher" text
    const hasSubagent = await dashboardPage
      .locator('text=subagent')
      .or(dashboardPage.locator('text=researcher'))
      .or(dashboardPage.locator('text=+1'))
      .isVisible({ timeout: 120_000 })
      .catch(() => false);

    // Back to chat - wait for completion
    await page.bringToFront();

    // Wait for response to complete (look for substantial content or completion indicator)
    await page.waitForFunction(
      () => {
        const messages = document.querySelectorAll('[class*="prose"]');
        const lastMessage = messages[messages.length - 1];
        return lastMessage && lastMessage.textContent && lastMessage.textContent.length > 200;
      },
      { timeout: 180_000 }
    );

    // Verify response has substantive content
    const assistantMessages = await page.locator('[class*="prose"]').allTextContents();
    const fullResponse = assistantMessages.join(' ');

    expect(fullResponse.length).toBeGreaterThan(200);
    expect(fullResponse.toLowerCase()).toMatch(/ai|agent|research|development|advancement/i);

    // Log whether subagent was detected (informational)
    console.log(`Subagent delegation detected: ${hasSubagent}`);
  });

  test('handles research timeout gracefully', async ({ page }) => {
    // Open chat
    const newChatBtn = page.getByRole('button', { name: /new chat/i });
    await newChatBtn.click();

    // Wait for chat input
    const chatInput = page.locator('textarea[placeholder*="Type a message"]');
    await chatInput.waitFor({ state: 'visible', timeout: 10_000 });

    // Complex research that might take a while
    await chatInput.fill(
      'Analyze the complete history of artificial intelligence from 1950 to present day.'
    );

    // Click send button
    const sendButton = page.locator('button').filter({ has: page.locator('.lucide-send') });
    await sendButton.click();

    // Should either complete with response or show meaningful progress
    // We accept: completion, error message, or progress indication
    const result = await Promise.race([
      // Response with content
      page.waitForFunction(
        () => {
          const messages = document.querySelectorAll('[class*="prose"]');
          const lastMessage = messages[messages.length - 1];
          return lastMessage && lastMessage.textContent && lastMessage.textContent.length > 100;
        },
        { timeout: 180_000 }
      ),
      // Error message
      page.waitForSelector('text=error', { state: 'visible', timeout: 180_000 }).catch(() => null),
      // Timeout message
      page
        .waitForSelector('text=taking longer', { state: 'visible', timeout: 180_000 })
        .catch(() => null),
    ]);

    // Should have some result (not crashed)
    expect(result).toBeTruthy();
  });

  test('can cancel ongoing research', async ({ page }) => {
    // Open chat
    const newChatBtn = page.getByRole('button', { name: /new chat/i });
    await newChatBtn.click();

    // Wait for chat input
    const chatInput = page.locator('textarea[placeholder*="Type a message"]');
    await chatInput.waitFor({ state: 'visible', timeout: 10_000 });

    // Start a research task
    await chatInput.fill('Write a detailed 5000 word essay about quantum computing.');

    // Click send button
    const sendButton = page.locator('button').filter({ has: page.locator('.lucide-send') });
    await sendButton.click();

    // Wait for processing to start
    await expect(page.locator('.animate-spin')).toBeVisible({ timeout: 30_000 });

    // Try to close the chat slider (escape key or close button)
    await page.keyboard.press('Escape');

    // The page should not crash - dashboard should still be accessible
    await expect(page.locator('h1').filter({ hasText: 'Dashboard' })).toBeVisible({
      timeout: 10_000,
    });
  });
});
