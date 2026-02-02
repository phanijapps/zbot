import { test, expect } from '@playwright/test';

/**
 * Multi-Turn Conversation E2E Tests
 * Tests for maintaining context across multiple conversation turns.
 *
 * NOTE: These tests are skipped in CI as they require real LLM API keys.
 * To run locally: npm run test:e2e -- tests/e2e/long-running/multi-turn.spec.ts
 */

// Increased timeout for multi-turn conversations
test.setTimeout(180_000); // 3 minutes

test.describe('Multi-Turn Conversations', () => {
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

    // Open chat
    const newChatBtn = page.getByRole('button', { name: /new chat/i });
    await newChatBtn.click();

    // Wait for chat input
    await page.waitForSelector('textarea[placeholder*="Type a message"]', {
      state: 'visible',
      timeout: 10_000,
    });
  });

  /**
   * Helper to send a message and wait for response
   */
  async function sendMessageAndWait(page: import('@playwright/test').Page, message: string) {
    const chatInput = page.locator('textarea[placeholder*="Type a message"]');
    const sendButton = page.locator('button').filter({ has: page.locator('.lucide-send') });

    // Clear and fill input
    await chatInput.fill(message);

    // Get current message count
    const beforeCount = await page.locator('[class*="prose"]').count();

    // Send message
    await sendButton.click();

    // Wait for new response (message count increases)
    await page.waitForFunction(
      (prevCount) => {
        const messages = document.querySelectorAll('[class*="prose"]');
        return messages.length > prevCount;
      },
      beforeCount,
      { timeout: 60_000 }
    );

    // Wait for response to finish streaming (no more animation)
    await page.waitForTimeout(2000); // Brief pause for streaming to complete

    // Get the last assistant message content
    const messages = await page.locator('[class*="prose"]').allTextContents();
    return messages[messages.length - 1] || '';
  }

  test('maintains context across turns', async ({ page }) => {
    // Turn 1: Introduce a topic
    await sendMessageAndWait(
      page,
      'My name is Alice and I work as a software engineer. Remember this for our conversation.'
    );

    // Turn 2: Reference the name
    const response2 = await sendMessageAndWait(page, 'What is my name?');

    // Should remember the name
    expect(response2.toLowerCase()).toContain('alice');

    // Turn 3: Reference the profession
    const response3 = await sendMessageAndWait(page, 'What do I do for work?');

    // Should remember the profession
    expect(response3.toLowerCase()).toMatch(/software|engineer|programming|developer/i);
  });

  test('maintains context with follow-up questions', async ({ page }) => {
    // Turn 1: Ask about a topic
    await sendMessageAndWait(page, 'What are the three primary colors?');

    // Turn 2: Follow-up without restating topic
    const response2 = await sendMessageAndWait(page, 'Can you list them again?');

    // Should still be talking about colors
    expect(response2.toLowerCase()).toMatch(/red|blue|yellow|green|primary/i);

    // Turn 3: Another follow-up
    const response3 = await sendMessageAndWait(
      page,
      'What happens when you mix the first two you mentioned?'
    );

    // Should understand we're still discussing colors
    expect(response3.toLowerCase()).toMatch(/color|mix|purple|orange|green|secondary/i);
  });

  test('can reference information from earlier turns', async ({ page }) => {
    // Turn 1: Provide multiple pieces of information
    await sendMessageAndWait(
      page,
      'I have a cat named Whiskers, a dog named Max, and I live in Seattle.'
    );

    // Turn 2: Ask about one piece
    const response2 = await sendMessageAndWait(page, "What is my cat's name?");
    expect(response2.toLowerCase()).toContain('whiskers');

    // Turn 3: Ask about another piece
    const response3 = await sendMessageAndWait(page, "What is my dog's name?");
    expect(response3.toLowerCase()).toContain('max');

    // Turn 4: Ask about the third piece
    const response4 = await sendMessageAndWait(page, 'Where do I live?');
    expect(response4.toLowerCase()).toContain('seattle');
  });

  test('handles conversation with code context', async ({ page }) => {
    // Turn 1: Share some code
    await sendMessageAndWait(
      page,
      'Here is my Python function:\n```python\ndef greet(name):\n    return f"Hello, {name}!"\n```'
    );

    // Turn 2: Ask about the code
    const response2 = await sendMessageAndWait(page, 'What does this function do?');
    expect(response2.toLowerCase()).toMatch(/greet|hello|name|return|function/i);

    // Turn 3: Ask for modification
    const response3 = await sendMessageAndWait(
      page,
      'Can you modify it to also say goodbye?'
    );
    expect(response3.toLowerCase()).toMatch(/goodbye|bye|function|def|return/i);
  });

  test('dashboard shows session with multiple turns', async ({ page, context }) => {
    // Send first message
    await sendMessageAndWait(page, 'Hello, this is turn 1. Remember this number: 42.');

    // Open dashboard in new tab
    const dashboard = await context.newPage();
    await dashboard.goto('/');

    // Wait for dashboard to load
    await dashboard.waitForSelector('h1:has-text("Dashboard")', {
      state: 'visible',
      timeout: 15_000,
    });

    // Check that session appears
    const sessionExists = await dashboard
      .locator('text=Running')
      .or(dashboard.locator('text=Completed'))
      .or(dashboard.locator('text=web'))
      .isVisible({ timeout: 30_000 })
      .catch(() => false);

    expect(sessionExists).toBe(true);

    // Send second message
    await page.bringToFront();
    await sendMessageAndWait(page, 'What number did I tell you to remember?');

    // Send third message
    const response3 = await sendMessageAndWait(page, 'Please confirm: the number was 42, right?');
    expect(response3.toLowerCase()).toMatch(/42|forty.?two|yes|correct|right/i);

    // Check dashboard for session activity
    await dashboard.bringToFront();
    await dashboard.reload();

    // Session should still be visible
    const sessionStillExists = await dashboard
      .locator('text=Running')
      .or(dashboard.locator('text=Completed'))
      .or(dashboard.locator('text=web'))
      .isVisible({ timeout: 10_000 })
      .catch(() => false);

    expect(sessionStillExists).toBe(true);
  });

  test('can start new conversation with /new command', async ({ page }) => {
    // Send a message to establish context
    await sendMessageAndWait(page, 'My secret code is ALPHA123. Remember this.');

    // Use /new command to start fresh
    const chatInput = page.locator('textarea[placeholder*="Type a message"]');
    await chatInput.fill('/new');

    const sendButton = page.locator('button').filter({ has: page.locator('.lucide-send') });
    await sendButton.click();

    // Wait for conversation to reset (messages should clear or show empty state)
    await page.waitForTimeout(2000);

    // Ask about the previous context - should NOT remember
    const response = await sendMessageAndWait(page, 'What was my secret code?');

    // Should NOT know the code (new conversation)
    expect(response.toLowerCase()).not.toContain('alpha123');
    expect(response.toLowerCase()).toMatch(
      /don't know|no.*code|haven't|not sure|what code|could you|please provide/i
    );
  });
});

