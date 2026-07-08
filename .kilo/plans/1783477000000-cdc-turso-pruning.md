# G5 — Pruning/retención de `turso_cdc`

## Por qué deberíamos hacerlo

Cada cambio local (crear/editar cliente, factura, etc.) hace que Limbo escriba un "recibo" del
cambio en una tabla oculta `turso_cdc`. El publish loop lee esos recibos y los gossipea a los
peers para sincronizar. **Limbo nunca los borra solos** — confirmado por el propio test
`crates/syntrix-network/tests/cdc_format_probe.rs:296` ("turso_cdc must not auto-prune") y el
comentario `crates/syntrix-network/src/cdc.rs:14`. Por lo tanto `turso_cdc` **crece sin límite**:
todo cambio histórico se acumula → gasta disco y ralentiza las lecturas.

- No es urgente en dev/demo (nada se rompe hoy).
- **Sí es necesario antes de producción**, sobre todo para los "fierros huecos" siempre-encendidos
  de Fase C: ahí "crece para siempre" es una bomba de tiempo.
- Prioridad: **por debajo** del plan de migraciones/refactor (`1783465859144-db-migrations-drizzle-refactor.md`),
  que sí toca correctness. Este es una salvaguarda de mantenimiento/escala.

## Por qué es seguro y simple

El catch-up es **basado en snapshot, no en replay histórico**: un peer que se une o reconecta
pide `request_catchup` y recibe una **foto del estado actual** (`snapshot_org_rows`), aplicada con
`apply_cdc_events` (`catchup.rs`). Nadie relee `turso_cdc` histórico. El **único** lector de
`turso_cdc` es el publish loop local, vía el cursor por-org (`cdc_cursor`, `indexes.rs:730`). Una
vez que ese cursor pasó un `change_id`, esa fila **no se vuelve a leer jamás** → se puede borrar.

## Decisiones fijadas

- Estrategia: `DELETE FROM turso_cdc WHERE change_id <= watermark`.
- `watermark = MIN(last_change_id)` **solo sobre orgs activos** (los presentes en `cdc_topics`),
  **y** limpiar la fila de `cdc_cursor` al salir de un org. Evita que un org muerto congele la poda.
- **No** hay ACKs por-peer ni retención por tiempo/tamaño (esa última es insegura: podría borrar
  writes locales aún no publicados, que los peers ya conectados nunca recibirían).
- Lógica compartida en `crates/syntrix-network` (donde vive `cdc.rs`), llamada por client y admin.

---

## Tareas (orden de ejecución)

1. **Función de poda** en `crates/syntrix-network/src/cdc.rs`:
   - `prune_cdc(conn: &Arc<turso_core::Connection>, watermark: u64) -> anyhow::Result<u64>`
     que ejecuta `DELETE FROM turso_cdc WHERE change_id <= ?1` y devuelve el nº de filas borradas.
   - Adquirir `db_lock` durante la operación (igual que las demás escrituras).
   - No-op si `watermark == 0`.

2. **Cálculo del watermark** (en `SqlEngine`, `indexes.rs`, junto a `get_cdc_cursor`):
   - `pub fn min_active_cdc_cursor(&self, active_orgs: &[String]) -> anyhow::Result<u64>`:
     `SELECT MIN(last_change_id) FROM cdc_cursor WHERE org_id IN (...)`. Si no hay orgs activos → `0`.
   - Método `pub fn prune_cdc_up_to(&self, watermark: u64)` que llama a `cdc::prune_cdc`.

3. **Limpieza de cursor al salir de un org**:
   - En el flujo de "leave/remove org" (buscar donde se remueve de `cdc_topics`), borrar también
     la fila de `cdc_cursor` de ese org: `DELETE FROM cdc_cursor WHERE org_id = ?1`.
   - Si hoy no existe un flujo de "leave org", dejar anotado como dependencia (la poda funciona
     igual; solo el edge de org obsoleto queda sin cubrir hasta que exista ese flujo).

4. **Disparo de la poda (piggyback + throttle)** en `apps/client/src-tauri/src/cdc_sync.rs`
   `run_cdc_publish_loop`:
   - Tras publicar los batches de todos los orgs, cada N ciclos (contador local, ej. cada 20),
     calcular `active_orgs` desde `cdc_topics`, obtener `min_active_cdc_cursor`, y llamar
     `prune_cdc_up_to(watermark)`.
   - Log a nivel debug con filas borradas.

5. **Equivalente en admin**: el admin también corre CDC (`apps/admin/src-tauri/src/gossip.rs`,
   publish loop propio). Cablear la misma poda en su loop de mantenimiento.

---

## Spikes de seguridad (verificar en implementación, antes de activar en release)

- **CDC recursivo**: confirmar que turso **no captura** el propio `DELETE` sobre `turso_cdc`
  (que borrar recibos no genere nuevos recibos). Fork: `/home/alcala/Documents/github/turso`.
- **Contador `change_id`**: confirmar que **no se reinicia** tras borrar filas (que siga monotónico;
  ver `turso_cdc_version` en `core/translate/pragma.rs`). Si se reiniciara, la poda por `change_id`
  sería insegura.

## Edge cases

- Sin orgs activos → `watermark = 0` → no se borra nada (seguro).
- Org recién unido con cursor 0 (aún no publicó) → `MIN = 0` → no se borra nada (seguro, conservador).
- Peer offline que reconecta → sigue funcionando: recibe estado completo por snapshot (`catchup.rs`),
  no depende de `turso_cdc` histórico.
- Org obsoleto en `cdc_cursor` → mitigado por scoping a orgs activos + limpieza en task 3.

## Validación

- Test unitario en `cdc.rs`: insertar N cambios con CDC on, publicar (avanzar cursor), llamar
  `prune_cdc(watermark)`, verificar que `SELECT COUNT(*) FROM turso_cdc` bajó y que las filas
  restantes son solo `change_id > watermark`.
- Test de convergencia: tras publicar + podar en el nodo A, un nodo B que aplica el batch queda
  convergido (los datos siguen correctos aunque los recibos ya no estén en A).
- Test de catch-up post-poda: un peer que se une **después** de podar recibe el estado completo
  vía `snapshot_org_rows`/`request_catchup` (no depende de `turso_cdc`).
- `just test-rust` en verde.

## Fuera de alcance

- Cambios al transporte de sync o al formato de `CdcEvent`.
- Retención configurable por política de negocio (solo se implementa la poda segura por watermark).
- Todo lo del plan de migraciones/refactor (`1783465859144-db-migrations-drizzle-refactor.md`).
