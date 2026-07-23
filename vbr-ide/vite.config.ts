import { defineConfig } from "vite";

// Tauri drives the dev server, so pin the port and keep its logs on screen.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: "es2020",
    // Monaco is a large dependency by nature; don't nag about chunk size.
    chunkSizeWarningLimit: 4000,
  },
});
