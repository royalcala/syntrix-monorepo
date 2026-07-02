---
title: "Motor de Base de Datos (Limbo)"
description: "Limbo (turso_core) como motor SQL relacional único — columnas tipadas + FTS + CDC nativo"
---

# Motor de Base de Datos

Syntrix usa **Limbo** (crate `turso_core`) como motor de base de datos SQL embebido. Tras la
migración relacional-CDC, las entidades ya **no** se guardan como blobs JSON: cada campo de
negocio es una columna SQL tipada, y la sincronización P2P lee directamente el CDC nativo de
Limbo (`turso_cdc`) en vez de retransmitir eventos JSON por gossip.

## Stack

```
┌─ Aplicación (Rust/Tauri) ─────────────────────────────────┐
│  Drizzle ORM (schemas TypeScript → SQL .sql, columnas     │
│  tipadas por entidad)                                     │
│  shared/drizzle/entity-schema-meta.mjs → schema.json       │
│  crates/syntrix-network/schema.json (registro de columnas)│
│  include_str!("../migrations/*.sql")                       │
│  turso_core::Connection (SQL embebido + CDC + FTS)         │
└─────────────────────────────────────────────────────────────┘
```

## Esquemas con Drizzle

Los schemas SQL se definen en TypeScript con Drizzle ORM en cada app:

- `apps/admin/drizzle/schema.ts`
- `apps/client/drizzle/schema.ts`

Cada entidad declara sus columnas de negocio explícitamente (no un blob genérico), más cuatro
columnas de sincronización compartidas: `org_id`, `doc_id`, `change_time`, `node_id`.

Para generar migraciones y el registro de columnas para Rust: `just drizzle-gen` (ejecuta
`drizzle-kit generate` en ambas apps y `pnpm export-schema`, que regenera
`crates/syntrix-network/schema.json` desde `shared/drizzle/entity-schema-meta.mjs`).

Esto produce archivos `.sql` en `apps/*/src-tauri/migrations/` que se ejecutan al arrancar via
`include_str!`.

## Tablas

### Entity tables (columnas tipadas)

Cada entidad tiene su propia tabla SQL con columnas de negocio reales:

```sql
CREATE TABLE customers (
    org_id TEXT NOT NULL,
    doc_id TEXT NOT NULL,
    name TEXT NOT NULL,
    tax_id TEXT,
    address TEXT,
    phone TEXT,
    email TEXT,
    change_time INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
    node_id TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (org_id, doc_id)
);
```

Entidades: customers, suppliers, products, invoices, orders, payroll.

### Tablas hijas (line items)

`invoices`/`orders` tienen tablas hijas propias para sus líneas de detalle
(`invoice_items`/`order_items`), cada una con su propio `PRIMARY KEY (org_id, <parent>_id,
line_id)` y columnas `change_time`/`node_id` — cada línea se sincroniza como su propia fila de
CDC, no como parte de un JSON anidado.

### Tablas de sistema

- `members` — miembros del colectivo (incluye el rol de cada dispositivo, usado para validar
  permisos de autor al aplicar CDC recibido)
- `roles` — roles y permisos (can_open/can_write)
- `heartbeats` — latidos de presencia P2P
- `event_log` (cliente) — historial local informativo de eventos propios, ya no es el
  transporte de sync
- `event_log` (admin) — **auditoría de datos**: una fila por cambio CDC recibido
  (`entity`/`change_type`/`doc_id`/`row_image`/`change_time`/`node_id`)
- `hlc_tracker` — deduplicación Last-Write-Wins para el path de escritura legacy
  (gossip/catchup/identity), hasta que el loop CDC lo sustituya por completo
- `cdc_cursor` (cliente) — último `turso_cdc.change_id` publicado por org, para reanudar el
  loop de sync tras un reinicio
- `saved_views` (admin) — consultas SQL favoritas de la consola SQL

## Registro de columnas (schema.json)

`shared/drizzle/entity-schema-meta.mjs` es la fuente única de columnas/tipos/flags
`searchable` por entidad (incluyendo tablas hijas). Se exporta a
`crates/syntrix-network/schema.json`, cargado en Rust vía `syntrix_network::schema` (re-exportado
como `syntrix_core::schema` para compatibilidad). Este registro permite:

- Construir `INSERT`/`SELECT` tipados genéricamente (`SqlEngine` en el cliente), sin
  hardcodear listas de columnas por entidad.
- Decodificar filas de `turso_cdc` posicionalmente (el orden de `schema.json` coincide con el
  orden físico de columnas de la tabla).
- Resolver alias de entidad (`"invoice"` vs `"invoices"`) de forma centralizada.

## CDC (Change Data Capture) — motor de sincronización

