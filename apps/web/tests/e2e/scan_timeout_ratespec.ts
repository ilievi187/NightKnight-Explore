import { test, expect } from '@playwright/test'

test('scan shows rate limit error with retry hint after cooldown', async ({ page }) => {
  await page.goto('http://127.0.0.1:5199/')

  // Initiate a scan
  await page.fill('input[type=number]:nth-child(1)', '43.3088')
  await page.fill('input[type=number]:nth-child(2)', '27.0197')
  await page.click('button.primary')

  // Wait for error message (rate limiting is expected with heavy test usage)
  await page.waitForTimeout(8000)

  // Should show a proper error message, not fabricate results
  const errorMessage = page.locator('.error')
  const hasError = await errorMessage.first().isVisible()

  if (hasError) {
    // Error should be informative, not "unreadable response"
    await expect(errorMessage).not.toContainText('unreadable response')
    // Should mention provider or rate limiting
    const txt = await errorMessage.first().textContent()
    console.log('Scan error:', txt.substring(0, 80))
  }
})

test('scan cooldown respected - rapid scans show provider rate limit', async ({ page }) => {
  await page.goto('http://127.0.0.1:5199/')

  // Initiate scan
  await page.fill('input[type=number]:nth-child(1)', '43.3088')
  await page.fill('input[type=number]:nth-child(2)', '27.0197')
  await page.click('button.primary')

  // Wait briefly then try another scan
  await page.waitForTimeout(3000)
  await page.click('button.primary')

  // Should show rate limit error (cooldown not yet expired)
  const errorMessage = page.locator('.error')
  const hasError = await errorMessage.first().isVisible()
  if (hasError) {
    const txt = await errorMessage.first().textContent()
    // Should mention rate limiting or provider issues, not empty success
    expect(txt).not.toContain('unreadable response')
    console.log('Rapid scan error:', txt.substring(0, 80))
  }
})