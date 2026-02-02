import { test, expect } from '@playwright/test';

/**
 * Session Management Debug Test
 * Verifies that multiple messages in the same conversation use the same session.
 */

test.describe('Session Management', () => {
  test('multi-turn conversation uses same session', async ({ page }) => {
    // Collect console logs
    const consoleLogs: string[] = [];
    page.on('console', (msg) => {
      const text = msg.text();
      if (text.includes('[SESSION_DEBUG]')) {
        consoleLogs.push(text);
        console.log('BROWSER:', text);
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

    // Click New Chat button
    const newChatBtn = page.getByRole('button', { name: /new chat/i });
    await newChatBtn.click();

    // Wait for chat input
    const chatInput = page.locator('textarea[placeholder*="Type a message"]');
    await chatInput.waitFor({ state: 'visible', timeout: 10_000 });

    // Send first message
    console.log('\n=== SENDING MESSAGE 1 ===');
    await chatInput.fill('Say "Hi Alice" and nothing else.');
    const sendButton = page.locator('button').filter({ has: page.locator('.lucide-send') });

    // Get current message count before sending
    const beforeCount1 = await page.locator('[class*="prose"]').count();
    await sendButton.click();

    // Wait for new response (message count increases)
    await page.waitForFunction(
      (prevCount) => {
        const messages = document.querySelectorAll('[class*="prose"]');
        return messages.length > prevCount;
      },
      beforeCount1,
      { timeout: 90_000 }
    );

    // Small delay to ensure session_id is stored
    await page.waitForTimeout(1000);

    // Get session_id after first message
    const sessionId1 = await page.evaluate(() => {
      return localStorage.getItem('agentzero_web_session_id');
    });
    console.log('Session ID after message 1:', sessionId1);

    // Send second message
    console.log('\n=== SENDING MESSAGE 2 ===');
    const beforeCount2 = await page.locator('[class*="prose"]').count();
    await chatInput.fill('Say "Goodbye" and nothing else.');
    await sendButton.click();

    // Wait for new response (message count increases again)
    await page.waitForFunction(
      (prevCount) => {
        const messages = document.querySelectorAll('[class*="prose"]');
        return messages.length > prevCount;
      },
      beforeCount2,
      { timeout: 90_000 }
    );

    await page.waitForTimeout(1000);

    // Get session_id after second message
    const sessionId2 = await page.evaluate(() => {
      return localStorage.getItem('agentzero_web_session_id');
    });
    console.log('Session ID after message 2:', sessionId2);

    // Print all captured logs
    console.log('\n=== ALL SESSION DEBUG LOGS ===');
    consoleLogs.forEach((log) => console.log(log));

    // Assertions
    expect(sessionId1).toBeTruthy();
    expect(sessionId2).toBeTruthy();
    expect(sessionId1).toBe(sessionId2); // CRITICAL: Same session for both messages

    console.log('\n=== TEST PASSED: Same session used for both messages ===');
    console.log('Session ID:', sessionId1);
  });

  test('new command creates new session', async ({ page }) => {
    // Collect console logs
    const consoleLogs: string[] = [];
    page.on('console', (msg) => {
      const text = msg.text();
      if (text.includes('[SESSION_DEBUG]')) {
        consoleLogs.push(text);
        console.log('BROWSER:', text);
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

    // Click New Chat button
    const newChatBtn = page.getByRole('button', { name: /new chat/i });
    await newChatBtn.click();

    // Wait for chat input
    const chatInput = page.locator('textarea[placeholder*="Type a message"]');
    await chatInput.waitFor({ state: 'visible', timeout: 10_000 });
    const sendButton = page.locator('button').filter({ has: page.locator('.lucide-send') });

    // Send first message
    console.log('\n=== SENDING MESSAGE 1 (first session) ===');
    const beforeCount1 = await page.locator('[class*="prose"]').count();
    await chatInput.fill('Say "Hello" and nothing else.');
    await sendButton.click();

    // Wait for response (message count increases)
    await page.waitForFunction(
      (prevCount) => {
        const messages = document.querySelectorAll('[class*="prose"]');
        return messages.length > prevCount;
      },
      beforeCount1,
      { timeout: 90_000 }
    );
    await page.waitForTimeout(1000);

    // Get session_id after first message
    const sessionId1 = await page.evaluate(() => {
      return localStorage.getItem('agentzero_web_session_id');
    });
    console.log('Session ID from first session:', sessionId1);

    // Use /new command
    console.log('\n=== SENDING /new COMMAND ===');
    await chatInput.fill('/new');
    await sendButton.click();
    await page.waitForTimeout(1000);

    // Check session was cleared
    const sessionIdAfterNew = await page.evaluate(() => {
      return localStorage.getItem('agentzero_web_session_id');
    });
    console.log('Session ID after /new:', sessionIdAfterNew);
    expect(sessionIdAfterNew).toBeNull();

    // Send message in new session
    console.log('\n=== SENDING MESSAGE 2 (new session) ===');
    const beforeCount2 = await page.locator('[class*="prose"]').count();
    await chatInput.fill('Say "Goodbye" and nothing else.');
    await sendButton.click();

    // Wait for response (message count increases)
    await page.waitForFunction(
      (prevCount) => {
        const messages = document.querySelectorAll('[class*="prose"]');
        return messages.length > prevCount;
      },
      beforeCount2,
      { timeout: 90_000 }
    );
    await page.waitForTimeout(1000);

    // Get session_id after second session message
    const sessionId2 = await page.evaluate(() => {
      return localStorage.getItem('agentzero_web_session_id');
    });
    console.log('Session ID from second session:', sessionId2);

    // Print all captured logs
    console.log('\n=== ALL SESSION DEBUG LOGS ===');
    consoleLogs.forEach((log) => console.log(log));

    // Assertions
    expect(sessionId1).toBeTruthy();
    expect(sessionId2).toBeTruthy();
    expect(sessionId1).not.toBe(sessionId2); // CRITICAL: Different sessions

    console.log('\n=== TEST PASSED: /new created a new session ===');
    console.log('Session 1:', sessionId1);
    console.log('Session 2:', sessionId2);
  });
});
