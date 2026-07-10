# Plan: Syntrix v2 — Greenfield backend (Zenoh + SQLite) con contrato IPC preservado

> **Estado**: Dirección aprobada. **Proyecto nuevo** (greenfield del backend/servicios), **portando** el frontend React actual sin reescribirlo, preservando el contrato Tauri IPC.
> **Fecha**: 2026-07-10.
> **Relacionado / supersede**: `1783697037000-zenoh-rearchitecture-mobile-iot.md` (mantiene el análisis y las decisiones de stack; este plan **reemplaza su enfoque "modificar en sitio"** por greenfield). ADRs: `1783695719137-adr-libp2p-vs-zenoh.md`, `1783695719135-adr-libp2p-vs-headscale.md`. `main` (repo actual) sigue despachando hasta paridad.
> **Ámbito**: proyecto nuevo. Backend/servicios desde cero; frontend y reglas de negocio portados.

## 0. Enfoque

El core cambia entero (transporte libp2p→Zenoh, sync CDC→replicación Zenoh, DB Turso→SQLite) y la topología se invierte (malla→hub-and-spoke). Envolver lo viejo detrás de adaptadores es trabajo desechable. Por eso: **construir el backend/servicios en un proyecto nuevo**, y **portar** lo maduro y transporte-agnóstico.

La palanca que lo hace seguro: **el frontend no habla con libp2p ni Turso, habla con comandos Tauri IPC**. Si el contrato IPC se congela y el backend nuevo lo implementa, el frontend se porta **sin una sola modificación**.

## 1. Decisiones firmes (heredadas, ya resueltas)

- **Transporte**: Eclipse Zenoh v1.9.x (modos peer/client/router).
- **Motor de sync**: replicación Zenoh (storage-manager + backend custom sobre `Store`), anti-entropy por fingerprints, HLC nativo. **Sin CDC.**
- **DB**: SQLite vía `rusqlite` (feature `bundled`, FTS5), detrás de un trait `Store`.
- **Identidad**: Ed25519 a nivel app para firmar/validar autor de fila + permisos por rol (Zenoh no da identidad de autor por-fila; su ACL/TLS es defensa en profundidad).
- **Modelo núcleo**: **espacio de keys jerárquico** `syntrix/org/{org}/{modulo?}/{entidad}/{doc}[/{sub}/{line}]`; **permisos = grants de key-expression** (DEC-8); **granularidad per-fila** (DEC-7 revisada). Ver §1.5.
- **Topología**: hub-and-spoke. **Ancla headless** por org (router Zenoh + store replicado autoritativo + autoridad admin). **No hay relay separado** — el ancla lo subsume.
- **Apps**: **una app unificada role-aware** (empleado + gobierno admin) + **servicio ancla headless**. Se depreca la app admin standalone.
- **`main`** (repo actual) se mantiene como producto vivo y **referencia** de reglas de negocio/casos-borde hasta el cutover.

## 1.5 Modelo de dominio y multi-org (transversal — lo que v2 DEBE preservar)

Verificado en el código actual. Este es el modelo que el greenfield debe reproducir de punta a punta; es donde más fácil se pierden casos-borde.

### Orgs (unidad de aislamiento)
- Un dispositivo pertenece a **múltiples orgs** simultáneamente (`orgs: HashMap`, `active_org`, `orgs.json`). El rol del nodo **difiere por org**.
- **Una sola DB por dispositivo** compartida entre orgs, con columna **`org_id`** en cada tabla; `NamespaceRegistry` keyed por `OrgId` (`crates/syntrix-core/src/registry.rs`).
- Mapeo v2: **key por org** `syntrix/org/{org_id}/**`; **replicación por scope de org** (cada org converge aislada); el storage backend parsea `org_id` de la key y escribe con `org_id` scoping. `active_org` es **solo UI**: el backend sincroniza **todas** las orgs.
- **Ancla multi-org**: un proceso ancla sirve N orgs (scope + config de replicación por org). Un dispositivo en orgs de anclas distintas mantiene una sesión Zenoh con múltiples endpoints (Zenoh multiplexa).

