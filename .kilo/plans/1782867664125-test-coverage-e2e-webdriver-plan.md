# Plan: Cobertura completa de tests + E2E con Playwright/WebDriver

## Objetivo

Unificar y expandir la cobertura de tests del monorepo, cubriendo Rust (unitarios + integración), frontend (Vitest) y UI E2E (Playwright + WebDriver contra Tauri v2 real).

---

## 1. Limpieza y reestructuración

### 1.1 Eliminar tests muertos
- `crates/syntrix-schema/` — 9 tests en `entities.rs`, `upcast.rs`, `encoded.rs`. El crate fue removido del workspace en la migración Limbo. Estos archivos son código muerto.
  - **Acción**: eliminar `crates/syntrix-schema/` completamente del disco.

### 1.2 Reorganizar tests Rust
```
apps/admin/src-tauri/tests/
├── unit/                    # [nuevo] tests unitarios aislados
│   ├── audit_test.rs
│   ├── storage_test.rs
│   └── admin_test.rs
├── integration/             # [renombrar actuales]
│   ├── basic_test.rs        # [movido desde tests/]
│   └── sync_test.rs         # [renombrar e2e_sync_test.rs]
└── common/
    └── mod.rs               # helpers compartidos: spawn_admin, spawn_client, invite_and_join

apps/client/src-tauri/tests/
├── unit/                    # [nuevo]
│   └── live_test.rs
├── integration/             # [nuevo]
│   ├── crud_test.rs
│   ├── search_test.rs
│   └── sync_test.rs
└── common/
    └── mod.rs

crates/syntrix-core/src/registry.rs   # [mantener] inline #[cfg(test)] — 5 tests
crates/syntrix-network/src/cdc.rs     # [nuevo] inline #[cfg(test)]

crates/syntrix-testkit/src/lib.rs     # [expandir] helpers: limbo_fixture, cdc_fixture
```

### 1.3 Reorganizar tests frontend
```
apps/admin/src/__tests__/     # [nuevo]
├── screens/
│   ├── CreateOrg.test.tsx
│   ├── DevicesGrid.test.tsx
│   ├── RolesGrid.test.tsx
│   ├── AuditTrail.test.tsx
│   └── ShareDialog.test.tsx
├── components/
│   └── SyncStatusIndicator.test.tsx
└── e2e/                      # [nuevo] Playwright/WebDriver
    ├── playwright.config.ts
    ├── fixtures.ts
    └── flows/
        ├── create-org.spec.ts
        ├── manage-devices.spec.ts
        └── audit.spec.ts

apps/client/src/__tests__/    # [expandir]
├── components/               # [movido desde src/test/]
│   ├── setup.ts
│   ├── EntityGrid.test.tsx
│   └── DetailPanel.test.tsx  # [nuevo]
├── screens/
│   ├── Setup.test.tsx        # [nuevo]
│   └── Inbox.test.tsx        # [nuevo]
└── e2e/                      # [nuevo] Playwright/WebDriver
    ├── playwright.config.ts
    ├── fixtures.ts
    └── flows/
        ├── join-org.spec.ts
        ├── entity-crud.spec.ts
        └── live-search.spec.ts
```

---

## 2. Arreglar tests e2e P2P existentes

**Archivo**: `apps/admin/src-tauri/tests/integration/sync_test.rs` (renombrado)

**Problema**: tests fallan con `no invites received` — el invite P2P no llega al cliente en 500ms porque los peers no están directamente conectados.

**Fix**:
1. En `invite_one_client`, antes de `admin::send_invite`, forzar conexión directa con `admin.p2p().dial(addr)` a la dirección del cliente.
2. Cambiar `tokio::time::sleep(500ms)` por `poll_until` con reintentos (máx 30 intentos, 200ms delay) esperando que `client.get_invites()` no esté vacío.
3. Agregar `wait_for_peer_connected` helper usando el evento `PeerConnected` del P2P.

### Archivos a modificar:
- `apps/admin/src-tauri/tests/common/mod.rs` — nuevos helpers
- `apps/admin/src-tauri/tests/integration/sync_test.rs` — usar los helpers

---

## 3. Agregar tests unitarios Rust (inline `#[cfg(test)]`)

### 3.1 `crates/syntrix-network/src/cdc.rs`

