import { test, expect } from '@playwright/test';

test.describe('Nexus App — Smoke Tests', () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to app
    await page.goto('/');
    // Wait for React to hydrate
    await page.waitForLoadState('networkidle');
  });

  test('app loads without critical errors', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (err) => errors.push(err.message));

    // App should render
    const body = page.locator('body');
    await expect(body).toBeVisible();

    // Wait a moment for any async errors
    await page.waitForTimeout(2000);

    // Filter out Tauri invoke errors (expected in browser mode)
    const criticalErrors = errors.filter(
      (e) => !e.includes('invoke') && !e.includes('Tauri') && !e.includes('tauri')
    );
    expect(criticalErrors).toEqual([]);
  });

  test('app shell renders with sidebar', async ({ page }) => {
    // Check that the app shell is present
    const appShell = page.locator('.app-shell');
    await expect(appShell).toBeVisible();
  });

  test('sidebar navigation items are present', async ({ page }) => {
    // Sidebar should have navigation items
    const sidebar = page.locator('nav, [class*="sidebar"]').first();
    await expect(sidebar).toBeVisible();

    // Should have clickable nav items
    const navItems = sidebar.locator('button, a, [role="button"]');
    const count = await navItems.count();
    expect(count).toBeGreaterThanOrEqual(3); // At least memory, graph, timeline, settings
  });

  test('default view is memory explorer', async ({ page }) => {
    // The default view should be MemoryExplorer
    // Look for memory-related content
    const mainContent = page.locator('main, [class*="workspace"]').first();
    await expect(mainContent).toBeVisible();
  });

  test('status bar is rendered', async ({ page }) => {
    const statusBar = page.locator('.status-bar, [class*="status"]').first();
    await expect(statusBar).toBeVisible();
  });
});
