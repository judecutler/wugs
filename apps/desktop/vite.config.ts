import { defineConfig } from "vite";

// Tauri expects a fixed dev port and must see Rust-side errors in the terminal.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "es2022",
  },
});
