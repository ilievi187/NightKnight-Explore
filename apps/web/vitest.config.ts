import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    include: ['src/**/*.test.ts', 'src/**/*.spec.ts'],
    exclude: ['tests/e2e/**'],
  },
})
