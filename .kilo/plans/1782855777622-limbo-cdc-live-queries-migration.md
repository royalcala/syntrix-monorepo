# Plan: Limbo + CDC + Live Queries Migration

## Objetivo

Reemplazar `redb` + `tantivy` + `syntrix-schema` con Limbo (`turso_core`) embebido como engine SQL único, usando CDC para sync P2P y FTS nativo para búsqueda.

## Decisiones de diseño

| Decisión | Elección |
|----------|----------|
| DB engine | Limbo (`turso_core`) embebido por peer |
| Sync data | CDC-only + LWW por `(table, row_id, change_time, node_id)` |
| Sync transporte | Híbrido: gossipsub anuncia cambio, request_response pull CDC |
| Sync permisos | Gossipsub separado (ya implementado) |
| FTS | Limbo FTS nativo: `CREATE INDEX ... USING fts` |
| Live queries | CDC-watcher → re-ejecutar SQL → Tauri `emit("live_update")` |
| Migraciones | Drizzle genera `.sql` → `include_str!` en binario Rust |
| Queries frontend | SQL directo desde React via Tauri commands |
| DDL sync | Cada binario lleva sus migraciones, no se syncronizan entre peers |

## Arquitectura resultante

```
crates/
├── syntrix-core/        # simplificado: addr, registry, heartbeat, can_access
├── syntrix-network/     # extendido: + SyncEngineIo CDC adapter
└── syntrix-logging/     # sin cambios

apps/
├── client/src-tauri/    # reescrito: redb → turso_core, tantivy → Limbo FTS
└── admin/src-tauri/     # reescrito: redb → turso_core
```

**Eliminado**: `crates/syntrix-schema/` (completo), `crates/syntrix-testkit/` (simplificado)

## Stack final por capa

```
┌────────────────────────────────────────────┐
│  React UI (Drizzle ORM → queries → Tauri)  │
│  liveQuery() hook → listen("live_update")  │
├────────────────────────────────────────────┤
│  Tauri v2 (commands + liveness)            │
├────────────────────────────────────────────┤
│  syntrix-core (addr, registry, heartbeat)  │
│  syntrix-network (libp2p + CDC adapter)    │
├────────────────────────────────────────────┤
│  turso_core (Limbo: SQL + FTS + CDC)       │
└────────────────────────────────────────────┘
```

## Plan de implementación

### Fase 1: Integrar `turso_core` en el workspace

1. Agregar `turso_core` como dependency en `syntrix-client` y `syntrix-admin` `Cargo.toml`
2. Referenciar el fork local: `turso_core = { path = "../../../../turso/core" }`
3. Crear `storage.rs` en ambos apps con helper `open_limbo(data_dir) -> turso_core::Connection`
4. Ejecutar migraciones al arrancar:
   ```rust
   let sql = include_str!("../migrations/001_customers.sql");
   conn.execute(sql)?;
   ```

### Fase 2: Reemplazar `RelationalEngine` (redb) por SQL

5. Migrar tablas de redb a `CREATE TABLE` SQL:
   - `DOCUMENTS` → tables por entidad (customers, invoices, etc.)
   - `EVENT_LOG` → `turso_cdc` (nativo de Limbo)
   - `MEMBERS` → `members` table
   - `ROLES` → `roles` table
   - `HEARTBEATS` → `heartbeats` table
   - `HLC_TRACKER` → columna `change_time` + `node_id` en CDC
6. Reemplazar `query_entity()` → `conn.query("SELECT * FROM {entity} WHERE ...", params)`
7. Reemplazar `append_event()` → usar CDC nativo (PRAGMA capture_data_changes_conn)
8. Reemplazar `upsert_document()` → SQL `INSERT OR REPLACE`
9. Eliminar `indexes.rs`, `RelationalEngine`, todas las constantes `TableDefinition`

### Fase 3: Migrar FTS de tantivy a Limbo nativo

10. Eliminar `search.rs`, `SearchEngine`, dependencia `tantivy`
11. Crear índices FTS en migraciones SQL:
    ```sql
    CREATE INDEX customers_fts ON customers USING fts (name, notes)
      WITH (tokenizer='default', weights='name=2.0,notes=1.0');
    ```
12. Reemplazar `indexer.search_engine.search()` → `SELECT *, fts_highlight(name, ?) FROM customers WHERE name MATCH ?`
13. Reemplazar `SearchResult` struct → rows directos de SQL

### Fase 4: Sync CDC sobre libp2p

14. Crear `sync/cdc.rs` en `syntrix-network` con:
    - `CdcSyncAdapter` struct
    - `push_cdc_changes(conn, peer)` — lee `turso_cdc`, envía via request_response
    - `pull_cdc_changes(conn, peer, since_change_id)` — recibe cambios, aplica LWW
    - LWW resolve: `(row_id, change_time DESC, node_id ASC)` → gana el más reciente
15. Implementar `SyncEngineIo` trait sobre request_response de libp2p:
    - Reemplazar HTTP con conexiones P2P directas
    - Manejar retry y timeout
