# Plan: Rearquitectura a Zenoh (rama nueva) — Sync por replicación, Hub/Anchor móvil + IoT

> **Estado**: Dirección aprobada. Rama nueva `zenoh-rearch`, migración por fases detrás de interfaces (transporte + store). No es big-bang del producto.
> **Fecha**: 2026-07-10.
> **Relacionado**: ADR `1783695719137-adr-libp2p-vs-zenoh.md` (a actualizar), `1783695719135-adr-libp2p-vs-headscale.md`, plan hardening libp2p `1783695719136-p2p-connectivity-hardening-plan.md` (mantiene vivo `main`).
> **Ámbito**: capa de red/transporte, **motor de sincronización** y **capa de persistencia**. NO toca lógica de negocio ni UI (salvo merge admin→cliente, fase Z7).

## 0. Decisión y racional

**Adoptar Eclipse Zenoh (v1.9.x) como transporte + motor de sincronización, y SQLite (rusqlite) como base de datos**, reemplazando libp2p **y** el stack Turso/CDC actual. Rama nueva, por fases. La topología ya decidida es **hub-and-spoke** (ancla siempre-activa por org), que es el modelo nativo de Zenoh (client→router). Verificado en el fork `/home/alcala/Documents/github/zenoh`:

1. **Zenoh usa HLC (`uhlc`) nativo** (`commons/zenoh-protocol/src/core/mod.rs:29`) — mismo modelo LWW que se necesita. Cero impedancia.
2. **La replicación de Zenoh es LogLatest + anti-entropy por Fingerprints XOR** sobre eras `hot/warm/cold` (`plugins/zenoh-plugin-storage-manager/src/replication/`), con **un Event por key expression** (LWW-per-key).
3. **El `Storage` trait es implementable sobre SQLite** (`plugins/zenoh-backend-traits/src/lib.rs:223`): `put(key,payload,encoding,timestamp)`, `delete(key,timestamp)`, `get(key,params)`, `get_all_entries()`.
4. NAT móvil (CGNAT): clientes conectan **outbound** al router/ancla → sin hole punching. IoT: `zenoh-pico` (C, MCU).

## 1. Decisiones firmes (resueltas en discusión)

- **DEC-1 — Motor de sync = replicación Zenoh.** El storage-manager + replication de Zenoh es el motor de anti-entropy. **Elimina** por completo la capa CDC+catchup manual del sistema actual (lectura de `turso_cdc`, `read_cdc_events`, `snapshot_org_rows`, `cdc_sync.rs`). La convergencia entre nodos la da la reconciliación por fingerprints de Zenoh.
- **DEC-2 — Base de datos = SQLite vía `rusqlite`.** SQLite es maduro, probado en Android/iOS/ARM, con FTS5 nativo, compilado con el feature `bundled` (sin dependencia del sistema). **Reemplaza a `turso_core`.** Al mover el sync a Zenoh (DEC-1), desaparece la única razón por la que se necesitaba Turso (su CDC nativo), lo que habilita este cambio.
- **DEC-3 — DB detrás de un trait `Store`.** El acople real de la app es a `commit_event` + proyección a columnas + FTS + query. Se abstrae en un trait `Store` (testabilidad + libertad futura), con una única impl de producción: `syntrix-store-sqlite`.
- **DEC-4 — Identidad de autor = Ed25519 a nivel app.** Zenoh no ofrece identidad de autor por-fila: su `auth_pubkey` es RSA/PKCS1 con gestión de llaves incompleta (`@TODO: populate lookup file`) y su ACL identifica subjects por `cert_common_names`/`usernames`/`zids` (ZID "should not be used in production" para ACL). Por tanto **Ed25519 sigue firmando/validando filas y permisos por rol** a nivel app; el ACL/TLS de Zenoh es defensa en profundidad de red.
- **DEC-5 — Una sola fuente de verdad relacional.** El backend de storage de Zenoh **ES** la DB SQLite proyectada a columnas; no hay segunda DB. Zenoh reconcilia por key; la proyección a columnas da joins/FTS/reportes.
- **DEC-6 — Migración detrás de interfaces, `main` siempre desplegable.** `main` (libp2p + Turso/CDC) se mantiene con el remanente de hardening Parte A hasta un release intermedio; el rework ocurre en `zenoh-rearch`.

