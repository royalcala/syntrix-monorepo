import { test, expect } from "../fixtures";
import type { Page } from "@playwright/test";

/**
 * test: cross-app-sync
 *
 * Flujo completo end-to-end:
 *   1. Admin crea org "acme" + rol "customer-rw" (customers lectura/escritura)
 *   2. Cliente 1 → obtener address → admin lo agrega como dispositivo → envía invite
 *   3. Cliente 1 → revisa inbox → acepta invite → escribe registro "Alpha"
 *   4. Cliente 2 → obtener address → admin lo agrega → envía invite
 *   5. Cliente 2 → inbox → acepta → escribe registro "Beta"
 *   6. Verificar sync bidireccional:
 *      - Cliente 2 ve "Alpha" (sync cliente1 → cliente2)
 *      - Cliente 1 ve "Beta"  (sync cliente2 → cliente1)
 *
 * PRECONDICIONES:
 *   Las 3 apps Tauri deben estar corriendo con WebDriver:
 *     Admin:   ws://localhost:9222  (SYNTRIX_DATA_DIR=/tmp/e2e/admin)
 *     Client1: ws://localhost:9223  (SYNTRIX_DATA_DIR=/tmp/e2e/client1)
 *     Client2: ws://localhost:9224  (SYNTRIX_DATA_DIR=/tmp/e2e/client2)
 *
 *   Ejecutar: scripts/start-e2e-cross-app.sh
 */

const ORG_NAME = "acme";
const ROLE_NAME = "customer-rw";
const POLL_MS = 2000;
const MAX_POLLS = 30; // 60 seconds max

