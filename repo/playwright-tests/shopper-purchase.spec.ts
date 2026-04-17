import { expect, test } from '@playwright/test';

test('shopper can log in search view and buy through the UI', async ({ page }) => {
  await page.goto('/');

  await page.getByRole('button', { name: 'Shopper' }).click();
  await page.getByRole('button', { name: 'Log in' }).click();

  await expect(page.getByText('Shopper workspace')).toBeVisible();

  const searchInput = page.getByPlaceholder('search devices or keywords');
  await searchInput.fill('ThinkPad');
  await page.getByRole('button', { name: 'Search' }).click();

  const resultCard = page.locator('.result-card').first();
  await expect(resultCard).toBeVisible();
  await expect(resultCard).toContainText('ThinkPad');

  await resultCard.getByRole('button', { name: 'View' }).click();
  await expect(page.locator('.detail')).toBeVisible();

  await resultCard.getByRole('button', { name: 'Buy' }).click();

  await expect(page.locator('body')).toContainText('placed');
  await expect(page.locator('body')).toContainText('$');
});
