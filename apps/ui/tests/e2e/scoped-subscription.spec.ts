import { test, expect } from '@playwright/test';

/**
 * Scoped Event Emission Tests
 *
 * Tests the server-side event filtering based on subscription scope:
 * - Session scope: only root execution events + delegation lifecycle markers
 * - Execution scope: all events for a specific execution (debug view)
 * - All scope: unfiltered (backward compatible)
 *
 * These tests verify the subscription mechanics work correctly.
 * Full integration testing of event filtering requires delegation scenarios.
 */

test.describe('Scoped Event Subscription', () => {
  test('session scope subscription receives events', async ({ page }) => {
    // Collect transport logs - MUST be set up before navigation
    const transportLogs: string[] = [];

    page.on('console', (msg) => {
      const text = msg.text();
      if (text.includes('[Transport]') || text.includes('[HttpTransport]') || text.includes('[WebChatPanel]')) {
        transportLogs.push(text);
        console.log('LOG:', text);
      }
    });

    // Navigate to app
    await page.goto('/');

    // Clear localStorage to start fresh
    await page.evaluate(() => {
      localStorage.removeItem('agentzero_web_session_id');
      localStorage.removeItem('agentzero_web_conv_id');
    });

    // Wait for app to load
    await page.waitForSelector('h1:has-text("Dashboard")', {
      state: 'visible',
      timeout: 15_000,
    });

    // Wait for WebSocket to connect
    await page.waitForTimeout(2000);

    // Open chat panel (uses session scope by default)
    const newChatBtn = page.getByRole('button', { name: /new chat/i });
    await newChatBtn.click();

    const chatInput = page.locator('textarea[placeholder*="Type a message"]');
    await chatInput.waitFor({ state: 'visible', timeout: 10_000 });

    // Wait for subscription to be established
    await page.waitForTimeout(1000);

    // Send a simple message
    await chatInput.fill('Say "test response" and nothing else.');
    const sendButton = page.locator('button').filter({ has: page.locator('.lucide-send') });

    const beforeCount = await page.locator('[class*="prose"]').count();
    await sendButton.click();

    // Wait for response
    await page.waitForFunction(
      (prevCount) => {
        const messages = document.querySelectorAll('[class*="prose"]');
        return messages.length > prevCount;
      },
      beforeCount,
      { timeout: 30_000 }
    );

    const afterCount = await page.locator('[class*="prose"]').count();
    expect(afterCount).toBeGreaterThan(beforeCount);

    // Log transport activity for debugging
    console.log('\n=== TRANSPORT LOGS ===');
    transportLogs.forEach((log) => console.log(log));

    // Verify session scope subscription was established
    // Look for "scope: session" in the subscribe or subscribed messages
    const hasSessionScopeLog = transportLogs.some(
      (log) => log.includes('scope: session') || log.includes('with scope: session')
    );
    expect(hasSessionScopeLog).toBe(true);
  });

  test('subscription confirmation includes root execution IDs', async ({ page }) => {
    // Collect transport logs
    const transportLogs: string[] = [];

    page.on('console', (msg) => {
      const text = msg.text();
      if (text.includes('[Transport]') || text.includes('[HttpTransport]')) {
        transportLogs.push(text);
        console.log('LOG:', text);
      }
    });

    await page.goto('/');

    // Clear localStorage
    await page.evaluate(() => {
      localStorage.removeItem('agentzero_web_session_id');
      localStorage.removeItem('agentzero_web_conv_id');
    });

    await page.waitForSelector('h1:has-text("Dashboard")', {
      state: 'visible',
      timeout: 15_000,
    });

    await page.waitForTimeout(2000);

    // Open chat panel
    const newChatBtn = page.getByRole('button', { name: /new chat/i });
    await newChatBtn.click();

    const chatInput = page.locator('textarea[placeholder*="Type a message"]');
    await chatInput.waitFor({ state: 'visible', timeout: 10_000 });

    // Wait for subscription to complete
    await page.waitForTimeout(1500);

    // Log transport activity
    console.log('\n=== TRANSPORT LOGS ===');
    transportLogs.forEach((log) => console.log(log));

    // Verify we got a subscribed response (with roots count)
    const hasSubscribedWithRoots = transportLogs.some(
      (log) => log.includes('Subscribed to') && log.includes('roots:')
    );
    expect(hasSubscribedWithRoots).toBe(true);

    // For a new conversation, roots should be 0
    const hasZeroRoots = transportLogs.some(
      (log) => log.includes('roots: 0')
    );
    expect(hasZeroRoots).toBe(true);
  });
});