| Test | Descripción |
|------|-------------|
| `test_read_cdc_events_empty` | CDC sin cambios retorna vec vacío, max_change_id = since |
| `test_read_cdc_events_with_changes` | Insertar en tabla, verificar CDC las captura |
| `test_read_cdc_events_table_filter` | Filtro por tabla específica retorna solo esa entidad |
| `test_apply_cdc_events_upsert` | aplicar evento INSERT OR REPLACE |
| `test_apply_cdc_events_lww_skip` | evento con change_time menor al existente se omite |
| `test_apply_cdc_events_lww_apply` | evento con change_time mayor pisa al existente |
| `test_entity_table_name_valid` | todas las entidades válidas mapean correctamente |
| `test_entity_table_name_invalid` | entidad desconocida → error |

### 3.2 `apps/client/src-tauri/src/indexes.rs`

| Test | Descripción |
|------|-------------|
| `test_upsert_document_new` | INSERT de documento nuevo, `get_document` lo recupera |
| `test_upsert_document_update` | INSERT OR REPLACE pisa documento existente |
| `test_upsert_document_with_hlc_new` | Sin entrada previa en HLC tracker, inserta normal |
| `test_upsert_document_with_hlc_skip` | HLC menor o igual al almacenado → no pisa |
| `test_query_all` | Sin filtros retorna todos los documentos de la entidad |
| `test_query_with_filter` | Con filtro `field=value` retorna subset |
| `test_query_sort_and_limit` | Sort por campo + limit funcionan |
| `test_append_event` | Insertar evento en event_log, `query_events_since` lo ve |
| `test_query_events_since_cursor` | Cursor > evento → no retorna ese evento |
| `test_get_heartbeats` | upsert + get retornan el timestamp correcto |
| `test_upsert_member` / `test_get_members` | CRUD miembros |
| `test_upsert_role_cfg` / `test_get_roles` | CRUD roles |

### 3.3 `apps/client/src-tauri/src/search.rs`

| Test | Descripción |
|------|-------------|
| `test_search_empty` | Sin resultados con query vacía |
| `test_search_by_entity_filter` | Filtrar por entidad específica |
| `test_search_across_all_entities` | entities=None busca en todas las tablas |
| `test_search_limit` | limit trunca resultados |

### 3.4 `apps/client/src-tauri/src/live.rs`

| Test | Descripción |
|------|-------------|
| `test_subscribe_and_unsubscribe` | subscribe retorna id incremental, unsubscribe remueve |
| `test_notify_matching_table` | cambiar tabla en depends_on → emite `live_update` |
| `test_notify_non_matching_table` | cambiar tabla no en depends_on → no emite |
| `test_notify_multiple_subscriptions` | múltiples subscripciones con overlap → todas reciben |

### 3.5 `apps/admin/src-tauri/src/audit.rs` / `apps/client/src-tauri/src/audit.rs`

| Test | Descripción |
|------|-------------|
| `test_audit_empty` | Sin eventos retorna vec vacío |
| `test_audit_filter_by_entity` | Filtro entity funciona |
| `test_audit_filter_by_date_range` | since_ts / until_ts filtran correctamente |
| `test_audit_pagination` | offset + limit funcionan |

### 3.6 `apps/client/src-tauri/src/events.rs`

| Test | Descripción |
|------|-------------|
| `test_commit_event_allowed` | Rol con can_write → ok |
| `test_commit_event_denied` | Rol sin can_write → error |
| `test_commit_event_wildcard` | Rol admin (*) → ok para cualquier entidad |

### 3.7 `apps/client/src-tauri/src/sync.rs`

| Test | Descripción |
|------|-------------|
| `test_sync_push_and_pull` | push de eventos, pull con cursor los retorna |
| `test_sync_pull_empty` | Pull sin eventos nuevos → batch vacío |
| `test_sync_status` | status retorna string con info de orgs |

---

## 4. Expandir syntrix-testkit

Agregar helpers para tests con Limbo:

