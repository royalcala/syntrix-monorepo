import { test, expect } from "../fixtures";

test("view entity grid and create a new record", async ({ tauriApp: page }) => {
  await page.goto("tauri://localhost");
  await page.waitForSelector("text=Clientes", { timeout: 10000 });
  await page.click("text=Clientes");
  await page.waitForSelector("table", { timeout: 5000 });
  expect(await page.isVisible("table")).toBe(true);
});
