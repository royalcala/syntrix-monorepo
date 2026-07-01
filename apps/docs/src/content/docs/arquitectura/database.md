---
title: "Motor de Base de Datos (Limbo)"
description: "Limbo (turso_core) como motor SQL único — embedded SQL + FTS + CDC"
---

# Motor de Base de Datos

Syntrix usa **Limbo** (crate `turso_core`) como motor de base de datos SQL embebido, reemplazando la arquitectura anterior basada en redb + tantivy + syntrix-schema.

## Stack

```
┌─ Aplicación (Rust/Tauri) ─────────────────────┐
│  Drizzle ORM (schemas TypeScript → SQL .sql)   │
│  include_str!("../migrations/*.sql")            │
│  turso_core::Connection (SQL embebido)          │
└────────────────────────────────────────────────┘
```

## Esquemas con Drizzle

Los schemas SQL se definen en TypeScript con Drizzle ORM en cada app:

- `apps/admin/drizzle/schema.ts`
- `apps/client/drizzle/schema.ts`

Para generar migraciones: `just drizzle-gen`

Esto produce archivos `.sql` en `apps/*/src-tauri/migrations/` que se ejecutan al arrancar via `include_str!`.

## Tablas

### Entity tables (cliente)

Cada entidad tiene su propia tabla SQL con estructura uniforme:

```sql
CREATE TABLE customers (
    org_id TEXT NOT NULL,
    doc_id TEXT NOT NULL,
    payload TEXT NOT NULL DEFAULT '{}',
    fts_title TEXT NOT NULL DEFAULT '',
    fts_body TEXT NOT NULL DEFAULT '',
    change_time INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
    node_id TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (org_id, doc_id)
);
```

Entidades: customers, suppliers, products, invoices, orders, payroll.

### Tablas de sistema

- `members` — miembros del colectivo
- `roles` — roles y permisos (can_open/can_write)
- `heartbeats` — latidos de presencia P2P
- `event_log` — auditoría de eventos con HLC
- `hlc_tracker` — deduplicación Last-Write-Wins

## CDC (Change Data Capture)

Limbo expone CDC nativo via `PRAGMA capture_data_changes_conn='full'`. Esto crea la tabla `turso_cdc` que captura automáticamente todo INSERT/UPDATE/DELETE.

El CDC se usa para:
- **Sync P2P**: el módulo `syntrix-network::cdc::read_cdc_events()` lee cambios desde `turso_cdc` y los envía a pares via libp2p request_response
- **Live Queries**: cuando hay cambios en una tabla, el `LiveManager` re-ejecuta SQL de suscripciones activas y emite eventos Tauri `live_update`
- **Auditoría**: el `event_log` mantiene un historial de eventos con HLC para compatibilidad con peers anteriores

## FTS (Full-Text Search)

Limbo soporta índices FTS nativos via tantivy integrado:

```sql
CREATE INDEX idx_customers_fts ON customers USING fts (fts_title, fts_body);
```

La búsqueda se realiza con `MATCH`:

```sql
SELECT doc_id, fts_title FROM customers WHERE (fts_title, fts_body) MATCH ? AND org_id = ?
```

## Consultas SQL

Todas las consultas van directo a SQL:

```rust
let mut stmt = conn.prepare("SELECT payload FROM customers WHERE org_id=?1 AND doc_id=?2")?;
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

Desde cliente, el `SqlEngine` envuelve la conexión y expone métodos equivalentes al anterior `RelationalEngine`.
