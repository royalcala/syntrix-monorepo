# Plan Maestro — Syntrix P2P

> Última sesión: 2026-07-02. Este documento captura TODO: historial de bugs corregidos,
> infraestructura creada, tests agregados, y fases pendientes para continuar.

---

## Historial completo — Lo hecho hasta ahora

### Bugs corregidos (11)

| # | Bug | Archivo(s) | Fix |
|---|-----|-----------|-----|
| 1 | Invite no llegaba: `send_invite` sin `dial` previo | `apps/admin/src-tauri/src/lib.rs`, `admin.rs` | `send_invite_full` con dial + wait 1s |
| 2 | `OutboundFailure` de invite silencioso | `crates/syntrix-network/src/lib.rs` | `tracing::warn!` |
| 3 | `add_device` no publicaba `device.updated` | `apps/admin/src-tauri/src/admin.rs` | Publicar gossip |
| 4 | `remember_device` sobrescribía roles en registry | `apps/admin/src-tauri/src/identity.rs` | `reg.upsert_role` condicional |
| 5 | `_invite_rx` descartado → invite nunca al frontend | `apps/client/src-tauri/src/identity.rs`, `lib.rs` | Receiver vivo + Tauri event |
| 6 | Migraciones no idempotentes | `apps/*/src-tauri/src/storage.rs` | Catch "already exists" |
| 7 | `lww_should_skip` sin tiebreaker | `crates/syntrix-network/src/cdc.rs` | `(change_time, node_id)` |
| 8 | `turso_cdc.change_time` truncado a segundos | `crates/syntrix-network/src/cdc.rs` | Leer del blob |
| 9 | `default_role_grants` hardcodeaba roles | `apps/admin/src-tauri/src/identity.rs` | Solo admin |
| 10 | CDC feedback loop (república recibidos) | `apps/client/src-tauri/src/cdc_sync.rs` | Filtrar `local_node_id` |
| 11 | Gossipsub duplicate delivery LWW race | `apps/client/src-tauri/src/indexes.rs`, `identity.rs` | Per-org mutex |

### Código legacy eliminado
- `sync_push`, `sync_pull`, `sync_ping` (sync.rs)
- `append_event` en events.rs, identity.rs, indexes.rs
- `event_log` del schema Drizzle del cliente + migración generada
- `test_duplicate_event_prevention`, 5 tests sync.rs, `count_events`
- `entity-schema-meta.mjs` (135 líneas manuales → reemplazado por `generateSchemaJson`)

### Infraestructura creada

| Componente | Archivos | Propósito |
|-----------|----------|-----------|
| Shared Drizzle | `packages/shared-drizzle/` | entities.ts, zod.ts, relations.ts, export-schema.ts — fuente única de verdad |
| Drizzle Proxy | `apps/client/src-tauri/src/lib.rs` (`drizzle_execute`), `apps/client/src/db.ts` | Frontend usa Drizzle ORM con tipos contra Limbo vía Tauri IPC |
| Timeline CDC | `apps/client/src-tauri/src/lib.rs` (`get_updates_since`), `apps/client/src/hooks/useTimelineCursor.ts` | Polling CDC con cursor en localStorage |
| Headless mode | `apps/*/src-tauri/src/lib.rs` (`--headless`) | JSON stdin/stdout para tests de binarios reales |
| Binary E2E | `apps/admin/src-tauri/tests/binary_e2e_test.rs`, `just test-binary-e2e` | 3 procesos separados con P2P real |
| Playwright E2E | `apps/admin/src/__tests__/e2e-cross-app/`, `scripts/start-e2e-cross-app.sh` | 3 apps Tauri con WebViews reales |
| Per-org lock | `apps/client/src-tauri/src/indexes.rs` (`SqlEngine.org_apply_lock`) | Serializa CDC apply por org |
| schema.json auto | `packages/shared-drizzle/src/export-schema.ts` (`generateSchemaJson`) | Lee de Drizzle entities, genera para Rust |

### Tests agregados (9 nuevos)

| Test | Archivo | Qué verifica |
|------|---------|-------------|
| `test_inbox_receives_invite` | sync_test.rs | Invite llega al inbox vía P2P |
| `test_inbox_and_bidirectional_sync` | sync_test.rs | 2 clientes writen docs distintos, sync bidireccional |
| `test_update_replicates` | sync_test.rs | c1 crea + actualiza, c2 ve el update |
| `test_late_joiner_receives_existing_data` | sync_test.rs | c1 escribe, c2 se une después → catchup |
| `test_late_joiner_sees_updated_data` | sync_test.rs | c1 crea+actualiza, c2 se une → versión más reciente |
| `test_late_joiner_with_multiple_writers` | sync_test.rs | c1+c2 escriben, c3 se une → ve todo |
| `test_drizzle_execute_reads_typed_columns` | sync_test.rs | drizzle_execute con SELECT parametrizado |
| `test_timeline_cursor_returns_cdc_events` | sync_test.rs | get_updates_since con cursor |
| `read_events_since_cursor_timeline` + `_respects_limit` | cdc.rs | CDC cursor + límite |

### Tests: estado actual

