import { defineConfig } from "vite";

// Frontend source lives in src/; build embeds into ../dist for Tauri.
export default defineConfig({
  root: "src",
  build: { outDir: "../dist", emptyOutDir: true },
  clearScreen: false,
  server: { port: 1420, strictPort: true },
});
