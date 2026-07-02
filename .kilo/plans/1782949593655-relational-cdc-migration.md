# Migración a modelo relacional + sync CDC-nativo

## Objetivo

Eliminar el patrón "document-store disfrazado de SQL" (columna `payload` JSON + `json_extract`)
y reemplazarlo por **tablas relacionales tipadas** con **Drizzle como fuente única de verdad**,
y reemplazar el transporte de sync hecho a mano (event_log JSON por gossip) por el
**CDC nativo de Limbo** (`turso_cdc`).

Esto resuelve la causa raíz de la confusión actual: hoy los campos de negocio viven
duplicados/partidos entre Zod (frontend, 2 copias), un `payload` JSON opaco en la DB, y
migraciones Drizzle que solo definen un "sobre" genérico. La base paga el costo de SQL sin
cobrar sus beneficios (columnas tipadas, índices reales, constraints, joins, agregaciones).

## Contexto verificado (estado real del código, no el de los docs)

- **DB:** Limbo (`turso_core`), SQLite embebido. No hay redb. Cliente: `syntrix.db`. Admin: `syntrix-admin.db`.
- **Tablas de entidad hoy** (`customers`, `suppliers`, `products`, `invoices`, `orders`, `payroll`):
  columnas genéricas `org_id, doc_id, payload(JSON), fts_title, fts_body, change_time, node_id`,
  PK `(org_id, doc_id)`. Todo el negocio va en `payload`. Queries via `json_extract` sin índices
  (`apps/client/src-tauri/src/indexes.rs`).
- **Sync real hoy:** `commit_event` (`events.rs`) inserta JSON en `event_log`, hace `upsert_document`,
  y publica bytes JSON por gossipsub. `event_log` es transporte **y** auditoría.
- **CDC:** el PRAGMA `capture_data_changes_conn='full'` está activo (`storage.rs`), y existe
  `crates/syntrix-network/src/cdc.rs` con `read_cdc_events`/`apply_cdc_events`, **pero es código
  muerto y no-funcional**: nadie los llama, y `read_cdc_events` no decodifica el row image
  (`org_id/doc_id/payload` quedan hardcodeados a vacío/`Null`, `cdc.rs:56-59`).
- **`@tanstack/db`:** los adapters `packages/syntrix-ui/src/collections/{syntrix,tauri}-adapter.ts`
  no se importan en ningún lado. Código muerto. El flujo real es
  `fetchEntityData` → IPC `query_entity` → `@tanstack/react-query`.
- **Registry / upcasters:** aspiracionales. `syntrix_core::schema_version_for` siempre retorna `1`,
  `upcast_payload` es no-op, `get_schema_registry` devuelve `"fields": []`.
- **Zod duplicado:** `packages/syntrix-ui/src/collections/schemas.ts` y
  `apps/client/src/collections/schemas.ts` son idénticos. `admin-schemas.ts` para device/role/org.
- **Admin hoy:** su DB tiene **solo** `event_log`; escucha gossip y hace `INSERT` de cada evento
  (`gossip.rs`). No materializa entidades. `AuditTrail.tsx`/`Logs.tsx` consultan `event_log`.

## Decisiones tomadas

1. **Modelo relacional:** columnas reales tipadas por entidad, con índices y constraints.
2. **Line items anidados:** tablas hijas con FK — `invoice_items` (FK→`invoices`) y `order_items`
   (FK→`orders`). Cada line item es su propia fila que CDC sincroniza. Update de factura/orden =
   borrar+reinsertar sus hijos; LWW por fila hija.
3. **Fuente única de verdad = Drizzle** (`apps/*/drizzle/schema.ts`):
   - `drizzle-kit` genera el SQL de migración (ya existe el flujo `just drizzle-gen`).
   - `drizzle-zod` genera los esquemas Zod → se elimina la duplicación de los `schemas.ts`.
   - Se exporta un **JSON de columnas por entidad** que Rust lee para construir proyección/INSERT
     tipados de forma genérica (evita hardcodear columnas en Rust por entidad).
4. **Sync = CDC-nativo:**
   - Escrituras locales = SQL `INSERT/UPDATE/DELETE` en tablas tipadas (con check de permisos).
   - `turso_cdc` captura los cambios de fila.
   - Un loop lee `turso_cdc` desde el último `change_id` y envía los cambios por gossip.
   - Los peers aplican con `apply_cdc_events`: LWW por `change_time` + **validación de permisos del
     autor** + **manejo de deletes** (`change_type==2`).
   - Se elimina `json_extract` de las queries de lectura y el JSON como transporte.
