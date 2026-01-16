import { defineConfig } from "vite";
import solidPlugin from "vite-plugin-solid";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [solidPlugin()],
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: "esnext",
    outDir: "dist",
  },
  // Prevent vite from obscuring rust errors
  clearScreen: false,
});
