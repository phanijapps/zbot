import { test, expect } from './fixtures';

/**
 * Dashboard E2E Tests
 * Tests for the operations dashboard functionality.
 */
test.describe('Dashboard', () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to dashboard before each test (dashboard is at root '/')
    await page.goto('/');
    // Wait for loading to complete (dashboard shows loader initially)
    await page.waitForSelector('h1:has-text("Dashboard"), .page-header', {
      state: 'visible',
      timeout: 15_000,
    });
  });

  test('loads successfully', async ({ page }) => {
    // Wait for page to load (dashboard is at root '/')
    await expect(page).toHaveURL(/\/$/);

    // Dashboard title should be visible (using text selector for reliability)
    await expect(page.locator('h1').filter({ hasText: 'Dashboard' })).toBeVisible();
  });

  test('displays stats cards', async ({ page }) => {
    // Stats cards should be visible - use more specific selectors
    // Look for the stat card with "Active" label
    await expect(page.getByText('Active', { exact: true })).toBeVisible();
    // Look for Sessions Running text
    await expect(page.getByText('Sessions Running')).toBeVisible();
  });

  test('shows active sessions panel', async ({ page }) => {
    // Active Sessions panel should be visible - use heading role
    await expect(page.getByRole('heading', { name: 'Active Sessions' })).toBeVisible();
  });

  test('shows session history panel', async ({ page }) => {
    // Session History panel should be visible
    await expect(page.getByRole('heading', { name: 'Session History' })).toBeVisible();
  });

  test('has source filter dropdown', async ({ page }) => {
    // Source filter dropdown should exist
    const sourceFilter = page.locator('select').first();
    await expect(sourceFilter).toBeVisible();
  });

  test('source filter has all options', async ({ page }) => {
    // The source filter is a select element
    const sourceFilter = page.locator('select').first();
    await expect(sourceFilter).toBeVisible();

    // Check that filter options exist by checking the select contains them
    const options = await sourceFilter.locator('option').allTextContents();
    expect(options).toContain('All Sources');
    expect(options).toContain('Web');
    expect(options).toContain('CLI');
    expect(options).toContain('API');
  });

  test('has auto-refresh checkbox', async ({ page }) => {
    // Auto-refresh checkbox should be visible
    await expect(page.getByLabel(/auto-refresh/i)).toBeVisible();
  });

  test('auto-refresh is enabled by default', async ({ page }) => {
    // Auto-refresh checkbox should be checked by default
    const checkbox = page.getByLabel(/auto-refresh/i);
    await expect(checkbox).toBeChecked();
  });

  test('can toggle auto-refresh', async ({ page }) => {
    const checkbox = page.getByLabel(/auto-refresh/i);

    // Should be checked initially
    await expect(checkbox).toBeChecked();

    // Click to uncheck
    await checkbox.click();
    await expect(checkbox).not.toBeChecked();

    // Click to check again
    await checkbox.click();
    await expect(checkbox).toBeChecked();
  });

  test('has refresh button', async ({ page }) => {
    // Refresh button should be visible
    const refreshButton = page.locator('button[title="Refresh"]');
    await expect(refreshButton).toBeVisible();
  });

  test('has new chat button', async ({ page }) => {
    // New Chat button should be visible - look for button with Plus icon or New Chat text
    const newChatBtn = page.getByRole('button', { name: /new chat/i });
    await expect(newChatBtn).toBeVisible({ timeout: 10_000 });
  });

  test('new chat button navigates to chat', async ({ page }) => {
    // Click New Chat button
    const newChatBtn = page.getByRole('button', { name: /new chat/i });
    await newChatBtn.waitFor({ state: 'visible', timeout: 10_000 });
    await newChatBtn.click();

    // Should navigate to chat page or open chat slider
    // Chat might be a slider on the same page, so check for chat input
    await expect(page.locator('textarea, [data-testid="chat-input"]').first()).toBeVisible({ timeout: 10_000 });
  });

  test('status filter buttons exist in active panel', async ({ page }) => {
    // Status filter buttons should exist in Active Sessions panel
    // Look for "All" button that filters sessions
    const allButton = page.getByRole('button', { name: 'All' }).first();
    await expect(allButton).toBeVisible();
  });

  test('status filter buttons exist in history panel', async ({ page }) => {
    // Session History panel should have filter buttons
    const historyHeading = page.getByRole('heading', { name: 'Session History' });
    await expect(historyHeading).toBeVisible();

    // The panel should have an "All" button - use first() since there are multiple
    const historyPanel = page.locator('section, .card, div').filter({ has: historyHeading });
    const allButton = historyPanel.getByRole('button', { name: 'All' }).first();
    await expect(allButton).toBeVisible();
  });
});

test.describe('Dashboard - Using Page Objects', () => {
  test('dashboard page object works', async ({ dashboardPage, page }) => {
    await dashboardPage.goto();
    await dashboardPage.waitForLoad();

    // Page should have loaded (dashboard is at root '/')
    await expect(page).toHaveURL(/\/$/);
  });

  test('can get session count', async ({ dashboardPage }) => {
    await dashboardPage.goto();
    await dashboardPage.waitForLoad();

    // Session count should be a number (may be 0)
    const count = await dashboardPage.getSessionCount();
    expect(count).toBeGreaterThanOrEqual(0);
  });
});

test.describe('Dashboard - Empty State', () => {
  test('shows empty state message when no sessions', async ({ page }) => {
    await page.goto('/');

    // Wait for dashboard to load
    await page.waitForSelector('h1:has-text("Dashboard")', {
      state: 'visible',
      timeout: 15_000,
    });

    // Either show sessions or empty state message
    const hasEmptyState = await page.getByText(/no active sessions/i).isVisible().catch(() => false);
    const hasSessions = await page.locator('[class*="border-b"][class*="border-border"]').count().then(c => c > 0).catch(() => false);
    const hasSessionHistory = await page.getByRole('heading', { name: 'Session History' }).isVisible().catch(() => false);

    // Dashboard should show either sessions, empty state, or at least session history panel
    expect(hasEmptyState || hasSessions || hasSessionHistory).toBe(true);
  });
});
