import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Vite configuration tuned for Tauri development.
// The dev server is pinned to a fixed port because Tauri's devUrl is static.
export default defineConfig({
  plugins: [react()],

  // Prevent Vite from obscuring Rust errors when the dev server starts slowly.
  clearScreen: false,

  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Don't watch the Rust sources — the Tauri CLI handles rebuilds.
      ignored: ["**/src-tauri/**"],
    },
  },

  build: {
    target: "es2021",
    outDir: "dist",
    sourcemap: false,
  },
});
