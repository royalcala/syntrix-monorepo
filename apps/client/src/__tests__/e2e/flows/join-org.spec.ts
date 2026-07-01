import { test, expect } from "../fixtures";

test("paste invite ticket and join org", async ({ tauriApp: page }) => {
  await page.goto("tauri://localhost");
  await page.waitForSelector("text=Syntrix", { timeout: 10000 });
  await page.fill("textarea", "invite-ticket-json");
  await page.click("text=Unirse");
  await page.waitForSelector("text=Org Navegador", { timeout: 10000 });
  expect(await page.isVisible("text=Org Navegador")).toBe(true);
});
