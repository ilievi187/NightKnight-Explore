import { test, expect } from '@playwright/test'

test('saves a place from detail panel', async ({ page }) => {
  await page.goto('http://127.0.0.1:5199/')

  await page.fill('input[type=number]:nth-child(1)', '42.6977')
  await page.fill('input[type=number]:nth-child(2)', '23.3219')
  await page.click('button.primary')

  await expect(page.locator('.place-card')).toHaveCountGreaterThan(0)

  // Open detail panel
  await page.focus('.place-card:first-child h3 button.place-select')
  await page.keyboard.press('Enter')

  await expect(page.locator('.selected-detail')).toBeVisible()

  // Save the place
  await page.click('button.favorite-toggle')

  // Verify favorites count increased
  await expect(page.locator('.view-switch button span')).toContainText('1')
})

test('removes a saved place', async ({ page }) => {
  await page.goto('http://127.0.0.1:5199/')

  await page.fill('input[type=number]:nth-child(1)', '42.6977')
  await page.fill('input[type=number]:nth-child(2)', '23.3219')
  await page.click('button.primary')

  // Save a place first
  await page.focus('.place-card:first-child h3 button.place-select')
  await page.keyboard.press('Enter')
  await page.click('button.favorite-toggle')

  // Go to saved places view
  await page.click('button:has-text("Saved places")')

  // Remove the saved place
  await page.click('button:has-text("Remove")')

  // Favorites count should be back to 0
  await expect(page.locator('.view-switch button span')).toContainText('0')
})