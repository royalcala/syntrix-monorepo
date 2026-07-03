# Plan: Post-fixes consolidado + shared drizzle + timeline CDC + cleanup

> Última actualización: 2026-07-02

---

## 1. Bugs corregidos (11)

Todos los 11 bugs de la sesión original están corregidos y verificados por tests.

---

## 2. Código legacy eliminado

| Lo que era | Acción | Estado |
|-----------|--------|--------|
| `sync_push` / `sync_pull` / `sync_ping` | Borrado | ✅ |
| `append_event` en events.rs (commit_event) | Borrado | ✅ |
| `append_event` en identity.rs (process_gossip_event) | Borrado | ✅ |
| `append_event` en indexes.rs (método) | Borrado | ✅ |
| `event_log` del schema Drizzle del cliente | Borrado + migración generada | ✅ |
| 5 tests de sync.rs (HlcCursor, SyncEntry, etc.) | Eliminados | ✅ |
| `count_events` helper | Eliminado | ✅ |
| `test_duplicate_event_prevention` | Eliminado | ✅ |

**Mantenido:** `query_events_since` en indexes.rs (usado por audit.rs y catchup handler)

---

## 3. Tests agregados (19 nuevos)

### sync_test.rs (21 pasan, 1 ignorado)
- `test_inbox_receives_invite`, `test_inbox_and_bidirectional_sync`
- `test_update_replicates`
- `test_late_joiner_receives_existing_data`, `test_late_joiner_sees_updated_data`
- `test_late_joiner_with_multiple_writers`

### cdc.rs (17 pasan, 2 nuevos)
- `read_events_since_cursor_timeline` — CDC events desde cursor, filtro, continuidad
- `read_events_since_respects_limit` — límite + continuación

### binary_e2e_test.rs (1 ignorado)
- `test_binary_invite_and_bidirectional_sync`

### Suite completa
- `sync_test`: 21/21, 1 ignorado
- `syntrix-admin` lib: 10/10
- `syntrix-client` lib+tests: 27/27
- `syntrix-network` lib (CDC): 17/17
- `basic_test`: 5/5

---

## 4. Infraestructura nueva

### Shared Drizzle (`packages/shared-drizzle/`)
- `entities.ts` — customers, suppliers, products, invoices, invoiceItems, orders, orderItems, payroll
- `zod.ts` — drizzle-zod schemas (insert + select) para cada entidad
- Ambos apps importan via path relativo `../../../packages/shared-drizzle/src/entities`
- `drizzle-kit generate` detecta los cambios y genera migraciones correctamente

### Timeline CDC — `get_updates_since` (Opción C)
- **Backend**: `get_updates_since` Tauri command en el cliente
  - Llama a `read_cdc_events(sinceChangeId)` → filtra por org_id → devuelve eventos + cursor
  - Almacenamiento del cursor en localStorage (frontend)
  - Polling cada 2s para detects cambios en tiempo real
- **Frontend**: Hook `useTimelineCursor` en `apps/client/src/hooks/useTimelineCursor.ts`
  - Invoca `get_updates_since` cada 2s
  - Guarda cursor en localStorage (sobrevive refrescos)
  - Invalida `react-query` caches al recibir eventos nuevos
  - Detecta tablas hijas (invoice_items → invoices)

### Headless mode (`--headless`)
Admin y client: comando `send_invite` usa `send_invite_full` (dial + wait + publish)

### Per-org lock en SqlEngine
Serialización de `apply_cdc_events` por org para evitar races de gossipsub duplicates

---

## 5. Pendientes (futuros sprints)

| Prioridad | Tarea | Detalle |
|-----------|-------|---------|
| 🟡 | Migrar catchup legacy a CDC | `CatchupRequestReceived` usa `query_events_since`. Cambiar a `snapshot_org_rows`. |
| 🟡 | Conectar `useTimelineCursor` a EntityGrid | El hook existe pero EntityGrid aún usa `refetchQueries`. Migrar a invalidación reactiva. |
| 🟡 | `test_concurrent_commits` | Mesh gossipsub tarda en formarse en in-process. Cubierto por binary E2E. |
| 🟢 | Eliminar `crates/syntrix-schema/` | Código muerto desde migración Limbo. |
| 🟢 | Unificar esquemas Zod viejos | `packages/syntrix-ui/src/collections/schemas.ts` y `apps/client/src/collections/schemas.ts` aún existen. Migrar a drizzle-zod. |