## 2. Arquitectura de datos (cómo encaja todo)

```
commit_event (app)
   │  firma Ed25519 (autor) + HLC
   ▼
Store::put(entity, doc_id, row_image, hlc)   →  key = syntrix/org/{id}/{entity}/{doc_id}
   │                                   │
   ▼ (proyección)                      ▼ (replicación)
 columnas SQLite                  Zenoh replication log (fingerprints)
   │  joins · FTS5 · reportes          │  anti-entropy entre réplicas (ancla ↔ hojas)
   ▼                                   ▼
 queries locales de la UI       convergencia LWW-per-key entre nodos
```

- **Escritura**: `commit_event` → `Store::put` proyecta a columnas SQLite y entrega el sample a Zenoh (`doc_id` como key, HLC como timestamp).
- **Reconciliación/catch-up**: la hace Zenoh (replication log + fingerprints). Un nodo que reconecta se alinea por anti-entropy. **No hay CDC ni snapshot manual.**
- **Scoped**: réplicas de hoja declaradas sobre sub-key-exprs por rol/entidad (`syntrix/org/{id}/{entity}/**` filtrado por `can_open`); `get` on-demand para lo fuera del slice.
- **Validación de autor**: al aplicar un sample se valida firma Ed25519 + permiso del autor por rol (código app-level actual, adaptado).

## 3. Interfaces (contratos de coexistencia y aislamiento)

**`crates/syntrix-transport` — trait `SyncTransport`** (permite libp2p ↔ Zenoh durante la migración):
- `session(mode)` (peer/client/router), `join_scope(org)`, `put(key,bytes,ts)`, `subscribe(key)->stream`, `query(key,payload)->reply`, `declare_queryable(key,handler)`, `presence()`.

**`crates/syntrix-store` — trait `Store`** (aísla la DB de la app):
- `put(entity, doc_id, row_image, hlc)`, `delete(entity, doc_id, hlc)`, `get(entity, doc_id)`, `query(sql/plan)`, `search(fts)`, `all_entries()`, `run_migrations()`.
- Impl de producción única: **`syntrix-store-sqlite`** (rusqlite `bundled`, FTS5).

## 4. Inventario: eliminar / mantener / modificar / crear

### MANTENER
- Toda la UI/React y apps Tauri (grids, detail, search, FieldRegistry, Drizzle proxy). La búsqueda pasa a FTS5 detrás de `Store::search`.
- Lógica de negocio: `commit_event`, upcasters, permisos `members`/`roles`/`can_open`/`can_write`, validación por autor.
- Identidad Ed25519 (`load_or_create_keypair`, `keypair.bytes`).
- `syntrix-ai`, `syntrix-logging`, `syntrix-migrate`, `syntrix-testkit` (salvo dependencias de red).

### MODIFICAR
- **`syntrix-core::addr`**: de `PeerId` libp2p a endpoint Zenoh; conservar pubkey Ed25519 como `node_id`.
- **`apps/{client,admin}/src-tauri/src/identity.rs`**: `P2PNode` → sesión Zenoh detrás de `SyncTransport`.
- **`apps/client/src-tauri/src/search.rs`**: la búsqueda actual usa funciones escalares Turso (`fts_match`/`fts_score`); reescribir a **FTS5** detrás de `Store::search`.
- **Proyección a columnas** (`upsert_document_full`): moverla detrás de `Store::put`.
- **`dirs_next::data_dir()`** → path resolver de Tauri (sandbox móvil) en `lib.rs`/`identity.rs`.