test.describe("Cross-app sync (admin + 2 clients)", () => {
  test.setTimeout(180_000); // 3 minutes for full P2P flow

  test("full flow: org → role → add device → invite → inbox → bidirectional sync", async ({
    adminPage,
    client1Page,
    client2Page,
  }) => {
    // =========================================================================
    // STEP 1: Admin crea organización "acme"
    // =========================================================================
    await adminPage.goto("tauri://localhost");
    await adminPage.waitForSelector("text=Crear organización", { timeout: 10000 });
    await adminPage.fill("input[placeholder='Nombre de la organización']", ORG_NAME);
    await adminPage.click("text=Crear");
    await adminPage.waitForSelector(`text=${ORG_NAME}`, { timeout: 10000 });
    console.log("[admin] org created:", ORG_NAME);

    // =========================================================================
    // STEP 2: Admin crea rol "customer-rw" con customers read/write
    // =========================================================================
    // Navigate to roles via sidebar
    await adminPage.click("text=Roles");
    await adminPage.waitForSelector("button:has-text('Nuevo Rol')", { timeout: 5000 });
    await adminPage.click("button:has-text('Nuevo Rol')");

    // Wait for the create role dialog
    const dialog = adminPage.getByRole("dialog");
    await dialog.waitFor({ timeout: 5000 });
    await dialog.getByPlaceholder("ej. ventas, contabilidad, admin").fill(ROLE_NAME);

    // Enable "Leer" + "Escribir" checkboxes for "customers" in PermissionMatrix
    const customersRow = dialog.locator("tr", { hasText: "customers" });
    // Read checkbox (first in row)
    const leerCb = customersRow.locator("input[type='checkbox']").first();
    if (!(await leerCb.isChecked())) await leerCb.click();
    // Write checkbox (second in row)
    const escribirCb = customersRow.locator("input[type='checkbox']").nth(1);
    if (!(await escribirCb.isChecked())) await escribirCb.click();

    await dialog.getByRole("button", { name: "Crear Rol" }).click();
    await dialog.waitFor({ state: "hidden", timeout: 5000 }).catch(() => {});
    console.log("[admin] role created:", ROLE_NAME);

    // =========================================================================
    // STEP 3: Obtener address del Cliente 1
    // =========================================================================
    await client1Page.goto("tauri://localhost/orgs");
    await client1Page.waitForSelector("text=Your Device Address", { timeout: 10000 });

    // Read the device address JSON from the <code> block
    const c1Address = await client1Page.evaluate(() => {
      const code = document.querySelector("code");
      return code?.textContent || "";
    });
    expect(c1Address).toBeTruthy();
    expect(c1Address).toContain("node_id");
    console.log("[client1] address:", c1Address.substring(0, 80) + "...");

    // =========================================================================
    // STEP 4: Admin agrega dispositivo Cliente 1 con rol "customer-rw"
    // =========================================================================
    await adminPage.click("text=Dispositivos");
    await adminPage.waitForSelector("button:has-text('Nuevo')", { timeout: 5000 });
    await adminPage.click("button:has-text('Nuevo')");

    // Fill device form (label + sibling div > input)
    await adminPage.locator('label:has-text("Node ID") + div input').fill(c1Address);
    await adminPage.locator('label:has-text("Nombre") + div input').fill("Client 1");
    await adminPage.locator('label:has-text("Persona") + div input').fill("client-1-person");

    // Role selector — Radix Select (combobox)
    const roleTrigger = adminPage.locator('label:has-text("Rol")').locator("..").getByRole("combobox");
    await roleTrigger.click();
    await adminPage.getByRole("option", { name: ROLE_NAME }).click();

    // "Crear" button calls send_invite internally
    await adminPage.getByRole("button", { name: "Crear" }).click();
    await adminPage.waitForTimeout(2000);
    console.log("[admin] device added + invite sent for client1");

    // =========================================================================
    // STEP 5: Cliente 1 revisa inbox y acepta invite
    // =========================================================================
    await client1Page.goto("tauri://localhost/inbox");
    const inviteCard1 = await pollForElement(
      client1Page, "text=Organization invitation", POLL_MS, MAX_POLLS,
    );
    expect(inviteCard1).toBeTruthy();
    console.log("[client1] invite received in inbox");

    // Verify invite details
    await expect(client1Page.locator("strong")).toContainText(ORG_NAME);
    await expect(client1Page.locator('[role="status"]')).toContainText(ROLE_NAME);

    // Accept
    await client1Page.click("button:has-text('Accept')");
    await client1Page.waitForTimeout(2000);
    console.log("[client1] invite accepted");

    // =========================================================================
    // STEP 6: Cliente 1 escribe "Customer Alpha"
    // =========================================================================
    await client1Page.goto("tauri://localhost/customers");
    await client1Page.waitForSelector("text=Clientes", { timeout: 5000 });
    await client1Page.click("button:has-text('Nuevo')");
    await fillCustomerForm(client1Page, {
      name: "Customer Alpha", address: "123 Main St",
      email: "alpha@test.com", phone: "555-0001",
    });
    await client1Page.getByRole("button", { name: "Crear" }).click();
    await client1Page.waitForTimeout(2000);
    console.log("[client1] customer 'Alpha' created");

    // =========================================================================
    // STEP 7: Obtener address del Cliente 2
    // =========================================================================
    await client2Page.goto("tauri://localhost/orgs");
    await client2Page.waitForSelector("text=Your Device Address", { timeout: 10000 });

    const c2Address = await client2Page.evaluate(() => {
      const code = document.querySelector("code");
      return code?.textContent || "";
    });
    expect(c2Address).toBeTruthy();
    expect(c2Address).toContain("node_id");
    console.log("[client2] address:", c2Address.substring(0, 80) + "...");

    // =========================================================================
    // STEP 8: Admin agrega dispositivo Cliente 2
    // =========================================================================
    await adminPage.goto("tauri://localhost/devices");
    await adminPage.waitForSelector("button:has-text('Nuevo')", { timeout: 5000 });
    await adminPage.click("button:has-text('Nuevo')");

    await adminPage.locator('label:has-text("Node ID") + div input').fill(c2Address);
    await adminPage.locator('label:has-text("Nombre") + div input').fill("Client 2");
    await adminPage.locator('label:has-text("Persona") + div input').fill("client-2-person");

    const roleTrigger2 = adminPage.locator('label:has-text("Rol")').locator("..").getByRole("combobox");
    await roleTrigger2.click();
    await adminPage.getByRole("option", { name: ROLE_NAME }).click();
    await adminPage.getByRole("button", { name: "Crear" }).click();
    await adminPage.waitForTimeout(2000);
    console.log("[admin] device added + invite sent for client2");

    // =========================================================================
    // STEP 9: Cliente 2 revisa inbox y acepta
    // =========================================================================
    await client2Page.goto("tauri://localhost/inbox");
    const inviteCard2 = await pollForElement(
      client2Page, "text=Organization invitation", POLL_MS, MAX_POLLS,
    );
    expect(inviteCard2).toBeTruthy();
    console.log("[client2] invite received in inbox");

    await client2Page.click("button:has-text('Accept')");
    await client2Page.waitForTimeout(2000);
    console.log("[client2] invite accepted");

    // =========================================================================
    // STEP 10: Cliente 2 escribe "Customer Beta"
    // =========================================================================
    await client2Page.goto("tauri://localhost/customers");
    await client2Page.waitForSelector("text=Clientes", { timeout: 5000 });
    await client2Page.click("button:has-text('Nuevo')");
    await fillCustomerForm(client2Page, {
      name: "Customer Beta", address: "456 Oak Ave",
      email: "beta@test.com", phone: "555-0002",
    });
    await client2Page.getByRole("button", { name: "Crear" }).click();
    await client2Page.waitForTimeout(2000);
    console.log("[client2] customer 'Beta' created");

    // =========================================================================
    // STEP 11: Verificar sync bidireccional
    // =========================================================================
    // 11a: Cliente 2 debe ver "Customer Alpha" (sync c1→c2)
    await client2Page.goto("tauri://localhost/customers");
    const alphaOnC2 = await pollForElement(
      client2Page, "text=Customer Alpha", POLL_MS, MAX_POLLS,
    );
    expect(alphaOnC2).toBeTruthy();
    console.log("[sync] client2 sees 'Alpha' ✅");

    // 11b: Cliente 1 debe ver "Customer Beta" (sync c2→c1)
    await client1Page.goto("tauri://localhost/customers");
    const betaOnC1 = await pollForElement(
      client1Page, "text=Customer Beta", POLL_MS, MAX_POLLS,
    );
    expect(betaOnC1).toBeTruthy();
    console.log("[sync] client1 sees 'Beta' ✅");
  });
});

// =============================================================================
// Helpers
// =============================================================================

async function pollForElement(
  page: Page,
  selector: string,
  intervalMs: number,
  maxRetries: number,
): Promise<boolean> {
  for (let i = 0; i < maxRetries; i++) {
    const visible = await page.isVisible(selector).catch(() => false);
    if (visible) return true;
    console.log(`  poll [${i + 1}/${maxRetries}]: waiting for "${selector}"...`);
    await page.waitForTimeout(intervalMs);
  }
  return false;
}

interface CustomerFields {
  name: string;
  address: string;
  email: string;
  phone: string;
}

async function fillCustomerForm(page: Page, fields: CustomerFields) {
  await page.waitForTimeout(500);
  await page.locator('label:has-text("Nombre") + div input').fill(fields.name);
  await page.locator('label:has-text("Dirección") + div input').fill(fields.address);
  await page.locator('label:has-text("Email") + div input').fill(fields.email);
  await page.locator('label:has-text("Teléfono") + div input').fill(fields.phone);
}
