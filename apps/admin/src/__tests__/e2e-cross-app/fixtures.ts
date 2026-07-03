import { test as base, type Page } from "@playwright/test";

/**
 * Cross-app E2E fixtures: connects to 3 running Tauri apps
 * (admin + 2 clientes) via separate WebDriver/CDP ports.
 *
 * PRECONDICIONES:
 *   Las 3 apps Tauri deben estar corriendo con WebDriver habilitado
 *   en los puertos definidos por las variables de entorno:
 *
 *   ADMIN_WS_URL    — default ws://localhost:9222
 *   CLIENT1_WS_URL  — default ws://localhost:9223
 *   CLIENT2_WS_URL  — default ws://localhost:9224
 *
 *   Usa `scripts/start-e2e-cross-app.sh` para lanzarlas.
 */

type CrossAppFixtures = {
  adminPage: Page;
  client1Page: Page;
  client2Page: Page;
};

async function connectPage(envVar: string, fallbackPort: number): Promise<Page> {
  const url = process.env[envVar] || `ws://localhost:${fallbackPort}`;
  const { chromium } = await import("@playwright/test");
  const browser = await chromium.connect({ wsEndpoint: url });
  const context = await browser.newContext();
  const page = await context.newPage();
  // Store browser on page so we can close it in teardown
  (page as any).__browser = browser;
  (page as any).__context = context;
  return page;
}

export const test = base.extend<CrossAppFixtures>({
  adminPage: async ({}, use) => {
    const page = await connectPage("ADMIN_WS_URL", 9222);
    await use(page);
    await (page as any).__context?.close();
  },

  client1Page: async ({}, use) => {
    const page = await connectPage("CLIENT1_WS_URL", 9223);
    await use(page);
    await (page as any).__context?.close();
  },

  client2Page: async ({}, use) => {
    const page = await connectPage("CLIENT2_WS_URL", 9224);
    await use(page);
    await (page as any).__context?.close();
  },
});

export { expect } from "@playwright/test";
