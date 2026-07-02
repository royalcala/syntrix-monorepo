import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./flows",
  timeout: 120_000, // 2 min — P2P gossip + polling takes time
  expect: {
    timeout: 15_000,
  },
  projects: [
    {
      name: "webkit",
      use: {
        browserName: "webkit",
      },
    },
  ],
});
