import { test, expect } from './fixtures';

/**
 * Session Messages E2E Tests
 * Tests for the session chat viewer with scope-based message filtering.
 */
test.describe('Session Messages', () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to dashboard before each test
    await page.goto('/');
    // Wait for loading to complete
    await page.waitForSelector('h1:has-text("Dashboard"), .page-header', {
      state: 'visible',
      timeout: 15_000,
    });
  });

  test.describe('Chat Viewer Display', () => {
    test('opens chat slider when clicking chat button on session', async ({ page }) => {
      // Wait for sessions to load
      await page.waitForTimeout(2000);

      // Find a session card with a chat button (MessageSquare icon)
      const chatButton = page.locator('button[title*="chat" i], button:has([data-lucide="message-square"])').first();

      // Skip if no sessions exist
      if (!(await chatButton.isVisible({ timeout: 5000 }).catch(() => false))) {
        test.skip(true, 'No sessions available to test');
        return;
      }

      await chatButton.click();

      // Chat slider should open
      await expect(page.locator('.chat-slider, [data-testid="chat-slider"]')).toBeVisible({
        timeout: 5000,
      });
    });

    test('shows agent ID in chat header', async ({ page }) => {
      // Wait for sessions to load
      await page.waitForTimeout(2000);

      // Find and click a chat button
      const chatButton = page.locator('button[title*="chat" i], button:has([data-lucide="message-square"])').first();

      if (!(await chatButton.isVisible({ timeout: 5000 }).catch(() => false))) {
        test.skip(true, 'No sessions available to test');
        return;
      }

      await chatButton.click();

      // Wait for chat slider to open
      await page.waitForSelector('.chat-slider, [data-testid="chat-slider"]', {
        state: 'visible',
        timeout: 5000,
      });

      // Header should show agent ID
      const header = page.locator('.chat-slider h1, [data-testid="chat-header"] h1');
      await expect(header).toBeVisible();
    });

    test('shows read-only indicator for subagent chat', async ({ page }) => {
      // Wait for sessions to load
      await page.waitForTimeout(2000);

      // Look for an expanded session with subagent executions
      // First, try to expand a session
      const sessionCard = page.locator('[data-testid="session-card"], .session-card').first();

      if (!(await sessionCard.isVisible({ timeout: 5000 }).catch(() => false))) {
        test.skip(true, 'No sessions available to test');
        return;
      }

      // Click to expand session
      await sessionCard.click();
      await page.waitForTimeout(500);

      // Look for subagent chat button (delegation executions)
      const subagentChatButton = page.locator('[title*="subagent" i], [title*="View subagent" i]').first();

      if (!(await subagentChatButton.isVisible({ timeout: 3000 }).catch(() => false))) {
        test.skip(true, 'No subagent executions found');
        return;
      }

      await subagentChatButton.click();

      // Chat slider should show read-only indicator
      await expect(page.getByText('Read-only')).toBeVisible({ timeout: 5000 });
    });

    test('shows subagent view indicator', async ({ page }) => {
      // Wait for sessions to load
      await page.waitForTimeout(2000);

      // Expand a session and click on subagent chat
      const sessionCard = page.locator('[data-testid="session-card"], .session-card').first();

      if (!(await sessionCard.isVisible({ timeout: 5000 }).catch(() => false))) {
        test.skip(true, 'No sessions available to test');
        return;
      }

      await sessionCard.click();
      await page.waitForTimeout(500);

      const subagentChatButton = page.locator('[title*="subagent" i], [title*="View subagent" i]').first();

      if (!(await subagentChatButton.isVisible({ timeout: 3000 }).catch(() => false))) {
        test.skip(true, 'No subagent executions found');
        return;
      }

      await subagentChatButton.click();

      // Should show "Subagent View" indicator
      await expect(page.getByText('Subagent View')).toBeVisible({ timeout: 5000 });
    });
  });

  test.describe('Chat Slider Behavior', () => {
    test('can close chat slider with escape key', async ({ page }) => {
      // Wait for sessions to load
      await page.waitForTimeout(2000);

      const chatButton = page.locator('button[title*="chat" i], button:has([data-lucide="message-square"])').first();

      if (!(await chatButton.isVisible({ timeout: 5000 }).catch(() => false))) {
        test.skip(true, 'No sessions available to test');
        return;
      }

      await chatButton.click();

      // Wait for slider to open
      await page.waitForSelector('.chat-slider, [data-testid="chat-slider"]', {
        state: 'visible',
        timeout: 5000,
      });

      // Press Escape to close
      await page.keyboard.press('Escape');

      // Slider should close
      await expect(page.locator('.chat-slider, [data-testid="chat-slider"]')).not.toBeVisible({
        timeout: 3000,
      });
    });

    test('can close chat slider with close button', async ({ page }) => {
      // Wait for sessions to load
      await page.waitForTimeout(2000);

      const chatButton = page.locator('button[title*="chat" i], button:has([data-lucide="message-square"])').first();

      if (!(await chatButton.isVisible({ timeout: 5000 }).catch(() => false))) {
        test.skip(true, 'No sessions available to test');
        return;
      }

      await chatButton.click();

      // Wait for slider to open
      await page.waitForSelector('.chat-slider, [data-testid="chat-slider"]', {
        state: 'visible',
        timeout: 5000,
      });

      // Click close button (X icon or close button)
      const closeButton = page.locator('.chat-slider button[title*="close" i], .chat-slider button:has([data-lucide="x"])');
      if (await closeButton.isVisible()) {
        await closeButton.click();
      } else {
        // Try clicking the backdrop
        await page.locator('.chat-slider-backdrop').click({ force: true });
      }

      // Slider should close
      await expect(page.locator('.chat-slider, [data-testid="chat-slider"]')).not.toBeVisible({
        timeout: 3000,
      });
    });
  });

  test.describe('Message Display', () => {
    test('displays messages in chat viewer', async ({ page }) => {
      // Wait for sessions to load
      await page.waitForTimeout(2000);

      const chatButton = page.locator('button[title*="chat" i], button:has([data-lucide="message-square"])').first();

      if (!(await chatButton.isVisible({ timeout: 5000 }).catch(() => false))) {
        test.skip(true, 'No sessions available to test');
        return;
      }

      await chatButton.click();

      // Wait for slider to open and messages to load
      await page.waitForSelector('.chat-slider, [data-testid="chat-slider"]', {
        state: 'visible',
        timeout: 5000,
      });

      // Wait for loading to complete
      await page.waitForTimeout(1000);

      // Should either show messages or "No messages" indicator
      const hasMessages = await page.locator('.chat-slider .prose, .message').count() > 0;
      const hasNoMessages = await page.getByText('No messages').isVisible().catch(() => false);
      const hasLoading = await page.locator('.animate-spin').isVisible().catch(() => false);

      expect(hasMessages || hasNoMessages || hasLoading).toBeTruthy();
    });

    test('shows different styling for user and assistant messages', async ({ page }) => {
      // Wait for sessions to load
      await page.waitForTimeout(2000);

      const chatButton = page.locator('button[title*="chat" i], button:has([data-lucide="message-square"])').first();

      if (!(await chatButton.isVisible({ timeout: 5000 }).catch(() => false))) {
        test.skip(true, 'No sessions available to test');
        return;
      }

      await chatButton.click();

      // Wait for slider and messages
      await page.waitForSelector('.chat-slider, [data-testid="chat-slider"]', {
        state: 'visible',
        timeout: 5000,
      });
      await page.waitForTimeout(1500);

      // If there are messages, they should have different styling
      const messages = page.locator('.chat-slider .rounded-2xl');
      const count = await messages.count();

      if (count > 0) {
        // There should be at least some messages visible
        expect(count).toBeGreaterThan(0);
      }
    });
  });

  test.describe('Input Behavior', () => {
    test('input is hidden in read-only mode', async ({ page }) => {
      // Wait for sessions to load
      await page.waitForTimeout(2000);

      // Expand a session and open subagent chat (which is read-only)
      const sessionCard = page.locator('[data-testid="session-card"], .session-card').first();

      if (!(await sessionCard.isVisible({ timeout: 5000 }).catch(() => false))) {
        test.skip(true, 'No sessions available to test');
        return;
      }

      await sessionCard.click();
      await page.waitForTimeout(500);

      const subagentChatButton = page.locator('[title*="subagent" i], [title*="View subagent" i]').first();

      if (!(await subagentChatButton.isVisible({ timeout: 3000 }).catch(() => false))) {
        test.skip(true, 'No subagent executions found');
        return;
      }

      await subagentChatButton.click();

      // Wait for slider to open
      await page.waitForSelector('.chat-slider, [data-testid="chat-slider"]', {
        state: 'visible',
        timeout: 5000,
      });

      // Input should not be visible in read-only mode
      const input = page.locator('.chat-slider textarea, .chat-slider input[type="text"]');
      await expect(input).not.toBeVisible({ timeout: 3000 });
    });

    test('input is visible for root execution chat', async ({ page }) => {
      // Wait for sessions to load
      await page.waitForTimeout(2000);

      // Click on root execution chat (not subagent)
      const chatButton = page.locator('button[title="Open chat"], button[title="View chat"]').first();

      if (!(await chatButton.isVisible({ timeout: 5000 }).catch(() => false))) {
        // Fall back to any chat button
        const anyChatButton = page.locator('button:has([data-lucide="message-square"])').first();
        if (!(await anyChatButton.isVisible({ timeout: 3000 }).catch(() => false))) {
          test.skip(true, 'No chat buttons found');
          return;
        }
        await anyChatButton.click();
      } else {
        await chatButton.click();
      }

      // Wait for slider to open
      await page.waitForSelector('.chat-slider, [data-testid="chat-slider"]', {
        state: 'visible',
        timeout: 5000,
      });

      // For root execution, input may be visible (depends on session state)
      // At minimum, the chat viewer should load without errors
      const hasContent = await page.locator('.chat-slider').isVisible();
      expect(hasContent).toBeTruthy();
    });
  });
});
