import { test, expect } from "../fixtures";

test("create org and see it in the list", async ({ tauriApp: page }) => {
  await page.goto("tauri://localhost");
  await page.waitForSelector("text=Crear organización", { timeout: 10000 });
  await page.fill("input[placeholder='Nombre de la organización']", "E2E Test Org");
  await page.click("text=Crear");
  await page.waitForSelector("text=E2E Test Org", { timeout: 10000 });
  expect(await page.isVisible("text=E2E Test Org")).toBe(true);
});