16. Gossipsub announce: después de un write local, publicar `{table, max_change_id}`
17. Peers escuchan announce → si les interesa la tabla → `pull_cdc_changes(since=...)`
18. Validación de permisos en push: antes de enviar CDC, verificar `can_write` del peer local
19. Validación de permisos en pull: antes de aplicar CDC, verificar `can_write` del peer remoto

### Fase 5: Live Queries

20. Crear `live.rs` con:
    ```rust
    struct LiveSubscription { id: u64, sql: String, tables: Vec<String>, last_result: Vec<Value> }
    HashMap<u64, LiveSubscription>
    ```
21. `#[tauri::command] fn live_subscribe(sql: String, depends_on: Vec<String>) -> u64`
22. `#[tauri::command] fn live_unsubscribe(id: u64)`
23. CDC-watcher: después de cada `push_cdc_changes` o `pull_cdc_changes`, escanear tablas afectadas, cruzar con `depends_on`, re-ejecutar SQL, emitir `app.emit("live_update", {id, rows})`
24. Frontend React: `useLiveQuery({ sql, dependsOn })` hook con Drizzle

### Fase 6: Drizzle migrations

25. Configurar `drizzle.config.ts` en `apps/admin/` y `apps/client/`
26. Crear `apps/admin/drizzle/schema.ts` con el schema SQL
27. `drizzle-kit generate` → `apps/admin/src-tauri/migrations/*.sql`
28. Rust: `include_str!("../migrations/001_customers.sql")` ejecutado al arrancar

### Fase 7: Eliminar `syntrix-schema`

29. Mover `can_access()` a `syntrix-core`:
    ```rust
    pub fn can_access(role_grants: &[String], entity: &str) -> bool {
        role_grants.iter().any(|g| g == "*" || g == entity)
    }
    ```
30. Eliminar crate `syntrix-schema/` del workspace
31. Eliminar imports de `syntrix_schema` en apps y tests
32. Verificar que `cargo check --workspace` pasa

### Fase 8: Tests y limpieza

33. Actualizar `syntrix-testkit`: eliminar dependencias de redb/tantivy/syntrix-schema
34. Actualizar `e2e_sync_test.rs` y `basic_test.rs` para usar Limbo
35. Eliminar `redb`, `tantivy`, `syntrix-schema` de todos los `Cargo.toml`
36. `cargo check --workspace --tests`
37. Verificar que docs `apps/docs/` reflejan el nuevo stack

## API de live queries (frontend)

```typescript
// Hook con Drizzle
const { data, status } = useLiveQuery(db.select().from(customers), ["customers"]);

// data: T[]  — resultado actual del query
// status: "loading" | "live" | "stale"
```

## Riesgos y mitigaciones

| Riesgo | Mitigación |
|--------|-----------|
| Limbo en early stage (CDC puede tener bugs) | Fork local, tests de integración CDC primero |
| `turso_cdc` no expone columnas extra (node_id) | Extender el PRAGMA Initn para aceptar columnas adicionales |
| LWW pierde historia en conflictos | Guardar ambas versiones en audit log antes de resolver |
| FTS requiere recrear índice al migrar | Ejecutar `REINDEX` en migración inicial |
| CDC: `turso_cdc` captura rowid interno, no `(org_id, doc_id)` | `CdcEvent` mapea CDC crudo a alto nivel (org_id, entity, doc_id, payload) consultando la tabla destino |
| Drizzle schema y SQL de Limbo divergen | Drizzle es fuente de verdad, SQL se regenera en cada migración (`just drizzle-gen`) |
| Migraciones duplicadas (manuscritas vs Drizzle) | Eliminar `migrations/*.sql` escritos a mano; solo conservar los generados por `drizzle-kit` (prefijo `0000_*`). `storage.rs` debe usar `include_str!("../migrations/0000_*.sql")` |
| Peers con distinta versión de esquema | Las migraciones deben ser **forward-compatible** (solo ADD, nunca DROP/RENAME). Incluir `schema_version` en heartbeat/gossip. Si un peer recibe CDC de una tabla/columna que no existe localmente, ignorar ese cambio. Para breaking changes futuros: implementar version negotiation via gossip con `min_schema_version`; peers por debajo de ese mínimo no pueden sincronizar hasta actualizar. Esto evita corrupción de datos y asegura que todos los peers en el mesh tengan un esquema compatible. |
| P2P: `PeerId::from_bytes` falla con `invalid multihash` en tests e2e | Fixed: `peer_id_to_bytes` ahora extrae los 32 bytes de la llave pública Ed25519 desde el encoding protobuf completo. `send_invite` reconstruye PeerId vía `PeerId::from_str(peer_id_base58)` del campo `peer_id` en endpoint JSON. |
| Tests e2e fallan con `no invites received` | Problema de timing P2P: el invite no llega al cliente dentro del sleep de 500ms. Requiere esperar conexión directa entre peers antes de enviar invites, o implementar retry con backoff en `invite_one_client`. |
