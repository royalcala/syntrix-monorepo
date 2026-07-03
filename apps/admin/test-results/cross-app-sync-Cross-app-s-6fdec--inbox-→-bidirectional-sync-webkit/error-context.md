# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: cross-app-sync.spec.ts >> Cross-app sync (admin + 2 clients) >> full flow: org → role → add device → invite → inbox → bidirectional sync
- Location: src/__tests__/e2e-cross-app/flows/cross-app-sync.spec.ts:34:3

# Error details

```
Error: Cannot find package 'playwright' imported from /home/alcala/Documents/github/syntrix-p2p/syntrix-monorepo/.kilo/worktrees/pouncing-noise/apps/admin/src/__tests__/e2e-cross-app/fixtures.ts
Did you mean to import "playwright/index.js"?
```

# Test source

```ts
  1  | import { test as base, type Page } from "@playwright/test";
  2  | 
  3  | /**
  4  |  * Cross-app E2E fixtures: connects to 3 running Tauri apps
  5  |  * (admin + 2 clientes) via separate WebDriver/CDP ports.
  6  |  *
  7  |  * PRECONDICIONES:
  8  |  *   Las 3 apps Tauri deben estar corriendo con WebDriver habilitado
  9  |  *   en los puertos definidos por las variables de entorno:
  10 |  *
  11 |  *   ADMIN_WS_URL    — default ws://localhost:9222
  12 |  *   CLIENT1_WS_URL  — default ws://localhost:9223
  13 |  *   CLIENT2_WS_URL  — default ws://localhost:9224
  14 |  *
  15 |  *   Usa `scripts/start-e2e-cross-app.sh` para lanzarlas.
  16 |  */
  17 | 
  18 | type CrossAppFixtures = {
  19 |   adminPage: Page;
  20 |   client1Page: Page;
  21 |   client2Page: Page;
  22 | };
  23 | 
  24 | async function connectPage(envVar: string, fallbackPort: number): Promise<Page> {
  25 |   const url = process.env[envVar] || `ws://localhost:${fallbackPort}`;
> 26 |   const { chromium } = await import("playwright");
     |                        ^ Error: Cannot find package 'playwright' imported from /home/alcala/Documents/github/syntrix-p2p/syntrix-monorepo/.kilo/worktrees/pouncing-noise/apps/admin/src/__tests__/e2e-cross-app/fixtures.ts
  27 |   const browser = await chromium.connect({ wsEndpoint: url });
  28 |   const context = await browser.newContext();
  29 |   const page = await context.newPage();
  30 |   // Store browser on page so we can close it in teardown
  31 |   (page as any).__browser = browser;
  32 |   (page as any).__context = context;
  33 |   return page;
  34 | }
  35 | 
  36 | export const test = base.extend<CrossAppFixtures>({
  37 |   adminPage: async ({}, use) => {
  38 |     const page = await connectPage("ADMIN_WS_URL", 9222);
  39 |     await use(page);
  40 |     await (page as any).__context?.close();
  41 |   },
  42 | 
  43 |   client1Page: async ({}, use) => {
  44 |     const page = await connectPage("CLIENT1_WS_URL", 9223);
  45 |     await use(page);
  46 |     await (page as any).__context?.close();
  47 |   },
  48 | 
  49 |   client2Page: async ({}, use) => {
  50 |     const page = await connectPage("CLIENT2_WS_URL", 9224);
  51 |     await use(page);
  52 |     await (page as any).__context?.close();
  53 |   },
  54 | });
  55 | 
  56 | export { expect } from "@playwright/test";
  57 | 
```