### Espacio de keys jerárquico (DEC-8 — modelo núcleo de v2)
Todo el dominio se direcciona como un árbol de key expressions; **entidad, jerarquía padre/hijo y permisos son el mismo path**:
```
syntrix/org/{org}/{modulo?}/{entidad}/{doc_id}[/{subentidad}/{line_id}]
   ej: syntrix/org/acme/compras/ordenes/{id}/items/{line}
```
- **Módulos opcionales** (compras, ventas…): nivel de agrupación **dirigido por metadata del schema** (Drizzle). Entidades sin módulo declarado caen en un nivel plano; el módulo aparece cuando el schema lo declare. No obligatorio hoy (6 entidades).
- **Storage sigue relacional**: la jerarquía es la capa de direccionamiento/permiso/sync; el `Store` (SQLite) mantiene tablas planas y **mapea path↔(tabla, fila)** para conservar joins/FTS/reportes. NO se convierte en KV.
- **Queryables por subárbol**: catch-up/scoped-sync = `subscribe`/`get` a un subárbol (`get .../compras/**`). La réplica scoped de un rol = los subárboles que sus grants conceden.

### Dispositivos, roles, permisos (DEC-8 — permisos como grants de key-expr)
- **Device**: `node_id` (pubkey Ed25519), `role`, `active`, `person`, `name` — **por org**.
- **Role = conjunto de grants de key-expression** (read/write) en vez de listas planas. Retro-compatible: `can_open:[invoices]` ≡ grant read `.../invoices/**`; `*` ≡ `.../**`. Un grant a nivel módulo (`.../compras/**`) cubre sus entidades e hijas. Defaults: `admin`(`.../**`), `sales`, `contabilidad`.
- **Doble aplicación**: la app es autoritativa (valida cada escritura por autor contra los grants del rol); el **ACL de Zenoh** (key-expr matching) es defensa en profundidad en el transporte.
- **Roster (`members`/`roles`)**: mapeo `node_id → role` + los grants del rol, por org. Base de la **validación por autor**: una escritura remota se acepta solo si el path de la key cae dentro de un grant `write` del rol del autor.
- **Autoridad**: el ancla (admin) funda la org (`create_org` registra el device admin) y es autoritativo del roster. Las **keys de roster/roles** (`syntrix/org/{org}/roster/**`, `.../roles/**`) solo las escribe la **llave admin**; los demás las validan contra esa firma. Revocación = baja de device (`update_device active=false`) + corte de sesión + revocar grant.

### Entidades y padre/hijo (= "namespaces", término legacy)
- "namespace" en el código = **entidad**. 6 hoy: `customers, suppliers, products, invoices, orders, payroll`.
- **Padre/hijo = niveles del path**: `invoices/{id}/items/{line}` (hoy `invoice_items`), `orders/{id}/items/{line}`. El grant del padre cubre a los hijos (prefijo de path).
- Columnas de sync en cada fila: `org_id, doc_id, change_time (HLC/LWW), node_id (autor)`. Esquema desde Drizzle (`packages/shared-drizzle` → `schema.json`); el backend lo consume genéricamente.

### DEC-7 (revisada) — Granularidad = per-fila / jerárquica
- **key = fila**: header en `.../{entidad}/{doc_id}`, cada línea hija en `.../{entidad}/{doc_id}/{subentidad}/{line_id}`. **LWW por fila** (editar líneas distintas del mismo documento no se pisa).
- **Borrado de documento** = wildcard-delete del subárbol (`delete .../{doc_id}/**`), nativo en Zenoh (tombstones de subárbol + `garbage_collection`). Sin cascada manual.
- **Reconstruir documento** = query del subárbol (`get .../{doc_id}/**`); el `Store` lo proyecta a tabla padre + hijas.
- **Edge de consistencia**: un subscriber puede ver el header antes que sus líneas; la UI lee el documento como subárbol y tolera líneas que llegan un instante después (o marca "cargando líneas").

