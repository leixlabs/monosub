import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tsconfigPaths from "vite-tsconfig-paths"

// Tauri sets this when developing against a physical device / another host.
const host = process.env.TAURI_DEV_HOST

// https://vite.dev/config/
export default defineConfig({
  // Don't let Vite wipe the Rust compiler errors Tauri prints.
  clearScreen: false,
  server: {
    // Must match `build.devUrl` in src-tauri/tauri.conf.json.
    port: 5173,
    // Tauri waits on a fixed port, so fail loudly instead of silently
    // moving to 5174 and leaving the webview pointed at nothing.
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // `src-tauri` is watched by the Rust toolchain, not Vite.
      ignored: ['**/src-tauri/**'],
    },
  },
  // Expose `import.meta.env.TAURI_ENV_*` to the frontend.
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    // Tauri uses WebKit on macOS/Linux and Chromium (WebView2) on Windows.
    target: process.env.TAURI_ENV_PLATFORM == 'windows' ? 'chrome105' : 'safari13',
    // `TAURI_ENV_DEBUG` is set by the Tauri CLI for dev/debug builds.
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  plugins: [
    react({
      babel: {
        plugins: [
          'react-dev-locator',
        ],
      },
    }),

    tsconfigPaths()
  ],
})
