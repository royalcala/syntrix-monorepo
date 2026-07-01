import { test, expect } from "../fixtures";

test("view devices in an org", async ({ tauriApp: page }) => {
  await page.goto("tauri://localhost");
  await page.waitForSelector("text=Dispositivos", { timeout: 10000 });
  await page.click("text=Dispositivos");
  await page.waitForSelector("text=admin", { timeout: 5000 });
  expect(await page.isVisible("text=admin")).toBe(true);
});
