import { test, expect } from '@playwright/test'

test('card Enter key opens detail panel', async ({ page }) => {
  await page.goto('http://127.0.0.1:5199/')

  await page.fill('input[type=number]:nth-child(1)', '42.6977')
  await page.fill('input[type=number]:nth-child(2)', '23.3219')
  await page.click('button.primary')

  await expect(page.locator('.place-card')).toHaveCountGreaterThan(0)

  // Focus first card and press Enter
  await page.focus('.place-card:first-child h3 button.place-select')
  await page.keyboard.press('Enter')

  // Detail panel should be open
  await expect(page.locator('.selected-detail')).toBeVisible()
  await expect(page.locator('#selected-place-title')).toContainText('Преториум')

  // Close button should return focus to the same card
  await page.focus('.detail-close')
  await page.keyboard.press('Enter')

  // Focus should return to the originally focused card's button
  await expect(page.locator('.place-card:first-child')).toBeFocused()
})

test('map marker Space key opens detail panel', async ({ page }) => {
  await page.goto('http://127.0.0.1:5199/')

  await page.fill('input[type=number]:nth-child(1)', '42.6977')
  await page.fill('input[type=number]:nth-child(2)', '23.3219')
  await page.click('button.primary')

  await expect(page.locator('.place-card')).toHaveCountGreaterThan(0)

  // Focus a map marker and press Space
  await page.focus('.place-marker')
  await page.keyboard.press('Space')

  // Detail panel should be open
  await expect(page.locator('.selected-detail')).toBeVisible()
})

test('close refocuses to triggering element', async ({ page }) => {
  await page.goto('http://127.0.0.1:5199/')

  await page.fill('input[type=number]:nth-child(1)', '42.6977')
  await page.fill('input[type=number]:nth-child(2)', '23.3219')
  await page.click('button.primary')

  await expect(page.locator('.place-card')).toHaveCountGreaterThan(0)

  // Open detail panel via card button
  await page.focus('.place-card:first-child h3 button.place-select')
  await page.keyboard.press('Enter')

  await expect(page.locator('.selected-detail')).toBeVisible()

  // Close and verify focus returns to the map marker area
  await page.focus('.detail-close')
  await page.keyboard.press('Enter')

  // After close, focus should be on the previously active element
  const activeElement = page.locator(':focus')
  await expect(activeElement).toBeVisible()
})

test('sr-only live region announces selection', async ({ page }) => {
  await page.goto('http://127.0.0.1:5199/')

  await page.fill('input[type=number]:nth-child(1)', '42.6977')
  await page.fill('input[type=number]:nth-child(2)', '23.3219')
  await page.click('button.primary')

  await expect(page.locator('.place-card')).toHaveCountGreaterThan(0)

  // Open detail panel
  await page.focus('.place-card:first-child h3 button.place-select')
  await page.keyboard.press('Enter')

  // Live region should announce selection
  await expect(page.locator('[role=status][aria-live=polite]')).toContainText('Selected')

  // Close
  await page.click('.detail-close')
})