### ELIMINAR
- **`crates/syntrix-network` (internals libp2p)**: gossipsub, kademlia, request_response, autonat/dcutr/relay, bootstrap, reconnect, scoring, `behaviour.rs`, `codecs.rs`.
- **Stack de datos legacy Turso/CDC**: dependencia `turso_core`, `crates/syntrix-network::cdc` (lectura de `turso_cdc`), el loop `cdc_sync.rs`, `snapshot_org_rows` y el catch-up manual. Reemplazados por SQLite (DEC-2) + replicación Zenoh (DEC-1).
- **`apps/syntrix-relay`**: el router/ancla Zenoh cumple el rendezvous.
- **Tareas Parte A específicas de libp2p** (mDNS/bootstrap/relay/discover/reconnect): obsoletas bajo Zenoh; no portar.

### CREAR
- **`crates/syntrix-transport`** (trait) + **`crates/syntrix-zenoh`** (impl).
- **`crates/syntrix-store`** (trait) + **`crates/syntrix-store-sqlite`** (rusqlite).
- **Backend de storage Zenoh sobre `Store`**: implementa `zenoh_backend_traits::{Volume,Storage}` delegando en `Store` (put/delete/get/get_all_entries). Ata la replicación de Zenoh a SQLite.
- **Ancla headless** (artefacto deploy): `admin-headless` + **router Zenoh + storage replicado** + endpoint público (TCP/QUIC/TLS), `--keypair-file`, `--data-dir`.
- **Protocolo control admin** vía query firmada `syntrix/org/{id}/admin-control`.
- **Mapa de key expressions**: `syntrix/org/{org_id}/{entity}/{doc_id}`, `.../presence`, `.../roster`, `.../admin-control`.

## 5. Fases (rama `zenoh-rearch`)

### Z0 — Interfaces (sin romper `main`)
- Crear rama. Definir traits `SyncTransport` y `Store`.
- Refactorizar la app para hablar solo con `Store` y `SyncTransport`, con las impls actuales detrás (envolver el `turso_core` + libp2p existentes) para que los tests sigan verdes. Es el punto de anclaje; a partir de aquí se sustituyen las impls.

### Z1 — `Store` SQLite (rusqlite) reemplaza a Turso
- Implementar `syntrix-store-sqlite`: esquema/migraciones, proyección a columnas, FTS5.
- Cambiar la impl de `Store` de Turso a SQLite y **retirar la dependencia `turso_core`**.
- Validar paridad funcional (entidades, FTS, queries de la UI) contra el comportamiento actual.

### Z2 — Transporte Zenoh mínimo (LAN, peer)
- `syntrix-zenoh`: sesión, publishers/subscribers sobre `syntrix/org/{id}/{entity}/**`, scouting multicast LAN.
- Dos nodos LAN sincronizan por Zenoh (put/subscribe) con validación de autor Ed25519.

### Z3 — Replicación Zenoh + backend sobre `Store` (reemplaza CDC/catchup)
- Implementar el backend `zenoh_backend_traits::Storage` sobre `Store` (SQLite).
- Configurar storage-manager + replication (interval/hot/warm/`garbage_collection`) en cada nodo.
- **Retirar** el sync CDC/catchup manual: la convergencia la da la anti-entropy de Zenoh.
- Validar: un nodo offline que vuelve se alinea sin snapshot manual.

### Z4 — Ancla como router + acceso scoped
- Ancla = router Zenoh + storage replicado autoritativo. Clientes en modo **client** conectan outbound.
- Réplicas de hoja **scoped** por rol/entidad; `get` on-demand para el resto.
- Validar: cliente CGNAT sincroniza su slice; aislamiento por rol.

### Z5 — Control admin remoto + identidad
- Query firmada `syntrix/org/{id}/admin-control` (alta/baja/rol/revocación); llave raíz solo en el ancla; teléfono admin emparejado por QR con su propia llave.
- ACL Zenoh (TLS cert_common_names) como defensa en profundidad opcional.

### Z6 — Móvil
- Targets Tauri Android/iOS; paths sandbox; SQLite `bundled` compila para móvil (toolchain C de Tauri mobile).
- Foreground-sync + throttling batería; presencia por key Zenoh; push opt-in (APNs/FCM, dependencia central etiquetada).