### Auditoría (`event_log`)
- `event_log` es **tabla de auditoría durable local** (Decisión 5): una fila por cambio (`entity/change_type/doc_id/row_image/change_time/node_id`), **no** transporte de sync. Se escribe al aplicar cambios; `audit_query`/`run_sql` la leen. Vive en el `Store` (SQLite); **no se replica** por Zenoh.

### IA org + permiso-aware
- `syntrix-ai` es **org-aware y permission-aware**: sus tools (`check_read_access(org,entity)`, `query_entity(org,...)`, `search_entity(org,...)`) van scoped por `org_id` y respetan `can_open`. v2 debe preservar ese scoping: los tools pasan por el mismo `Store` + checks de permiso.

**Fuera de alcance v2 (nota de futuro)**: **interconexión cross-org** (redes de franquicias / 100+ colectivos que comparten datos entre sí, `vision.md:28`). El modelo por-org aísla; el puente cross-org (nodo miembro de varias orgs que reexpone datos, o federación de anclas) es track posterior.

## 2. El contrato IPC a congelar (la frontera del proyecto)

El backend nuevo DEBE implementar estos comandos con **mismos nombres, args y forma de retorno** para portar el frontend intacto. Catalogado del código actual:

**Datos / entidades / búsqueda:**
`commit_event`, `query_entity`, `query_entity_advanced`, `search_entity`, `drizzle_execute`, `get_schema_registry`, `check_entity_access`, `audit_query`, `run_sql`, `list_saved_views`, `create_saved_view`, `delete_saved_view`, `seed_dev_data`

**Live / sync:**
`live_subscribe`, `live_unsubscribe`, `get_updates_since`, `get_sync_info`, `sync_status`, `network_status`

**Org / identidad / enrolamiento:**
`get_node_id`, `list_orgs`, `set_active_org`, `create_org`, `join_org`, `enroll_org`, `get_invites`, `get_invite_info`, `send_invite`, `get_endpoint_addr`, `get_pending_enrollments`, `approve_enrollment`, `reject_enrollment`

**Gobierno (devices/roles):**
`add_device`, `update_device`, `list_devices`, `create_role`, `update_role`, `list_roles`

**Logs / IA:**
`query_logs`, `summarize_logs`, `start_tail_logs`, `ai_chat`, `ai_status`

Notas de preservación (cambia la implementación, NO el contrato):
- `commit_event` → escribe en `Store` (proyección a columnas) + firma Ed25519 + `put` a Zenoh.
- `get_updates_since` / `live_subscribe` → sobre un **change-stream del `Store`** (hooks SQLite), no sobre `turso_cdc`.
- `search_entity` → **FTS5** en vez de `fts_match`/`fts_score` escalares.
- `drizzle_execute` → SQL dialecto SQLite (Drizzle ya lo genera).
- `get_sync_info` / `sync_status` → estado de sesión/replicación Zenoh.
- `send_invite` / `enroll_org` / `get_pending_enrollments` / `approve_enrollment` / `reject_enrollment` → sobre query firmada Zenoh `syntrix/org/{id}/admin-control` (el flujo de enrolamiento inverso ya existe en el contrato actual).
- La app unificada expone la **unión** de los command sets de client + admin; los de gobierno se gatean por rol admin.

## 3. Estructura del proyecto nuevo

```
syntrix/ (proyecto nuevo)
├── apps/
│   ├── app/                    ← Tauri: frontend React PORTADO + backend Rust nuevo (todos los comandos IPC)
│   └── anchor/                 ← servicio headless: router Zenoh + Store replicado + autoridad admin
├── crates/
│   ├── syntrix-ipc/            ← contrato IPC + handlers (compartidos por app y anchor headless)
│   ├── syntrix-domain/         ← PORTADO: permisos, upcasters, HLC, proyección a columnas, folios, schema registry
│   ├── syntrix-store/          ← trait Store
│   ├── syntrix-store-sqlite/   ← rusqlite (bundled) + FTS5 + migraciones
│   ├── syntrix-transport/      ← trait SyncTransport
│   ├── syntrix-zenoh/          ← impl Zenoh: sesión, pub/sub, query, storage backend, replication
│   ├── syntrix-identity/       ← Ed25519: keypair, firma/validación de autor
│   ├── syntrix-ai/             ← PORTADO
│   └── syntrix-logging/        ← PORTADO
└── packages/
    ├── syntrix-ui/             ← PORTADO (design system)
    └── shared-drizzle/         ← PORTADO (schema/entidades)
```

