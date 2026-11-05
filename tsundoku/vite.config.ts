/// <reference types="vitest/config" />
import { defineConfig } from "vite"
import react from "@vitejs/plugin-react"
import tailwindcss from "@tailwindcss/vite"
import process from "node:process"

const host = process.env.TAURI_DEV_HOST

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [react(), tailwindcss()],

  // Tailored for Tauri, and only used by `tauri dev` and `tauri build`:
  // 1. Vite doesn't clear the screen, so Rust errors stay visible.
  clearScreen: false,
  // 2. Tauri expects a fixed port, so a busy one is an error.
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. Vite doesn't watch the Rust side.
      ignored: ["**/src-tauri/**", "**/core/**"],
    },
  },

  test: {
    include: ["src/**/*.test.ts"],
  },
}))
