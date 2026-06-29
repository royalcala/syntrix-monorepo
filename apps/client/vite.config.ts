import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";
import { reactClickToComponent } from "vite-plugin-react-click-to-component";

export default defineConfig({
  plugins: [react(), tailwindcss(), reactClickToComponent()],
  resolve: {
    alias: {
      "@syntrix/ui": path.resolve(__dirname, "../../packages/syntrix-ui/src"),
    },
  },
  clearScreen: false,
  server: { port: 1420, strictPort: true, fs: { allow: ["../.."] }, watch: { ignored: ["**/src-tauri/**"] } },
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: "./src/test/setup.ts",
  },
} as any);
