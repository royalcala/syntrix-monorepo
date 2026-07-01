import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./flows",
  timeout: 30000,
  projects: [
    {
      name: "webkit",
      use: {
        browserName: "webkit",
      },
    },
  ],
});
