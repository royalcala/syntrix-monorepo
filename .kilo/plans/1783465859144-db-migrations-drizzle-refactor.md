# Refactor de migraciones + rol de Drizzle + acceso a datos

## Objetivo

Endurecer la capa de base de datos y quitar peso muerto, **sin cambiar de motor**.
Mantener **Limbo/turso** (su CDC nativo `turso_cdc` y su FTS `fts_match`/`fts_score` son la
columna vertebral del sync P2P y de la búsqueda; reemplazarlos por rusqlite/sqlx obligaría a
reconstruir CDC + FTS y es el mayor riesgo posible sin beneficio). Mantener **Drizzle solo como
fuente declarativa del schema** (genera el `.sql` de migraciones y el `schema.json` que alimenta
a la IA) y eliminar su capa ORM en runtime.

Estamos en desarrollo: **se pueden borrar las DBs locales y las migraciones actuales.**

## Decisiones fijadas

- Motor: **turso_core (Limbo)**, sin cambios. rusqlite/sqlx = fuera de alcance (riesgo).
- Drizzle: **solo schema source** (`entities.ts` + `schema.ts` → drizzle-kit). Fuera el ORM runtime.
- Migraciones: **squash a un `0000` limpio** regenerado desde el schema actual. Sin baseline/backfill.
- Runner: descubrimiento automático (nunca más editar `storage.rs` por migración).
- Acceso a datos frontend, partido por **origen del dato**:
  - Datos de entidad (viven en turso) → **un comando SQL genérico** desde el frontend.
  - Estado de red/runtime (orgs unidas, peers, sync, identidad) → **se queda como comando** (no está en la DB; vive en el `NamespaceRegistry` del nodo P2P, ej. `identity.rs:241`, `identity.rs:404`).
  - Escrituras → **helper tipado** que estampa `change_time`/`node_id` (evita el bug de no-replicación descrito en Riesgos).

## Confirmado en el fork de turso (`/home/alcala/Documents/github/turso`, `COMPAT.md`)

- `BEGIN`/`COMMIT`/`ROLLBACK`/`END` ✅ · `CREATE TABLE`/`ALTER TABLE` ✅ · `CREATE TABLE IF NOT EXISTS` ✅.
- `turso_cdc` es tabla real; `capture_data_changes_conn` es **por conexión** y se activa después de migrar → las escrituras de `__migrations` no se capturan ni se gossipean (el lector CDC solo escanea `schema::all_physical_tables()`).

---

## Tareas (orden de ejecución)

### G1 — Runner de migraciones robusto (crítico)

1. Crear crate `crates/syntrix-migrate` con:
   - `run_migrations(conn: &Arc<turso_core::Connection>, dir: &include_dir::Dir, journal_json: &str)`.
   - Crea `__migrations (idx INTEGER PRIMARY KEY, tag TEXT NOT NULL, applied_at INTEGER NOT NULL)` con `IF NOT EXISTS`.
   - Ordena por las entradas de `_journal.json` (`idx`, `tag`); mapea `tag` → `<tag>.sql` embebido.
   - Aplica **solo pendientes** (las que no están en `__migrations`). Por cada una:
     `BEGIN` → split por `--> statement-breakpoint` y ejecutar cada statement → `INSERT INTO __migrations` → `COMMIT`. Si algo falla: `ROLLBACK` + `bail!`.
   - **Quitar** el swallow de `"already exists"`. Mantener `PRAGMA foreign_keys=OFF` durante migración (defensivo; el baseline squasheado no tendrá FKs, así que el skip de `"foreign key mismatch"` deja de ser necesario).
   - Al terminar todas: `PRAGMA capture_data_changes_conn='full'` (como hoy).
2. Cablear en `apps/client/src-tauri/src/storage.rs` y `apps/admin/src-tauri/src/storage.rs`:
   - Reemplazar el cuerpo de `run_migrations` por una llamada al crate, pasando
     `include_dir!("$CARGO_MANIFEST_DIR/../migrations")` y `include_str!("../migrations/meta/_journal.json")`.
   - Eliminar la lista hardcodeada de `include_str!` + `run_single_migration`.
   - Añadir dep `include_dir` (y `syntrix-migrate`) en ambos `Cargo.toml`.

### G2 — Squash de migraciones (aprovechando que se puede borrar todo)

3. Borrar `apps/client/src-tauri/migrations/*` y `apps/admin/src-tauri/migrations/*` (incluye `meta/`).
4. Borrar las DBs locales de desarrollo (`dirs_next::data_dir()/syntrix*/…`, `syntrix.db` / `syntrix-admin.db`).
5. Regenerar baseline limpio: `just drizzle-gen` (corre `drizzle-kit generate` en ambas apps) → produce `0000_*.sql` + `meta/_journal.json` nuevos. Verifica que el `0000` no contenga `DROP TABLE` ni rebuilds.
6. Actualizar tests que referencian archivos de migración por nombre:
   - `apps/client/src-tauri/tests/common/mod.rs` (`run_migration_0003`, `include_str!("../../migrations/0003_...")`).
   - `apps/client/src-tauri/tests/ai_views_test.rs` (múltiples `run_migration_0003`).
   - Cambiarlos a correr el runner completo (`storage::run_migrations`) o el nuevo `0000`.

### G3 — `schema.json` de fuente única

