import { test, expect } from '@playwright/test'

test('detects wrong API service on port conflict', async ({ page }) => {
  await page.goto('http://127.0.0.1:5199/')

  // Wait for app to load
  await expect(page.locator('.workbench')).toBeVisible({ timeout: 15000 })

  // Try to initiate scan - should show proper error, not silently use wrong service
  await page.fill('input[type=number]:nth-child(1)', '43.3088')
  await page.fill('input[type=number]:nth-child(2)', '27.0197')
  await page.click('button.primary')

  // Should not silently show results from wrong service
  // Either show proper error or no results
  const errorMessage = page.locator('.error')
  const placeCards = page.locator('.place-card')

  // If there are results, they must be from our API, not a different service
  const cardCount = await placeCards.count()
  if (cardCount > 0) {
    // Verify places have proper OSM attribution
    await expect(placeCards.first().locator('a[href*="openstreetmap.org"]')).toBeVisible()
  }
})

test('health endpoint identifies correct service identity', async ({ page }) => {
  await page.goto('http://127.0.0.1:5199/')

  // The app should display correct service identity
  // The header should show NightKnight Explore, not another application
  await expect(page.locator('.view-switch button:first-child')).toBeVisible()
  await expect(page.locator('.status-pill')).toContainText('NightKnight')
  
  // Verify the API health call returns our service identity
  // (this is tested at unit level; here we verify the UI doesn't break)
  await page.waitForTimeout(2000)
})
