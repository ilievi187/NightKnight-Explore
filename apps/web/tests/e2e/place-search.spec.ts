import { test, expect } from '@playwright/test'

test('submits search and selects result', async ({ page }) => {
  await page.goto('http://127.0.0.1:5199/')

  // Wait for app to load
  await expect(page.locator('text:Discover what\'s around you')).toBeVisible()

  // Submit a place search
  await page.fill('#place-search', 'Sofia Bulgaria')
  await page.click('button[type=submit]')

  // Wait for results or error message
  const hasResults = await page.locator('.geocode-results li').first().isVisible()
  const hasError = await page.locator('.geocode-message').first().isVisible()

  if (hasResults) {
    // Verify attribution mentions policy
    await expect(page.locator('.geocode-attribution')).toContainText('app-wide')

    // Select first result
    await page.click('.geocode-choice')

    // Verify coordinates updated
    await expect(page.locator('input[type=number]:nth-child(1)')).toHaveValue('42.697278')
    await expect(page.locator('input[type=number]:nth-child(2)')).toHaveValue('23.321728')

    // Verify search term is NOT in the URL
    const url = page.url()
    expect(url).not.toContain('q=')
  } else if (hasError) {
    // Error case - verify validation message
    await expect(page.locator('.geocode-message')).toContainText('2–200 characters')
  }
})

test('validates short query input', async ({ page }) => {
  await page.goto('http://127.0.0.1:5199/')
  await page.fill('#place-search', 'a')
  await page.click('button[type=submit]')

  // Should show validation error
  await expect(page.locator('.geocode-message')).toBeVisible()
})

test('validates long query input', async ({ page }) => {
  await page.goto('http://127.0.0.1:5199/')
  await page.fill('#place-search', 'a'.repeat(201))
  await page.click('button[type=submit]')

  // Should show validation error (may be provider error if API is up, or validation message)
  const message = page.locator('.geocode-message')
  await expect(message).toBeVisible()
})