Limbo expone CDC nativo via `PRAGMA capture_data_changes_conn='full'`. Esto crea la tabla
`turso_cdc`, que captura automáticamente todo INSERT/UPDATE/DELETE como un blob de registro
binario estándar (mismo formato que una fila de tabla, decodificable con
`turso_core::types::ImmutableRecord`).

Hallazgos clave (ver `crates/syntrix-network/tests/cdc_format_probe.rs`):

- `change_type`: `1` = insert, `0` = update, `-1` = delete. `change_type == 2` es un
  **marcador de commit** de transacción (`table_name IS NULL`), no un delete — se descarta al
  leer.
- `turso_cdc` no tiene retención/poda automática; el crecimiento se gestiona a nivel de
  aplicación (cursor por org, sin purgado automático todavía).

El CDC-nativo reemplaza el transporte JSON-por-gossip anterior:

- `syntrix_network::cdc::read_cdc_events()` decodifica filas de `turso_cdc` a `CdcEvent`
  (columna por columna, según `schema.json`).
- `apps/client/src-tauri/src/cdc_sync.rs::run_cdc_publish_loop` escanea periódicamente
  `turso_cdc` desde el último cursor por org y publica los cambios via gossipsub.
- `syntrix_network::cdc::apply_cdc_events()` aplica el batch recibido: valida permisos del
  autor (`node_id` de la fila) contra `members`/`roles`, aplica LWW por `change_time`, y
  escribe columnas tipadas (incluye cascada de borrado a tablas hijas).
- **Live Queries**: cuando hay cambios en una tabla, el `LiveManager` re-ejecuta SQL de
  suscripciones activas y emite eventos Tauri `live_update`.
- **Auditoría (admin)**: cada `CdcEvent` aceptado se registra como fila de `event_log`
  (`gossip.rs::apply_cdc_batch`), y también se aplica a la réplica relacional del admin.

## FTS (Full-Text Search)

Limbo no implementa FTS5 estilo SQLite (tablas virtuales `CREATE VIRTUAL TABLE ... USING
fts5`) en la versión vendorizada; su feature `fts` integra **Tantivy** como un conjunto de
funciones escalares SQL (`fts_match`, `fts_score`, `fts_highlight`) que operan directamente
sobre columnas reales — no se necesita una tabla sombra.

```sql
SELECT doc_id, name, email, fts_score(name, email, ?1) AS score
FROM customers
WHERE org_id = ?2 AND fts_match(name, email, ?1)
```

Las columnas buscables por entidad están declaradas en `schema.json` (`searchable: true`);
`apps/client/src-tauri/src/search.rs` construye estas consultas genéricamente a partir del
registro, sin columnas `fts_title`/`fts_body` dedicadas.

## Consultas SQL

Las consultas van directo a columnas reales (sin `json_extract`):

```rust
let mut stmt = conn.prepare("SELECT name, email FROM customers WHERE org_id=?1 AND doc_id=?2")?;
stmt.bind_at(NonZero::new(1).unwrap(), Value::from_text(org_id.to_string()))?;
// ...
```

## Migraciones

Las migraciones son generadas por Drizzle y ejecutadas al arrancar en `storage.rs`:

```rust
pub fn run_migrations(conn: &Arc<turso_core::Connection>) -> anyhow::Result<()> {
    let sql = include_str!("../migrations/0000_*.sql");
    conn.execute(sql)?;
    conn.execute("PRAGMA capture_data_changes_conn='full'")?;
    Ok(())
}
```

Desde el cliente, el `SqlEngine` (`apps/client/src-tauri/src/indexes.rs`) envuelve la conexión
y expone proyecciones tipadas genéricas (`upsert_document_full`, `query`, `get_document`,
`delete_document`) construidas a partir de `schema.json`, más el cursor de CDC
(`get_cdc_cursor`/`set_cdc_cursor`) y la validación de permisos de autor
(`can_node_write`).

### Reset de bases de datos locales de desarrollo

Esta migración cambió el esquema físico de las tablas de entidades (columnas tipadas en vez
de `payload` JSON). No hay backfill automático desde bases de datos antiguas: si tienes datos
locales de una versión previa, bórralos antes de correr la app:

```bash
just clean-data-all       # borra datos de ambas apps (admin + client)
# o selectivamente:
just clean-data-admin
just clean-data-client
```

## Consola SQL (admin)

El admin incluye una consola SQL de solo lectura (`apps/admin/src/screens/SqlConsole.tsx`,
comando `run_sql`): acepta únicamente sentencias `SELECT`/`WITH` (se valida en
`sql_console.rs::validate_readonly_query`, rechazando `INSERT`/`UPDATE`/`DELETE`/`PRAGMA`/etc.
y sentencias apiladas), con paginación y **vistas guardadas** (`saved_views`). El "Audit
Trail" es ahora una vista guardada por defecto sobre `event_log`, en vez de una pantalla a
medida.