### Z7 — App unificada + cutover
- Merge admin UI → cliente como módulo "Gobierno" (rol-aware, control remoto del ancla). Deprecar `apps/admin` standalone.
- Suite E2E completa verde sobre Zenoh+SQLite → deprecar `syntrix-network`/`syntrix-relay`, mergear. Actualizar ADR Zenoh a "Aceptado".

### Z8 — IoT (post-cutover)
- **Hojas MCU**: `zenoh-pico` (C), **sin DB local** — publican samples a `syntrix/org/{id}/{entity}/**`; el ancla/gateway persiste.
- **Gateways SBC (Linux ARM)**: `syntrix-store-sqlite` (SQLite embebido probado).
- Definir permisos/entidades acotadas para nodos IoT (sensor → escritura de ciertas entidades).

## 6. Riesgos

| Riesgo | Mitigación |
|---|---|
| Acople a APIs `unstable` de Zenoh (advanced pub/sub, replication) | Aceptado explícitamente. Fijar versión Zenoh; aislar tras `SyncTransport`; tests de convergencia. |
| La replicación Zenoh no converge / edge-cases (wildcard delete, tombstones, orden invertido) | Tests de anti-entropy en Z3 (offline/reconexión, deletes); configurar `garbage_collection`. |
| Cambio de modelo de sync (CDC → anti-entropy Zenoh) introduce regresiones | Validar paridad de convergencia en Z3 antes de retirar el path CDC. |
| Reescribir FTS escalar → FTS5 | Aislado tras `Store::search`; alcance acotado a `search.rs`. |
| Identidad: ACL Zenoh no es autor por-fila | DEC-4: Ed25519 app-level se conserva; ACL Zenoh solo defensa en profundidad. |
| Ancla detrás de NAT sin endpoint público | VPS/port-forward; documentar. El router debe ser alcanzable. |
| Rewrite se desborda | Interfaces (`Store`/`SyncTransport`) + fases; `main` siempre desplegable. |

## 7. Validación (global)

1. Z0: refactor a traits sin regresiones (tests actuales verdes con impls existentes).
2. Z1: paridad funcional de datos (entidades, FTS, queries de la UI) sobre SQLite; sin dependencia `turso_core`.
3. Z2: sync LAN por Zenoh con validación de autor.
4. Z3: **anti-entropy** — nodo offline reconecta y converge sin snapshot manual; deletes/tombstones correctos; sin path CDC.
5. Z4: cliente CGNAT sincroniza slice scoped; aislamiento por rol.
6. Z5: admin remoto sin llave raíz.
7. Z6: app arranca y sincroniza en Android/iOS reales (SQLite bundled).
8. Z7: E2E completa verde sobre Zenoh+SQLite antes del cutover.
9. Z8: hoja `zenoh-pico` publica a gateway; gateway SQLite persiste.

## 8. Decisiones abiertas (confirmar en implementación)

- ¿`main` se congela al arrancar Z0, o se termina el remanente de hardening Parte A para un release estable primero? (recomendado: terminar remanente, luego foco Zenoh).
- ¿Replicación con `History::Latest` (solo estado) o `History::All`? (recomendado: `Latest`; la auditoría histórica sigue en la tabla `event_log`).
- ¿Ancla hosteada por Syntrix (SaaS) además de self-host? (afecta pitch/pricing).
- ¿Un ancla por org (sin HA) o multi-ancla/réplica desde el inicio? (recomendado: una, HA después — Zenoh replication soporta múltiples réplicas cuando se necesite).
- ¿Spike temprano de `zenoh-pico` en paralelo o estrictamente post-cutover? (recomendado: spike de viabilidad temprano, integración post-cutover).

## 9. Fuera de alcance / nota de futuro

- **Turso como DB futura**: se descarta para esta rearquitectura (DEC-2). Si en el futuro Turso sale de beta y sus MVCC/DBSP/async justifican volver, el trait `Store` permite añadir una impl `syntrix-store-turso` sin tocar la app. No es parte de este plan.