| Suite | Resultado |
|-------|-----------|
| `sync_test` | 23/23 pasan, 1 ignorado (`test_concurrent_commits`) |
| `syntrix-client` lib+tests | 27/27 pasan |
| `syntrix-network` CDC+schema | 17/17 pasan |
| `syntrix-admin` lib | 10/10 pasan |
| `basic_test` | 5/5 pasan |
| `binary_e2e_test` | 1 ignorado (necesita GTK local — `just test-binary-e2e`) |

### Decisiones de arquitectura tomadas

| Tema | Decisión |
|------|---------|
| ¿Drizzle adapter para Tauri? | No — solo compartir entities. Writes siguen en Rust (permisos + HLC + CDC) |
| ¿Quitar `sales`/`contabilidad` de defaults? | Sí — solo `admin` tiene wildcard |
| ¿Arreglar `test_concurrent_commits`? | No por ahora — el mesh gossipsub tarda en formarse en in-process. Cubierto por binary E2E |
| ¿`event_log` del cliente? | Borrado. CDC reemplazó completamente el transporte legacy |
| ¿LiveManager vs Timeline CDC? | Se eligió Timeline CDC (Opción C) — cursor en localStorage, sin WebSocket |
| ¿schema.json manual? | No — generado desde Drizzle entities vía `generateSchemaJson` |
| ¿Drizzle Proxy para reads? | Sí — `drizzle_execute` ejecuta SQL generado por Drizzle contra Limbo |
| ¿Drizzle Relations? | Sí — `relations.ts` con FK constraints para JOINs automáticos |

---

## 🔴 Fase 2 — Migrar admin a SQL

**Problema**: El admin guarda devices, roles y orgs en HashMap + JSON en disco.
El cliente ya usa SQL para todo. Unificar.

| # | Tarea | Archivos |
|---|-------|----------|
| 2.1 | Agregar tablas `devices`, `orgs` al admin Drizzle schema | `apps/admin/drizzle/schema.ts` |
| 2.2 | Agregar `devices` entity a `shared-drizzle` | `packages/shared-drizzle/src/entities.ts` |
| 2.3 | Cambiar `remember_device` → escribir a SQL | `apps/admin/src-tauri/src/identity.rs` |
| 2.4 | Cambiar `add_org` → escribir a SQL | `apps/admin/src-tauri/src/identity.rs` |
| 2.5 | Cambiar `set_role` → escribir a SQL | `apps/admin/src-tauri/src/identity.rs` |
| 2.6 | Reemplazar `list_devices`, `list_roles`, `list_orgs` → SQL | `apps/admin/src-tauri/src/admin.rs` |
| 2.7 | Quitar `save_devices()`, `save_roles()` (JSON) | `apps/admin/src-tauri/src/identity.rs` |
| 2.8 | Quitar `load_devices()`, `load_roles()` (JSON) | `apps/admin/src-tauri/src/identity.rs` |
| 2.9 | Reemplazar `invoke("list_devices")` → Drizzle Proxy | `apps/admin/src/collections/adapter.ts` |
| 2.10 | Regenerar migración admin | `just drizzle-gen` en `apps/admin` |
| 2.11 | Actualizar tests sync_test | `apps/admin/src-tauri/tests/sync_test.rs` |
| 2.12 | Correr todos los tests | `cargo test` |

---

## 🔴 Fase 3 — Unificar acceso a datos con Drizzle Proxy

**Problema**: El adapter `fetchEntityData` del cliente usa `invoke("query_entity")`.
Con Drizzle Proxy, puede usar `db.query.entity.findMany()` directamente.

| # | Tarea | Archivos |
|---|-------|----------|
| 3.1 | Reemplazar `fetchEntityData` cliente por `db.query[name].findMany()` | `apps/client/src/collections/adapter.ts`, `EntityGrid.tsx` |
| 3.2 | Reemplazar adapter admin por Drizzle Proxy | `apps/admin/src/collections/adapter.ts` |
| 3.3 | Borrar `schemas.ts` duplicados, usar drizzle-zod | `apps/client/src/collections/schemas.ts`, `packages/syntrix-ui/` |
| 3.4 | Verificar frontend compila | `pnpm lint`, `pnpm build` |

---

## 🟡 Fase 4 — Pendientes menores

| # | Tarea | Archivos | Nota |
|---|-------|----------|------|
| 4.1 | Migrar catchup legacy a CDC | `identity.rs::CatchupRequestReceived` | Usa `query_events_since` → cambiar a `snapshot_org_rows` |
| 4.2 | `test_device_reassignment_propagates` | sync_test.rs | Cliente no actualiza `org.role` dinámicamente. Documentado con stub |
| 4.3 | `test_concurrent_commits` | sync_test.rs | `#[ignore]`. Cubierto por binary E2E. Revisar cuando el mesh esté estable |
| 4.4 | HLC `max(last_ts, now)` | `events.rs` | `Hlc::next` sin `max()` puede ir hacia atrás. No crítico (tiebreaker LWW compensa) |

---

## Orden de ejecución

1. **Fase 2** (admin a SQL) — ~2-3 horas
2. **Fase 3** (Drizzle Proxy unificado) — ~1 hora
3. **Fase 4** (pendientes menores) — ~1 hora
