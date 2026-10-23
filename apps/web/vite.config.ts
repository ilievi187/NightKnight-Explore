import { defineConfig, loadEnv } from 'vite'
import react from '@vitejs/plugin-react'
// Local development defaults to the dedicated Explore Rust API on 8792.
// Override EXPLORE_API_TARGET only when intentionally running the API elsewhere.
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, '.', 'EXPLORE_')
  const apiTarget = env.EXPLORE_API_TARGET?.trim() || 'http://127.0.0.1:8792'
  return {
  plugins: [react()],
  server: { host: '127.0.0.1', port: 5173, strictPort: true,
    proxy: { '/api': apiTarget } },
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          maplibre: ['maplibre-gl'],
        },
      },
    },
  },
  }
})