import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  worker: 'process',
  reporter: 'list',
  use: {
    baseURL: 'http://127.0.0.1:5199',
    headless: true,
    viewport: { width: 1440, height: 900 },
    ignoreHTTPSErrors: true,
  },
})