```rust
// syntrix-testkit/src/lib.rs

use std::sync::Arc;
use std::path::PathBuf;

/// Crea una DB Limbo temporal en un TempDir y retorna (TempDir, Arc<Connection>)
pub fn temp_limbo_db() -> (tempfile::TempDir, Arc<turso_core::Connection>) { ... }

/// Inserta un documento de prueba y retorna el doc_id
pub fn seed_test_document(
    conn: &Arc<turso_core::Connection>,
    entity: &str,
    org_id: &str,
    data: serde_json::Value,
) -> anyhow::Result<(String, serde_json::Value)> { ... }

/// Espera hasta que un documento aparezca vía CDC o query directo
pub async fn wait_for_document_limbo(
    conn: &Arc<turso_core::Connection>,
    org_id: &str,
    entity: &str,
    doc_id: &str,
) -> anyhow::Result<serde_json::Value> { ... }

/// Espera que un evento aparezca en event_log
pub async fn wait_for_event(
    conn: &Arc<turso_core::Connection>,
    org_id: &str,
    key: &str,
) -> anyhow::Result<serde_json::Value> { ... }
```

---

## 5. Frontend Vitest tests (componentes)

### 5.1 Admin (`apps/admin/src/__tests__/`)

| Test | Pantalla/Componente | Flujo |
|------|-------------------|-------|
| CreateOrg | `CreateOrg.tsx` | Form submit → mock `create_org` invoke → redirect |
| DevicesGrid | `DevicesGridPage.tsx` | Carga lista de devices, click en row, botón add | 
| RolesGrid | `RolesGridPage.tsx` | Lista roles, crear/editar flujo con permission matrix |
| AuditTrail | `AuditTrail.tsx` | Filtros (entity, date, type), paginación |
| ShareDialog | `ShareDialog.tsx` | Modal de invite, select role, copiar link |

### 5.2 Client (`apps/client/src/__tests__/`)

| Test | Componente | Flujo |
|------|-----------|-------|
| EntityGrid | `EntityGrid.tsx` | Carga datos, click row, edit/create (existente + expandir) |
| DetailPanel | `DetailPanel.tsx` | Abrir panel, campos editables, save/cancel |
| Setup | `Setup.tsx` | Flujo de join org con invite JSON |
| Inbox | `Inbox.tsx` | Lista de invites pendientes, accept |

---

## 6. UI E2E con Playwright + WebDriver (Tauri v2 real)

### 6.1 Setup WebDriver (Nix)

Agregar al `justfile`:

```justfile
# Instala dependencias para tests E2E con WebDriver
e2e-setup:
    nix --extra-experimental-features "nix-command flakes" shell \
        nixpkgs#webkitgtk_4_1 \
        --command bash -c 'which WebKitWebDriver && echo "WebDriver OK"'
    cd apps/admin && pnpm exec playwright install webkit
    cd apps/client && pnpm exec playwright install webkit
```

El paquete `webkitgtk_4_1` (sin `.dev`) incluye el binario `WebKitWebDriver` necesario para que Playwright controle la app Tauri vía protocolo WebDriver.

### 6.2 Configuración Playwright

```typescript
// apps/admin/playwright.config.ts
import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './src/__tests__/e2e',
  timeout: 30000,
  projects: [
    {
      name: 'webkit',
      use: {
        browserName: 'webkit',
        // Conectar a la app Tauri ya corriendo vía WebDriver
        // La URL real se obtiene al iniciar la app
      },
    },
  ],
});
```

### 6.3 Flujo de ejecución E2E

1. `just remote-compile-admin` — compila binario en server-1
2. Iniciar app: `./target/debug/syntrix-admin` (ya compilado)
3. `npx playwright test` — conecta vía WebDriver y ejecuta specs

### 6.4 Specs E2E

**Admin** (`apps/admin/src/__tests__/e2e/flows/`):

| Spec | Flujo |
|------|-------|
| `create-org.spec.ts` | Abrir app → click "Create Org" → llenar nombre → submit → ver org en lista |
| `manage-devices.spec.ts` | Seleccionar org → ver devices → add device → ver en grid |
| `roles-crud.spec.ts` | Abrir roles → crear rol con permisos → editar → ver cambios |
| `audit-trail.spec.ts` | Navegar a audit → aplicar filtros → ver resultados → paginación |
| `invite-flow.spec.ts` | Abrir share dialog → generar invite → ver JSON copiable |

**Client** (`apps/client/src/__tests__/e2e/flows/`):

