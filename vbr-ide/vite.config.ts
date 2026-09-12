import { defineConfig } from "vite";

// Tauri drives the dev server, so pin the port and keep its logs on screen.
// Windows uses 1421 so it doesn't collide with WSL2's localhost relay on 1420.
const devPort = process.platform === "win32" ? 1421 : 1420;

export default defineConfig({
  clearScreen: false,
  server: {
    port: devPort,
    strictPort: true,
    // The example picker imports `.vbr` files from the repo's examples/ dir,
    // one level above this project — let the dev server read them.
    fs: {
      allow: [".."],
    },
  },
  build: {
    target: "es2020",
    // Monaco is a large dependency by nature; don't nag about chunk size.
    chunkSizeWarningLimit: 4000,
  },
});
