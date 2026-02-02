import { test, expect } from '@playwright/test';

/**
 * Debug tests for reported UI issues:
 * 1. Chat in session history shows 404 for messages
 * 2. Executions running shows incorrect count
 * 3. Crashed sessions show as running when expanded
 */

test.describe('Debug: Session History Issues', () => {
  test.beforeEach(async ({ page }) => {
    // Collect all network requests
    page.on('request', (request) => {
      if (request.url().includes('/api/')) {
        console.log('>> REQUEST:', request.method(), request.url());
      }
    });

    page.on('response', (response) => {
      if (response.url().includes('/api/')) {
        console.log('<< RESPONSE:', response.status(), response.url());
      }
    });

    // Collect console errors
    page.on('console', (msg) => {
      if (msg.type() === 'error') {
        console.log('CONSOLE ERROR:', msg.text());
      }
    });
  });

  test('Issue 1: Click on session in history - check for 404', async ({ page }) => {
    await page.goto('/');

    // Wait for dashboard to load
    await page.waitForSelector('h1:has-text("Dashboard")', {
      state: 'visible',
      timeout: 15_000,
    });

    // Wait for sessions to load
    await page.waitForTimeout(2000);

    // Look for any session card or row in the session list
    const sessionElements = await page.locator('[class*="session"]').all();
    console.log(`Found ${sessionElements.length} session elements`);

    // Try to find clickable session items
    const clickableSession = page
      .locator('button, [role="button"], tr, [class*="cursor-pointer"]')
      .filter({ hasText: /sess-|exec-|root|web|cli/i })
      .first();

    const isVisible = await clickableSession.isVisible().catch(() => false);

    if (isVisible) {
      console.log('Found clickable session, clicking...');

      // Listen for the specific 404 request
      const responsePromise = page.waitForResponse(
        (response) => response.url().includes('/messages') || response.url().includes('/executions/exec-'),
        { timeout: 10_000 }
      ).catch(() => null);

      await clickableSession.click();

      const response = await responsePromise;
      if (response) {
        console.log('Messages API Response:', response.status(), response.url());
        if (response.status() === 404) {
          console.log('CONFIRMED: 404 error on messages endpoint');
          // Get the URL pattern to understand what endpoint is being called
          const url = response.url();
          console.log('Failed URL pattern:', url);
        }
      }

      // Wait to see the chat panel
      await page.waitForTimeout(2000);

      // Check if there's a 404 displayed
      const has404 = await page.locator('text=404').isVisible().catch(() => false);
      console.log('404 visible in UI:', has404);
    } else {
      console.log('No clickable session found - check if there are any sessions');

      // Let's see what's on the page
      const pageContent = await page.content();
      console.log('Page contains "session":', pageContent.includes('session'));
      console.log('Page contains "Active":', pageContent.includes('Active'));
    }
  });

  test('Issue 2: Check executions running count accuracy', async ({ page }) => {
    await page.goto('/');

    await page.waitForSelector('h1:has-text("Dashboard")', {
      state: 'visible',
      timeout: 15_000,
    });

    // Capture the stats API response
    const statsResponse = await page.waitForResponse(
      (response) => response.url().includes('/stats/counts'),
      { timeout: 10_000 }
    ).catch(() => null);

    if (statsResponse) {
      const stats = await statsResponse.json();
      console.log('Stats API Response:', JSON.stringify(stats, null, 2));

      // Check what the UI shows
      await page.waitForTimeout(1000);

      // Look for the running count in the UI
      const runningText = await page.locator('text=/Running|running/').allTextContents();
      console.log('Running text in UI:', runningText);

      // Look for numbers near "Running" or "Active"
      const statsPanel = page.locator('[class*="stats"], [class*="panel"], [class*="card"]').first();
      const statsPanelText = await statsPanel.textContent().catch(() => '');
      console.log('Stats panel content:', statsPanelText);

      // Check if executions_running in API matches reality
      console.log('API says executions_running:', stats.executions_running);
      console.log('API says sessions_running:', stats.sessions_running);
    }

    // Also check the sessions list endpoint
    const sessionsResponse = await page.waitForResponse(
      (response) => response.url().includes('/sessions'),
      { timeout: 5_000 }
    ).catch(() => null);

    if (sessionsResponse) {
      const sessions = await sessionsResponse.json();
      console.log('Sessions API Response count:', Array.isArray(sessions) ? sessions.length : 'not array');

      if (Array.isArray(sessions)) {
        const runningSessions = sessions.filter(
          (s: any) => s.session?.status === 'running' || s.status === 'running'
        );
        console.log('Actually running sessions:', runningSessions.length);

        // Count running executions
        let runningExecs = 0;
        sessions.forEach((s: any) => {
          if (s.executions) {
            runningExecs += s.executions.filter((e: any) => e.status === 'running').length;
          }
        });
        console.log('Actually running executions:', runningExecs);
      }
    }
  });

  test('Issue 3: Crashed sessions showing as running when expanded', async ({ page }) => {
    await page.goto('/');

    await page.waitForSelector('h1:has-text("Dashboard")', {
      state: 'visible',
      timeout: 15_000,
    });

    await page.waitForTimeout(2000);

    // Get sessions data
    const sessionsResponse = await page.waitForResponse(
      (response) => response.url().includes('/sessions'),
      { timeout: 10_000 }
    ).catch(() => null);

    let crashedSessions: any[] = [];
    if (sessionsResponse) {
      const sessions = await sessionsResponse.json();
      if (Array.isArray(sessions)) {
        crashedSessions = sessions.filter(
          (s: any) => s.session?.status === 'crashed' || s.status === 'crashed'
        );
        console.log('Crashed sessions from API:', crashedSessions.length);
        crashedSessions.forEach((s: any, i: number) => {
          console.log(`Crashed session ${i}:`, {
            id: s.session?.id || s.id,
            status: s.session?.status || s.status,
            executions: s.executions?.map((e: any) => ({
              id: e.id,
              status: e.status,
              agent_id: e.agent_id,
            })),
          });
        });
      }
    }

    // Try to find and expand a crashed session in the UI
    const crashedBadge = page.locator('text=/crashed/i').first();
    const hasCrashed = await crashedBadge.isVisible().catch(() => false);

    if (hasCrashed) {
      console.log('Found crashed badge in UI');

      // Find the parent row/card and click to expand
      const crashedRow = crashedBadge.locator('xpath=ancestor::tr | ancestor::div[contains(@class, "card")]').first();

      // Look for expand button or click the row
      const expandButton = crashedRow.locator('button, [class*="expand"], [class*="chevron"]').first();
      const hasExpand = await expandButton.isVisible().catch(() => false);

      if (hasExpand) {
        await expandButton.click();
        console.log('Clicked expand button');
      } else {
        await crashedRow.click();
        console.log('Clicked crashed row');
      }

      await page.waitForTimeout(1000);

      // Check what status is shown in the expanded view
      const expandedContent = await page.locator('[class*="expanded"], [class*="detail"], [class*="execution"]').allTextContents();
      console.log('Expanded content:', expandedContent);

      // Look for status indicators in expanded view
      const runningInExpanded = await page.locator('text=/running/i').count();
      const crashedInExpanded = await page.locator('text=/crashed/i').count();
      console.log('Running count in expanded:', runningInExpanded);
      console.log('Crashed count in expanded:', crashedInExpanded);
    } else {
      console.log('No crashed sessions visible in UI');
    }
  });

  test('Debug: List all API endpoints and their responses', async ({ page }) => {
    const apiCalls: { url: string; status: number; data?: any }[] = [];

    page.on('response', async (response) => {
      if (response.url().includes('/api/')) {
        const data = await response.json().catch(() => null);
        apiCalls.push({
          url: response.url(),
          status: response.status(),
          data: data,
        });
      }
    });

    await page.goto('/');
    await page.waitForTimeout(5000);

    console.log('\n=== API CALLS MADE ===');
    apiCalls.forEach((call) => {
      console.log(`\n${call.status} ${call.url}`);
      if (call.data) {
        console.log('Response:', JSON.stringify(call.data, null, 2).substring(0, 500));
      }
    });
  });
});