| Spec | Flujo |
|------|-------|
| `join-org.spec.ts` | Abrir app → pegar invite JSON → join → ver org en selector |
| `entity-grid.spec.ts` | Seleccionar customers → ver grid → crear nuevo → editar → eliminar |
| `search.spec.ts` | Usar search bar → ver resultados → filtrar por entidad |
| `live-updates.spec.ts` | Abrir grid → esperar `live_update` event → ver fila actualizada sin refresh |

### 6.5 Fixtures Playwright

```typescript
// apps/admin/src/__tests__/e2e/fixtures.ts
import { test as base } from '@playwright/test';

export const test = base.extend({
  tauriApp: async ({}, use) => {
    // Conectar a la app Tauri ya corriendo
    const browser = await playwright.webkit.connect({
      wsEndpoint: process.env.TAURI_WEBDRIVER_URL || 'ws://localhost:9222',
    });
    const context = await browser.newContext();
    const page = await context.newPage();
    await use(page);
    await context.close();
  },
});
```

---

## 7. Comandos en justfile

```justfile
# Tests Rust unitarios (rápidos, sin P2P)
test-unit:
    nix ... cargo test --workspace --lib

# Tests Rust integración (admin/client, con P2P limitado)
test-integration:
    nix ... cargo test --workspace --tests

# Tests Rust completos
test-rust: test-unit test-integration

# Tests frontend Vitest (admin + client)
test-frontend:
    cd apps/client && pnpm test
    cd apps/admin && pnpm test

# Tests E2E con Playwright/WebDriver (requiere app compilada y corriendo)
test-e2e-admin:
    cd apps/admin && npx playwright test
test-e2e-client:
    cd apps/client && npx playwright test

# Setup WebDriver para E2E
e2e-setup:
    @echo "Instalando WebKitWebDriver..."
    nix ... webkitgtk_4_1 ...
    cd apps/admin && npx playwright install webkit
    cd apps/client && npx playwright install webkit
```

---

## 8. Ejecución

### Fase 1: Limpieza y reestructuración
1. Eliminar `crates/syntrix-schema/` del disco
2. Crear estructura de directorios `tests/unit/`, `tests/integration/`, `tests/common/`
3. Mover `basic_test.rs` y `e2e_sync_test.rs` a `tests/integration/`
4. Extraer helpers de `e2e_sync_test.rs` a `tests/common/mod.rs`
5. Mover `apps/client/src/test/` → `apps/client/src/__tests__/components/`

### Fase 2: Arreglar tests e2e P2P
6. Agregar `wait_for_peer_connected` en testkit
7. Fixear `invite_one_client` con dial + poll_until
8. Verificar que sync_test pasa

### Fase 3: Unit tests Rust (inline #[cfg(test)])
9. `cdc.rs` — 8 tests
10. `indexes.rs` — 10 tests
11. `search.rs` — 4 tests
12. `live.rs` — 4 tests
13. `audit.rs` (admin + client) — 5 tests c/u
14. `events.rs` — 3 tests
15. `sync.rs` — 3 tests

### Fase 4: Expandir testkit
16. Agregar `temp_limbo_db`, `seed_test_document`, `wait_for_document_limbo`, `wait_for_event`

### Fase 5: Frontend Vitest
17. Admin: 5 test suites
18. Client: 4 test suites

### Fase 6: E2E Playwright/WebDriver
19. Instalar deps (webkitgtk_4_1, playwright)
20. Crear config playwright para admin y client
21. Escribir fixtures y specs (8 specs total)
22. Agregar `just e2e-setup`, `just test-e2e-admin`, `just test-e2e-client`

### Fase 7: Justfile y verificación
23. Unificar `just test-rust` para correr unit + integration
24. `just test` → unit + integration + frontend
25. `just test-all` → todo incluyendo e2e
26. `cargo check --workspace --tests` pasa

---

## Riesgos y mitigaciones

| Riesgo | Mitigación |
|--------|-----------|
| WebKitWebDriver inestable en Nix | Probar primero con `WebKitWebDriver --help`; si falla, usar Playwright Chromium + mock Tauri |
| Tests e2e P2P siguen fallando por timing | Agregar timeouts más largos + reintentos exponenciales en `wait_for_peer_connected` |
| Limbo no tiene API de cleanup entre tests | Usar `temp_limbo_db()` con TempDir (se borra al finalizar el test) |
| Playwright fixtures para Tauri complejos | Empezar con 1 spec simple (create-org) como smoke test, expandir después |
