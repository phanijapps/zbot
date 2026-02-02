import { test, expect } from './fixtures';

/**
 * Smoke tests - verify basic app functionality.
 * These tests should always pass and run quickly.
 */
test.describe('Smoke Tests', () => {
  test('app loads successfully', async ({ page }) => {
    await page.goto('/');

    // Should have a title
    await expect(page).toHaveTitle(/AgentZero|Agent/i);
  });

  test('navigation works', async ({ page }) => {
    await page.goto('/');

    // Page should be visible
    await expect(page.locator('body')).toBeVisible();
  });

  test('can navigate to dashboard', async ({ page }) => {
    // Dashboard is at root '/'
    await page.goto('/');

    // Should navigate successfully (no error)
    await expect(page).toHaveURL(/\/$/);
  });

  test('can navigate to settings', async ({ page }) => {
    await page.goto('/settings');

    // Should navigate successfully
    await expect(page).toHaveURL(/settings/);
  });
});

test.describe('Dashboard Page', () => {
  test('dashboard loads', async ({ dashboardPage, page }) => {
    await dashboardPage.goto();

    // Dashboard is at root '/'
    await expect(page).toHaveURL(/\/$/);
  });
});

test.describe('Chat Page', () => {
  test('chat page loads', async ({ chatPage, page }) => {
    await chatPage.goto();

    // Should load successfully
    await expect(page.locator('body')).toBeVisible();
  });
});