5. **`event_log`:** deja de ser transporte. Queda **solo como auditoría/historial en admin**,
   poblado desde eventos CDC recibidos (fila: `tabla / INSERT|UPDATE|DELETE / doc_id / row image /
   change_time / node_id`). Nota: se pierde la semántica de negocio (`invoice.paid`); el audit
   pasa a ser data-audit por fila. El cliente ya no usa `event_log` como transporte.
6. **Admin = réplica relacional completa + consola SQL:**
   - Admin materializa las mismas tablas relacionales aplicando CDC recibido por gossip.
   - Nueva **consola SQL read-only**: comando Tauri `run_sql(query)` → filas → grid
     (`@tanstack/react-table`). **Guardrail: solo lectura**; las mutaciones deben ir por eventos/CDC,
     nunca por SQL directo (si no, no se propagan a los peers).
   - Tabla `saved_views` para guardar queries. El `AuditTrail` se vuelve una vista guardada sobre
     `event_log`.
7. **FTS:** FTS5 sobre columnas reales declaradas buscables en Drizzle. Se elimina la heurística
   `fts_title/fts_body` y `extract_title/extract_body`.
8. **Datos:** reset limpio de las DBs locales de dev. Sin backfill.
9. **Limpieza de código muerto:** borrar `syntrix-adapter.ts`, `tauri-adapter.ts`, la dep
   `@tanstack/db` (admin), y el path `sync_pull` / event_log-como-transporte.

## Fronteras afectadas

- `apps/client/drizzle/schema.ts`, `apps/admin/drizzle/schema.ts` (nuevas columnas + tablas hijas + FTS).
- `apps/*/src-tauri/migrations/*.sql` (regeneradas por drizzle-kit).
- `crates/syntrix-network/src/cdc.rs` (arreglar decode + apply con permisos/deletes).
- `apps/client/src-tauri/src/{events,sync,indexes,gossip,storage,lib}.rs`.
- `apps/admin/src-tauri/src/{audit,gossip,lib,storage}.rs` + nueva consola SQL.
- `crates/syntrix-core/src/lib.rs` (column registry / permisos si aplica).
- Frontend: `apps/*/src/collections/*`, `apps/*/src/entities/*`, `packages/syntrix-ui/src/collections/*`.

## Tareas (orden sugerido)

### Fase 0 — Investigación bloqueante
1. **Verificar experimentalmente el formato del row image de `turso_cdc`**: qué contienen las
   columnas `after` y `updates` (encoding: record binario, JSON, texto). Escribir un test que
   inserte una fila, lea `turso_cdc` y dumpee `after`. De aquí depende toda la decodificación.
2. Confirmar comportamiento de `change_type` (insert/update/delete) y de la poda/retención de
   `turso_cdc` (¿crece indefinido? ¿hay que truncar tras sync?).

### Fase 1 — Esquema (Drizzle SOT)
3. Redefinir `apps/client/drizzle/schema.ts` con columnas tipadas por entidad
   (`customers`, `suppliers`, `products`, `invoices`, `orders`, `payroll`), + metadatos de sync
   (`org_id`, `doc_id`/PK de negocio, `change_time`, `node_id`), + marcar columnas buscables (FTS).
4. Agregar tablas hijas `invoice_items` y `order_items` con FK e índices.
5. Definir columnas de sistema en admin (mantener `event_log`, agregar tablas de entidad + hijas
   + `saved_views`).
6. Generar migraciones con `just drizzle-gen`; verificar el SQL de salida.
7. Añadir exportación de un `schema.json` (columnas por entidad + flags buscables) consumible por Rust.
8. Configurar `drizzle-zod` para generar los Zod; eliminar los `schemas.ts` duplicados y repuntar imports.

### Fase 2 — Proyección tipada en Rust
9. Reemplazar `SqlEngine.upsert_document`/`upsert_document_with_hlc` por proyección tipada:
   mapear campos de negocio → columnas reales usando el `schema.json`. Escribir hijos
   (invoice_items/order_items) con delete+reinsert.
