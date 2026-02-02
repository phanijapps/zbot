import { test, expect } from '@playwright/test';

/**
 * Subscription Debug Test
 *
 * This test captures the issue where events aren't received when the chat
 * panel stays open continuously. The hypothesis is that subscriptions are
 * sent before the WebSocket connects, so they're silently dropped.
 */

test.describe('Subscription Timing', () => {
  test('events received when panel stays open', async ({ page }) => {
    // Collect all console logs for debugging
    const allLogs: string[] = [];
    const transportLogs: string[] = [];
    const sessionLogs: string[] = [];

    page.on('console', (msg) => {
      const text = msg.text();
      allLogs.push(`[${msg.type()}] ${text}`);

      if (text.includes('[Transport]') || text.includes('[HttpTransport]')) {
        transportLogs.push(text);
        console.log('TRANSPORT:', text);
      }
      if (text.includes('[SESSION_DEBUG]')) {
        sessionLogs.push(text);
        console.log('SESSION:', text);
      }
    });

    // Navigate to app
    await page.goto('/');
    await page.waitForSelector('h1:has-text("Dashboard")', {
      state: 'visible',
      timeout: 15_000,
    });

    // Clear localStorage to start fresh
    await page.evaluate(() => {
      localStorage.removeItem('agentzero_web_session_id');
      localStorage.removeItem('agentzero_web_conv_id');
    });

    // Wait a moment for WebSocket to stabilize
    await page.waitForTimeout(2000);

    console.log('\n=== OPENING CHAT PANEL ===');

    // Click New Chat button to open panel
    const newChatBtn = page.getByRole('button', { name: /new chat/i });
    await newChatBtn.click();

    // Wait for chat input to be visible
    const chatInput = page.locator('textarea[placeholder*="Type a message"]');
    await chatInput.waitFor({ state: 'visible', timeout: 10_000 });

    // Log subscription state after panel opens
    console.log('\n=== PANEL OPENED - Checking subscription state ===');
    await page.waitForTimeout(1000);

    // Check WebSocket state
    const wsState = await page.evaluate(() => {
      // Access transport internals via window for debugging
      const transport = (window as any).__DEBUG_TRANSPORT__;
      if (transport) {
        return {
          wsReadyState: transport.ws?.readyState,
          subscriptionCount: transport.conversationSubscriptions?.size || 0,
          connectionStatus: transport.connectionState?.status,
        };
      }
      return null;
    });
    console.log('WebSocket state:', wsState);

    // Send first message WITHOUT closing the panel
    console.log('\n=== SENDING MESSAGE (panel stayed open) ===');
    await chatInput.fill('Say "Hello from test" and nothing else.');
    const sendButton = page.locator('button').filter({ has: page.locator('.lucide-send') });

    const beforeCount = await page.locator('[class*="prose"]').count();
    console.log('Message count before send:', beforeCount);

    await sendButton.click();

    // Wait for response - this is where it would hang if subscriptions aren't working
    console.log('\n=== WAITING FOR RESPONSE ===');

    try {
      await page.waitForFunction(
        (prevCount) => {
          const messages = document.querySelectorAll('[class*="prose"]');
          return messages.length > prevCount;
        },
        beforeCount,
        { timeout: 30_000 }
      );

      const afterCount = await page.locator('[class*="prose"]').count();
      console.log('Message count after response:', afterCount);
      console.log('\n=== SUCCESS: Response received! ===');
    } catch (error) {
      console.log('\n=== FAILURE: No response received ===');
      console.log('This confirms the subscription timing bug.');
    }

    // Print all transport logs
    console.log('\n=== ALL TRANSPORT LOGS ===');
    transportLogs.forEach((log) => console.log(log));

    // Print all session logs
    console.log('\n=== ALL SESSION LOGS ===');
    sessionLogs.forEach((log) => console.log(log));

    // Verify we got a response
    const finalCount = await page.locator('[class*="prose"]').count();
    expect(finalCount).toBeGreaterThan(beforeCount);
  });

  test('events received after closing and reopening panel', async ({ page }) => {
    // Collect console logs
    const transportLogs: string[] = [];

    page.on('console', (msg) => {
      const text = msg.text();
      if (text.includes('[Transport]') || text.includes('[HttpTransport]')) {
        transportLogs.push(text);
        console.log('TRANSPORT:', text);
      }
    });

    // Navigate to app
    await page.goto('/');
    await page.waitForSelector('h1:has-text("Dashboard")', {
      state: 'visible',
      timeout: 15_000,
    });

    // Clear localStorage
    await page.evaluate(() => {
      localStorage.removeItem('agentzero_web_session_id');
      localStorage.removeItem('agentzero_web_conv_id');
    });

    // Wait for WebSocket
    await page.waitForTimeout(2000);

    console.log('\n=== OPENING CHAT PANEL ===');
    const newChatBtn = page.getByRole('button', { name: /new chat/i });
    await newChatBtn.click();

    const chatInput = page.locator('textarea[placeholder*="Type a message"]');
    await chatInput.waitFor({ state: 'visible', timeout: 10_000 });

    console.log('\n=== CLOSING PANEL ===');
    // Close the panel by pressing Escape or clicking the backdrop
    await page.keyboard.press('Escape');
    await page.waitForTimeout(500);

    console.log('\n=== REOPENING PANEL ===');
    await newChatBtn.click();
    await chatInput.waitFor({ state: 'visible', timeout: 10_000 });

    // Now send message
    console.log('\n=== SENDING MESSAGE (after reopen) ===');
    await chatInput.fill('Say "Hello after reopen" and nothing else.');
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

    console.log('\n=== SUCCESS: Response received after reopen ===');

    // Print transport logs
    console.log('\n=== TRANSPORT LOGS ===');
    transportLogs.forEach((log) => console.log(log));

    const finalCount = await page.locator('[class*="prose"]').count();
    expect(finalCount).toBeGreaterThan(beforeCount);
  });
});
