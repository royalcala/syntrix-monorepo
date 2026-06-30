import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@syntrix/ui": path.resolve(__dirname, "../../packages/syntrix-ui/src"),
    },
  },
  server: { port: 5173, strictPort: true },
});
