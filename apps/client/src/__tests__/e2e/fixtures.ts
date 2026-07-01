import { test as base } from "@playwright/test";

export const test = base.extend({
  tauriApp: async ({}, use) => {
    const { chromium } = await import("playwright");
    const url = process.env.TAURI_WEBDRIVER_URL || "ws://localhost:9222";
    const browser = await chromium.connect({ wsEndpoint: url });
    const context = await browser.newContext();
    const page = await context.newPage();
    await use(page);
    await context.close();
  },
});

export { expect } from "@playwright/test";