10. Reescribir `SqlEngine.query`/`get_document`/`delete_document` para columnas reales
    (sin `json_extract`); `query_entity` reconstruye el objeto JSON desde columnas para mantener
    la forma del IPC que ya consume el frontend.
11. Migrar el path de escritura (`commit_event`) para hacer SQL tipado + check de permisos;
    dejar de escribir `event_log` como transporte.
12. FTS: crear índice FTS5 sobre columnas buscables; actualizar `search_entity`/`search.rs` a `MATCH`
    sobre esas columnas; eliminar `extract_title/extract_body`.

### Fase 3 — Sync CDC-nativo
13. Arreglar `read_cdc_events` para decodificar el row image real (org_id, doc_id, columnas, node_id,
    change_time) según el hallazgo de Fase 0.
14. Conectar un loop que lea `turso_cdc` desde el último `change_id` por org y publique los cambios
    por gossip (reemplaza la publicación JSON de `events.rs`).
15. Endurecer `apply_cdc_events`: LWW por `change_time`, **validar permisos** del autor
    (`can_write` del rol vía `roles`), **manejar deletes** (`change_type==2`), y aplicar a columnas
    tipadas + hijos.
16. Integrar `catchup.rs` (snapshot de estado completo) para peers fuera de la retención de `turso_cdc`.
17. Persistir el cursor `change_id` por peer/org.
18. Eliminar `sync_pull` / `SyncPullResult` / uso de `event_log` como transporte.

### Fase 4 — Admin: réplica + consola SQL
19. Admin: aplicar CDC recibido por gossip a sus tablas relacionales (reusar la proyección de Fase 2).
20. Admin: seguir escribiendo `event_log` como auditoría a partir de los eventos CDC recibidos.
21. Nuevo comando `run_sql(query)` **read-only** (rechazar todo lo que no sea `SELECT`; idealmente
    conexión/transacción de solo lectura) + paginación.
22. Tabla `saved_views` + comandos CRUD; UI de consola SQL (grid + editor + guardar/cargar vistas).
23. Reescribir `AuditTrail`/`Logs` como una vista guardada sobre `event_log`.

### Fase 5 — Limpieza y frontend
24. Borrar `packages/syntrix-ui/src/collections/{syntrix,tauri}-adapter.ts` y la dep `@tanstack/db`.
25. Unificar consumo de Zod generado; ajustar `apps/*/src/entities/*` y colecciones.
26. Reset de DBs de dev (documentar el paso; sin backfill).

## Riesgos

- **Formato del row image `after` de `turso_cdc`** (Fase 0): mayor incertidumbre técnica; bloquea Fase 3.
- **Permisos en `apply_cdc_events`**: hoy inexistente; sin esto un peer malicioso/rol equivocado
  podría inyectar filas. Debe validarse contra `roles.can_write`.
- **LWW en filas hijas**: reconstrucción de line items con delete+reinsert puede competir con LWW por
  fila; definir clave de LWW por hijo (p.ej. `(org_id, invoice_id, line_id)` + `change_time`).
- **Retención/poda de `turso_cdc`** vs peers offline: sin catch-up, se pierden cambios.
- **Consola SQL**: garantizar realmente read-only para no romper consistencia P2P.
- **Deletes en CDC**: propagación correcta de borrados (hoy se ignoran).

## Validación

- Tests unitarios de proyección negocio→columnas (incluye hijos invoice_items/order_items).
- Test de Fase 0 que dumpea `turso_cdc.after` y valida el decode de `read_cdc_events`.
- Round-trip CDC entre 2 nodos: write → `turso_cdc` → read → gossip → `apply` → estado igual.
- LWW multi-peer (escrituras concurrentes al mismo doc convergen determinísticamente).
- Deletes se propagan y borran en peers.
- Permisos: rol Sales no puede escribir Payroll ni via commit ni via CDC recibido.
- FTS sobre columnas reales devuelve resultados correctos.
- Consola SQL admin: `SELECT` funciona; `INSERT/UPDATE/DELETE/PRAGMA` se rechazan.

## Preguntas abiertas menores (no bloquean)

- Confirmar la lista exacta de columnas **buscables** (FTS) por entidad.
- Clave de LWW definitiva para filas hijas de line items.
- Si el admin debe conservar además un audit semántico (nombre de evento) o basta el data-audit por fila.