7. Canonizar `packages/shared-drizzle/src/export-schema.ts::generateSchemaJson` como el único generador.
   - `apps/client/scripts/export-schema-json.ts` ya lo usa → sin cambios.
   - Reescribir `apps/admin/scripts/export-schema-json.mjs`: hoy importa `../../../shared/drizzle/entity-schema-meta.mjs` (**ruta inexistente → generador roto**). Que use `generateSchemaJson` de `@syntrix/shared-drizzle` igual que el client, escribiendo el mismo `crates/syntrix-network/schema.json`.
   - Ajustar `justfile` (`drizzle-gen`, línea ~238) si hace falta para que ambos `export-schema` produzcan contenido idéntico.

### G4 — Limpiar acceso a datos y ORM muerto

8. Eliminar la capa ORM runtime del client:
   - Borrar `apps/client/src/db.ts` (proxy `drizzle-orm/sqlite-proxy`).
   - Borrar `packages/shared-drizzle/src/relations.ts` y `packages/shared-drizzle/src/zod.ts` (código muerto).
   - Quitar sus re-exports de `packages/shared-drizzle/src/index.ts`.
   - Quitar la dep `drizzle-zod` de `packages/shared-drizzle`. **Mantener** `drizzle-orm` (lo usan `entities.ts`/`schema.ts` para drizzle-kit).
9. Reescribir `apps/client/src/collections/adapter.ts::fetchEntityData`:
   - En vez de `db.query.X.findMany({where})`, llamar al **comando genérico de lectura** de Rust con SQL (`SELECT ... FROM <tabla> WHERE org_id=?`).
   - Quitar imports de `drizzle-orm` (`eq`, `and`, `SQL`) y de `db`.
10. Comando de lectura único (Rust): estandarizar el frontend en un solo comando que ejecute SQL y devuelva filas como **objetos con columnas nombradas y tipadas**.
    - `apps/client/src-tauri/src/indexes.rs::drizzle_execute` hoy devuelve **todo como string** (`row.get::<String>` en 32 cols) → **bug**: números llegan como texto. Al quitar el ORM (que antes coercía tipos), corregirlo para devolver tipos (patrón de `execute_sql_query`, `indexes.rs:833`) o migrar el frontend a un comando que ya lo haga.
    - Unificar los tres caminos de lectura actuales del frontend (`db.query`, `invoke("query_entity")` en `App.tsx:148`, `invoke("drizzle_execute")` en `HomeScreen.tsx:30`/`useIAQueue.ts`) a este comando.
11. Escrituras por helper tipado:
    - Canalizar las escrituras de UI por `commit_event` (`events.rs`) o un helper `upsert_entity(entity, org_id, doc_id, fields)` que estampe `change_time` y `node_id` automáticamente.
    - Migrar `save_view` (`indexes.rs:255`) para que use el helper en vez de `INSERT` crudo vía `drizzle_execute`.
    - **No** exponer INSERT/UPDATE crudo desde el frontend.
12. Dejar intactos los comandos de estado de red (`list_orgs`, sync info, identidad): no son datos de DB.
    - Nota: `EntityDetailComponent.tsx` usa `component.relations` (campo del catálogo UI, **no** las relations de Drizzle) → no tocar.

---

## Riesgos y mitigación

- **DDL transaccional en turso**: `BEGIN/COMMIT/ROLLBACK` existen, pero verificar en impl que un DDL que falla a mitad revierte limpio. Mitigación: al poder wipear, un fallo en dev se corrige y se re-corre sobre DB fresca.
- **Bug de no-replicación en escrituras crudas**: un `INSERT` del frontend que olvide `node_id` lo deja en `""` (default). El write se captura y gossipea, pero el peer corre `perm.can_write("", entity)` (`cdc.rs:318`) → **lo rechaza en silencio**. Por eso G4-11 (helper que estampa sync-meta) es obligatorio, no opcional.
- **Tests con nombres de migración hardcodeados** (G2-6): romperán si no se actualizan al nuevo baseline.
- **`drizzle_execute` all-strings** (G4-10): al quitar el ORM, el frontend deja de recibir tipos coercidos; corregir el comando o los consumidores.
- **FK workaround**: el baseline squasheado desde `entities.ts` no declara FKs, así que el skip de `"foreign key mismatch"` puede eliminarse; mantener `foreign_keys=OFF` es inofensivo.

## Validación

- `just drizzle-gen` genera un único `0000` + `_journal.json` por app (sin `DROP TABLE`/rebuilds).
- `just test-rust`: pasan los tests de `cdc.rs`, `indexes.rs` y los binarios que llaman `run_migrations`.
- `just test` (vitest): lecturas/escrituras del frontend funcionan por el comando único.
- Manual runner: DB fresca arranca → `__migrations` tiene 1 fila; segundo arranque no aplica nada; añadir una migración dummy → se aplica una sola vez.
- Manual P2P: una escritura desde el frontend se replica a un peer (con `node_id` estampado).

## Fuera de alcance (planes/decisiones aparte)

- **G5 — Pruning/retención de `turso_cdc`** (crecimiento ilimitado, confirmado por `cdc.rs:14` y el test `cdc_format_probe.rs:296`). Requiere coordinar el cursor mínimo confirmado entre peers → **plan propio**.
- **Cambio de motor** a rusqlite/sqlx: rechazado; riesgo documentado (reconstruir CDC + FTS).
- Divergencia de tablas admin-específicas vs compartidas más allá de `schema.json`.