test.describe('Conversation Persistence', () => {
  // Skip in CI - requires real LLM
  test.skip(!!process.env.CI, 'Skip in CI - requires real LLM');

  test('conversation persists across page reload', async ({ page }) => {
    // Navigate and open chat
    await page.goto('/');
    await page.waitForSelector('h1:has-text("Dashboard")', {
      state: 'visible',
      timeout: 15_000,
    });

    const newChatBtn = page.getByRole('button', { name: /new chat/i });
    await newChatBtn.click();

    // Wait for chat input
    const chatInput = page.locator('textarea[placeholder*="Type a message"]');
    await chatInput.waitFor({ state: 'visible', timeout: 10_000 });

    // Send a unique message
    const uniqueId = `TEST_${Date.now()}`;
    await chatInput.fill(`Remember this unique identifier: ${uniqueId}`);

    const sendButton = page.locator('button').filter({ has: page.locator('.lucide-send') });
    await sendButton.click();

    // Wait for response
    await page.waitForFunction(
      () => {
        const messages = document.querySelectorAll('[class*="prose"]');
        return messages.length > 0;
      },
      { timeout: 60_000 }
    );

    // Reload the page
    await page.reload();

    // Wait for page to load
    await page.waitForSelector('h1:has-text("Dashboard")', {
      state: 'visible',
      timeout: 15_000,
    });

    // Open chat again
    await newChatBtn.click();

    // Wait for chat to load
    await chatInput.waitFor({ state: 'visible', timeout: 10_000 });

    // The conversation history should be loaded
    // Check if we can see the previous messages or ask about the identifier
    await page.waitForTimeout(3000); // Allow history to load

    // Either messages are visible, or we can verify by asking
    const hasHistory = await page
      .locator(`text=${uniqueId}`)
      .isVisible({ timeout: 5_000 })
      .catch(() => false);

    if (!hasHistory) {
      // Ask about the identifier
      await chatInput.fill('What was the unique identifier I told you to remember?');
      await sendButton.click();

      await page.waitForFunction(
        () => {
          const messages = document.querySelectorAll('[class*="prose"]');
          return messages.length > 1;
        },
        { timeout: 60_000 }
      );

      const messages = await page.locator('[class*="prose"]').allTextContents();
      const allText = messages.join(' ');

      // Should remember the unique ID if conversation persisted
      expect(allText).toContain(uniqueId);
    } else {
      // History is visible directly
      expect(hasHistory).toBe(true);
    }
  });
});
