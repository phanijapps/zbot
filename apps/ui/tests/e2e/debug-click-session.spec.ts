import { test, expect } from '@playwright/test';

/**
 * Test clicking on a session in the history to verify the messages endpoint
 */

test('click session row to open chat', async ({ page }) => {
  // Collect all network requests
  const requests: { method: string; url: string; status?: number }[] = [];

  page.on('request', (request) => {
    if (request.url().includes('/api/')) {
      requests.push({ method: request.method(), url: request.url() });
    }
  });

  page.on('response', (response) => {
    if (response.url().includes('/api/')) {
      const req = requests.find((r) => r.url === response.url() && !r.status);
      if (req) {
        req.status = response.status();
      }
      console.log(`${response.status()} ${response.url()}`);
    }
  });

  await page.goto('/');

  // Wait for dashboard to load
  await page.waitForSelector('h1:has-text("Dashboard")', {
    state: 'visible',
    timeout: 15_000,
  });

  // Wait for sessions to load
  await page.waitForTimeout(3000);

  // Take screenshot of initial state
  await page.screenshot({ path: 'test-results/debug-1-dashboard.png' });

  // Find and click on the "Open chat" button (MessageSquare icon button)
  const chatButton = page.locator('button[title="Open chat"]').first();

  const hasButton = await chatButton.isVisible().catch(() => false);

  if (hasButton) {
    console.log('Found Open chat button, clicking...');
    await chatButton.click();

    // Wait for any API calls
    await page.waitForTimeout(2000);

    // Take screenshot after click
    await page.screenshot({ path: 'test-results/debug-2-after-click.png' });

    // Check for 404 responses
    const failed = requests.filter((r) => r.status === 404);
    if (failed.length > 0) {
      console.log('=== 404 ERRORS ===');
      failed.forEach((r) => console.log(`404: ${r.url}`));
    }

    // Check if chat panel opened
    const chatPanel = page.locator('[class*="chat"], [class*="slide"], [class*="panel"]');
    const chatVisible = await chatPanel.first().isVisible().catch(() => false);
    console.log('Chat panel visible:', chatVisible);
  } else {
    console.log('No session row found');

    // List all visible text on page
    const bodyText = await page.locator('body').textContent();
    console.log('Page content sample:', bodyText?.substring(0, 500));
  }

  // Print all API requests
  console.log('\n=== ALL API REQUESTS ===');
  requests.forEach((r) => {
    const status = r.status || 'pending';
    const icon = status === 404 ? '❌' : status === 200 ? '✓' : '?';
    console.log(`${icon} ${status} ${r.method} ${r.url}`);
  });
});
