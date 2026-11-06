/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// The Tauri CLI runs `pnpm dev` and points the window at this port (tauri.conf.json devUrl).
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    // The webviews Tauri 2 targets: WebView2 on Windows, WKWebView on macOS 10.15+.
    target: ["es2021", "chrome105", "safari15"],
    outDir: "dist",
    emptyOutDir: true,
  },
  test: {
    include: ["src/**/*.test.ts"],
  },
});
