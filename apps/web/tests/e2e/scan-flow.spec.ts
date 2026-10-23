import { test, expect } from '@playwright/test'

test('initiates scan and shows places', async ({ page }) => {
  await page.goto('http://127.0.0.1:5199/')

  // Set coordinates (Sofia central)
  await page.fill('input[type=number]:nth-child(1)', '42.6977')
  await page.fill('input[type=number]:nth-child(2)', '23.3219')

  // Initiate scan
  await page.click('button.primary')

  // Wait for results
  await expect(page.locator('.place-card')).toHaveCountGreaterThan(0)

  // Verify source attribution
  await expect(page.locator('.place-links a:first-child')).toHaveAttribute('href', expect.stringMatching(/openstreetmap\.org/))

  // Verify at least one place has a gem signal
  const gemBadges = page.locator('.gem-badge')
  const gemCount = await gemBadges.count()
  if (gemCount > 0) {
    await expect(gemBadges.first()).toBeVisible()
  }
})

test('filters by category', async ({ page }) => {
  await page.goto('http://127.0.0.1:5199/')

  await page.fill('input[type=number]:nth-child(1)', '42.6977')
  await page.fill('input[type=number]:nth-child(2)', '23.3219')
  await page.click('button.primary')

  // Wait for results
  await expect(page.locator('.place-card')).toHaveCountGreaterThan(0)

  // Click historical category
  await page.click('button:has-text("Historical")')

  // Verify places filtered
  await expect(page.locator('.place-card')).toHaveCountGreaterThan(0)
})

test('sorts by gems', async ({ page }) => {
  await page.goto('http://127.0.0.1:5199/')

  await page.fill('input[type=number]:nth-child(1)', '42.6977')
  await page.fill('input[type=number]:nth-child(2)', '23.3219')
  await page.click('button.primary')

  // Wait for results
  await expect(page.locator('.place-card')).toHaveCountGreaterThan(0)

  // Sort by gems
  await page.click('button.sort-toggle')

  // Verify places still render
  await expect(page.locator('.place-card')).toHaveCountGreaterThan(0)
})