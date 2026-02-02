import { test as base, expect, Page } from '@playwright/test';

/**
 * Page Object: Dashboard
 * Encapsulates dashboard page interactions for cleaner tests.
 */
export class DashboardPage {
  constructor(private page: Page) {}

  /** Navigate to the dashboard (dashboard is at root '/') */
  async goto() {
    await this.page.goto('/');
  }

  /** Wait for dashboard to fully load */
  async waitForLoad() {
    // Wait for dashboard header or page container
    await this.page.waitForSelector('h1:has-text("Dashboard"), .page-header, .page', {
      state: 'visible',
      timeout: 15_000,
    });
  }

  /** Get the count of visible sessions */
  async getSessionCount(): Promise<number> {
    const sessions = await this.page.locator('[data-testid="session-card"], .session-card').all();
    return sessions.length;
  }

  /** Filter sessions by source */
  async filterBySource(source: string) {
    const filter = this.page.locator('[data-testid="source-filter"], .source-filter');
    if (await filter.isVisible()) {
      await filter.click();
      await this.page.click(`[data-testid="source-option-${source}"], [data-value="${source}"]`);
    }
  }

  /** Get all visible session cards */
  async getVisibleSessions() {
    return this.page.locator('[data-testid="session-card"], .session-card').all();
  }

  /** Check if stats panel is visible */
  async isStatsPanelVisible(): Promise<boolean> {
    const statsPanel = this.page.locator('[data-testid="stats-panel"], .stats-panel, .stats');
    return statsPanel.isVisible();
  }

  /** Get a specific stat value by label */
  async getStatValue(label: string): Promise<string | null> {
    const stat = this.page.locator(`text=${label}`).locator('..').locator('.stat-value, [data-testid="stat-value"]');
    if (await stat.isVisible()) {
      return stat.textContent();
    }
    return null;
  }
}

/**
 * Page Object: Chat
 * Encapsulates chat page interactions.
 */
export class ChatPage {
  constructor(private page: Page) {}

  /** Navigate to chat with optional conversation ID */
  async goto(conversationId?: string) {
    if (conversationId) {
      await this.page.goto(`/chat/${conversationId}`);
    } else {
      await this.page.goto('/');
    }
  }

  /** Wait for chat to be ready */
  async waitForReady() {
    await this.page.waitForSelector('[data-testid="chat-input"], .chat-input, textarea', {
      state: 'visible',
      timeout: 10_000,
    });
  }

  /** Send a message */
  async sendMessage(message: string) {
    const input = this.page.locator('[data-testid="chat-input"], .chat-input, textarea').first();
    await input.fill(message);
    
    const sendButton = this.page.locator('[data-testid="send-button"], button[type="submit"], .send-button');
    await sendButton.click();
  }

  /** Wait for assistant response */
  async waitForResponse(timeout = 30_000): Promise<string | null> {
    const response = this.page.locator('[data-testid="assistant-message"], .assistant-message, .message-assistant').last();
    await response.waitFor({ state: 'visible', timeout });
    return response.textContent();
  }

  /** Get all messages */
  async getMessages() {
    return this.page.locator('[data-testid="message"], .message').all();
  }

  /** Check if chat is loading/thinking */
  async isLoading(): Promise<boolean> {
    const loader = this.page.locator('[data-testid="loading"], .loading, .thinking');
    return loader.isVisible();
  }
}

/**
 * Page Object: Settings
 * Encapsulates settings page interactions.
 */
export class SettingsPage {
  constructor(private page: Page) {}

  /** Navigate to settings */
  async goto() {
    await this.page.goto('/settings');
  }

  /** Navigate to a specific settings tab */
  async gotoTab(tab: 'agents' | 'providers' | 'mcps' | 'skills' | 'tools') {
    await this.page.goto(`/settings/${tab}`);
  }

  /** Wait for settings page to load */
  async waitForLoad() {
    await this.page.waitForSelector('[data-testid="settings"], .settings-page, main', {
      state: 'visible',
      timeout: 10_000,
    });
  }
}

// Extend base test with custom fixtures
export const test = base.extend<{
  dashboardPage: DashboardPage;
  chatPage: ChatPage;
  settingsPage: SettingsPage;
}>({
  dashboardPage: async ({ page }, use) => {
    const dashboard = new DashboardPage(page);
    await use(dashboard);
  },
  chatPage: async ({ page }, use) => {
    const chat = new ChatPage(page);
    await use(chat);
  },
  settingsPage: async ({ page }, use) => {
    const settings = new SettingsPage(page);
    await use(settings);
  },
});

// Re-export expect for convenience
export { expect };