## 4. Portar vs construir

**PORTAR tal cual (copiar, no reescribir):**
- Frontend React completo (EntityGrid, DetailPanel, FieldRegistry, TanStack, shadcn, Command Palette).
- `packages/syntrix-ui`, `packages/shared-drizzle` (schema/entidades Drizzle).
- `syntrix-ai`, `syntrix-logging`.

**PORTAR como referencia + limpiar (misma lógica, crate nuevo `syntrix-domain`):**
- Permisos por rol/autor (`can_open`/`can_write` por entidad, wildcard `*`, hijo hereda entidad padre; validación por `node_id`).
- Roster (`members`/`roles`) + validación por autor de eventos remotos.
- Upcasters de `schema_version`, HLC/LWW por documento, proyección documento→padre/hijo, folios, schema registry (entidad/namespace + aliasing).
- Auditoría `event_log` (escritura al aplicar; lectura por `audit_query`/`run_sql`).
- Scoping org+permiso de los tools de `syntrix-ai` (`check_read_access`, `query/search` por `org_id`).

**CONSTRUIR nuevo:**
- `syntrix-store` + `syntrix-store-sqlite`, `syntrix-transport` + `syntrix-zenoh`, backend de storage Zenoh sobre `Store`, `syntrix-identity`, `syntrix-ipc`, la app unificada (cascarón backend) y el `anchor`.

## 5. Fases (ordenadas)

### G0 — Scaffold + congelar contrato + portar assets
- Crear proyecto nuevo (repo o workspace — ver §8).
- Portar frontend, `syntrix-ui`, `shared-drizzle`, `syntrix-ai`, `syntrix-logging`.
- Congelar el **catálogo de comandos IPC** (§2) como contrato versionado (tipos de args/retorno).

### G1 — `Store` SQLite (rutas de lectura del frontend)
- `syntrix-store` (trait) + `syntrix-store-sqlite`: migraciones desde Drizzle, **mapeo path↔(tabla, fila)** (DEC-7 per-fila: header en `.../{entidad}/{doc}`, líneas en `.../{doc}/{sub}/{line}`), aliasing de entidad (`invoice`/`invoices`), columnas de sync (`org_id/doc_id/change_time/node_id`), FTS5.
- Reconstrucción de documento = query de subárbol (`get .../{doc}/**`) proyectada a padre+hijas.
- Implementar `query_entity`, `query_entity_advanced`, `search_entity`, `drizzle_execute`, `get_schema_registry`, `audit_query`/`run_sql` (leen `event_log` local), saved views.
- **Meta**: el frontend renderiza grids/búsqueda contra SQLite (datos sembrados con `seed_dev_data`); facturas con líneas hijas se leen como subárbol.

### G2 — `syntrix-domain` + escritura + autorización
- Portar: **permisos como grants de key-expr (DEC-8)** por rol (retro-compat con `can_open`/`can_write`; hijo cubierto por prefijo de path; wildcard `.../**`), roster (`members`/`roles`), **validación por autor** (path de la key ∈ grant write del rol), upcasters de `schema_version`, HLC/**LWW por fila**, folios.
- `syntrix-identity` (Ed25519). Implementar `commit_event` (valida grant propio → firma → `Store::put` de las filas afectadas → escribe auditoría en `event_log`) y `check_entity_access`.
- **Meta**: CRUD local completo con permisos, auditoría y reactividad optimista, sin red.

### G3 — Transporte Zenoh (LAN peer) + validación por autor en apply
- `syntrix-transport` (trait) + `syntrix-zenoh` (sesión, pub/sub, query).
- Al recibir un sample: **validar firma Ed25519 + grant** (path de la key ∈ grant write del rol del autor en esa org) **antes** de escribir; descartar si no autorizado. Escribir auditoría en `event_log`.
- Dos instancias en LAN se descubren (scouting) y propagan writes autorizados.

### G4 — Replicación Zenoh + backend sobre `Store` + live
- Backend `zenoh_backend_traits::{Volume,Storage}` sobre `Store`; **el `put` mapea la key (fila) a (tabla, fila)** y corre la validación por autor de G3; **borrado de documento = wildcard-delete de subárbol** (`.../{doc}/**`) → cascada de filas hijas; storage-manager + replication **por scope de org** (`syntrix/org/{org_id}/**`, interval/hot/warm/gc).
- `get_updates_since` / `live_subscribe` / `live_unsubscribe` sobre change-stream del `Store`.
- `get_sync_info` / `sync_status` desde estado Zenoh.
- **Meta**: anti-entropy real (nodo offline reconecta y converge, sin snapshot manual); líneas hijas convergen aunque lleguen tras el header.

### G5 — Ancla headless + roster + gobierno/enrolamiento
- `apps/anchor`: router Zenoh + Store replicado autoritativo + endpoint público (QUIC/TCP/TLS), `--keypair-file`, `--data-dir`. **Multi-org capable**: un proceso ancla sirve N orgs.
- **Roster autoritativo**: las keys `syntrix/org/{org}/roster/**` y `.../roles/**` solo las escribe la **llave admin**; los nodos las validan contra esa firma y pueblan `members`/`roles` locales (base de la validación por autor).
- Protocolo `syntrix/org/{id}/admin-control` (query firmada): `create_org` (funda org + device admin), `add_device`, `update_device` (baja/revocación), `create_role`, `update_role`, `send_invite`, `enroll_org`, `get_pending_enrollments`, `approve_enrollment`, `reject_enrollment`.
- Emparejamiento teléfono-admin ↔ ancla por QR; llave raíz solo en el ancla.

### G6 — App unificada role-aware
- Fusionar comandos client+admin; módulo "Gobierno" gateado por rol admin (control remoto del ancla).
- Clientes en modo **client** conectan outbound al ancla; **réplica scoped = subárboles concedidos por los grants del rol** (`subscribe`/`get` por subárbol); `get` on-demand para lo demás.

### G7 — Móvil
- Targets Tauri Android/iOS; paths sandbox; SQLite `bundled` compila con toolchain C de Tauri mobile.
- Foreground-sync + throttling batería; presencia por key Zenoh; push opt-in (APNs/FCM, dependencia central etiquetada).

### G8 — Paridad + cutover
- Suite E2E de paridad contra `main` (invite/enroll, sync multi-nodo, permisos, búsqueda, auditoría).
- Cuando v2 pase la suite: v2 se vuelve el producto; `main` queda archivado como referencia.

### G9 — IoT (post-cutover)
- Hojas MCU: `zenoh-pico` (C), sin DB local, publican a `syntrix/org/{id}/{entity}/**`.
- Gateways SBC (Linux ARM): `syntrix-store-sqlite`. Permisos/entidades acotadas para nodos IoT.

## 6. Riesgos

| Riesgo | Mitigación |
|---|---|
| Rewrite pierde casos-borde del backend actual | `main` es la referencia viva; portar reglas a `syntrix-domain` con tests derivados del comportamiento actual. |
| Deriva del contrato IPC rompe el frontend portado | Congelar el catálogo (§2) con tests de contrato; el frontend no se toca. |
| APIs `unstable` de Zenoh (advanced pub/sub, replication) | Aceptado; fijar versión; aislar tras `SyncTransport`; tests de convergencia. |
| Anti-entropy Zenoh: wildcard delete/tombstones/orden | Tests dedicados en G4; configurar `garbage_collection`. |
| Identidad: ACL Zenoh no es autor por-fila | Ed25519 app-level (`syntrix-identity`); ACL Zenoh solo defensa en profundidad. |
| Storage backend Zenoh escribe sin validar autor (bypass de permisos) | El `put` del backend corre validación por autor (firma + grant de key-expr) antes de escribir; tests de aislamiento por rol. |
| Roster no llega antes que los datos → autor no validable | El roster (keys admin) se replica/pide primero; si falta el rol del autor, encolar/reintentar en vez de aceptar. |
| Per-fila: subscriber ve header antes que sus líneas (documento parcial) | La UI lee el documento como subárbol (`get .../{doc}/**`) y tolera líneas que llegan un instante después; opcional marca "cargando líneas". |
| Per-fila: borrar documento debe eliminar todas sus líneas | Wildcard-delete de subárbol nativo de Zenoh (`.../{doc}/**`) + `garbage_collection`; test de borrado en G4. |
| Grants de key-expr mal formados abren/cierran de más | Retro-compat estricta (`[entidad]`≡`.../entidad/**`); tests de matching de grant vs key. |
| Ancla sin endpoint público (NAT oficina) | VPS/port-forward; documentar. |
| v2 tarda en alcanzar paridad | `main` sigue despachando; cutover solo tras E2E verde. |

## 7. Validación

1. G1: frontend renderiza grids/búsqueda contra SQLite; facturas con líneas hijas reconstruidas como subárbol; `get_schema_registry`/`drizzle_execute` funcionan.
2. G2: CRUD local + permisos como grants de key-expr (incl. hijo cubierto por prefijo) + upcasters + auditoría en `event_log` (paridad con `main` sin red).
3. G3: sync LAN por Zenoh; **un autor cuyo grant no cubre el path es rechazado en apply** (no se escribe ni audita).
4. G4: anti-entropy (offline→converge); **dos nodos editan líneas distintas del mismo documento sin pisarse** (LWW per-fila); borrar documento elimina sus líneas (wildcard-delete); live sin CDC.
5. G5: roster firmado por admin puebla `members`/`roles`; enrolamiento inverso + gobierno remoto sin llave raíz en el teléfono.
6. G6: app unificada; rol admin ve gobierno; **réplica scoped = subárboles concedidos**; IA respeta org + grants.
7. Multi-org: un dispositivo en 2+ orgs sincroniza cada org aislada (sin cruce); rol distinto por org respetado; un ancla sirviendo 2 orgs las mantiene separadas.
8. G7: app arranca y sincroniza en Android/iOS reales (SQLite bundled).
9. G8: E2E de paridad verde antes del cutover.

## 8. Decisiones abiertas (confirmar)

- **Repo del proyecto nuevo**: ¿repo nuevo independiente, o nuevo root/workspace junto al actual? (recomendado: repo nuevo, `main` intacto como referencia).
- **Nombre/versión**: ¿`syntrix` v2 nuevo, o convivencia temporal de nombres? (recomendado: proyecto `syntrix`, el actual pasa a `syntrix-legacy` al cutover).
- **Replicación**: ¿`History::Latest` (estado) o `History::All`? (recomendado: `Latest`; auditoría en tabla `event_log`).
- **Ancla hosteada por Syntrix (SaaS)** además de self-host (afecta pitch/pricing).
- **Un ancla por org** al inicio vs multi-ancla/HA (recomendado: una, HA después).
- **Persistencia multi-org**: ¿una DB por dispositivo con `org_id` scoping (como hoy) o una DB por org? (recomendado: una DB con `org_id` — preserva el contrato y Drizzle; per-org-DB solo si se requiere aislamiento físico fuerte).
- **Ancla multi-tenant**: ¿un proceso ancla sirve varias orgs desde el inicio, o una org por proceso? (recomendado: multi-org capable en el diseño, deploy flexible según cliente).
- **Módulos en el path**: ¿introducir el nivel `{modulo}` ahora (dirigido por schema Drizzle) o mantener entidades planas hasta que el schema los declare? (recomendado: soportar el segmento opcional en el path desde el diseño; poblarlo cuando el schema declare módulos — evita sobre-ingeniería con 6 entidades).
- **Grants de key-expr en la UI de gobierno**: ¿exponer los permisos como listas de entidades (como hoy) o como editor de grants de key-expr? (recomendado: UI por entidades/módulos que compila a grants por debajo; grants crudos solo para poweruser).